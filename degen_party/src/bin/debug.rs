use anyhow::{Context, Result};
use axum::Router;
use clap::{command, Parser};
use client_sdk::rest_client::NodeApiHttpClient;
use config::{Config, Environment, File};
use degen_party::debug::DebugAnalyzer;
use hyle_modules::{
    bus::{metrics::BusMetrics, SharedMessageBus},
    modules::{
        da_listener::{DAListener, DAListenerConf},
        BuildApiContextInner, ModulesHandler,
    },
};
use sdk::ContractName;
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use serde::{Deserialize, Serialize};
use std::{
    env,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(long, default_value = "config.toml")]
    pub config_file: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ContractsConf {
    pub board_game: String,
    pub crash_game: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Conf {
    /// The log format to use - "json", "node" or "full" (default)
    pub log_format: String,
    /// Directory name to store node state.
    pub data_directory: PathBuf,

    pub start_block: u64,

    /// The address of the Rest API to connect to
    pub node_api: String,

    pub contracts: ContractsConf,

    /// When running only the indexer, the address of the DA server to connect to
    pub da_read_from: String,
}

impl Conf {
    pub fn new(config_files: Vec<String>) -> Result<Self, anyhow::Error> {
        let mut s = Config::builder().add_source(File::from_str(
            include_str!("../conf_defaults.toml"),
            config::FileFormat::Toml,
        ));
        // Priority order: config file, then environment variables
        for config_file in config_files {
            s = s.add_source(File::with_name(&config_file).required(false));
        }
        let conf: Self = s
            .add_source(
                Environment::with_prefix("hyle")
                    .separator("__")
                    .prefix_separator("_")
                    .list_separator(",")
                    .try_parsing(true),
            )
            .build()?
            .try_deserialize()?;
        Ok(conf)
    }
}

use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config = Conf::new(args.config_file).context("Failed to load config")?;

    // Set up tracing to print to a file
    let log_file_writer = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("debug_analyzer.log")
        .context("Failed to open log file")?;
    let mut filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::DEBUG.into())
        .from_env()?;
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .compact()
                .with_writer(log_file_writer)
                .with_filter(filter),
        )
        .init();

    tracing::info!("Starting debug analyzer with config: {:?}", &config);
    let config = Arc::new(config);

    let bus = SharedMessageBus::new(BusMetrics::global("debug".to_string()));
    let client = Arc::new(NodeApiHttpClient::new(config.node_api.clone())?);

    let secp = Secp256k1::new();
    let secret_key =
        hex::decode(env::var("DEGEN_PARTY_BACKEND_PKEY").unwrap_or(
            "0000000000000001000000000000000100000000000000010000000000000001".to_string(),
        ))
        .expect("DEGEN_PARTY_BACKEND_PKEY must be a hex string");
    let secret_key = SecretKey::from_slice(&secret_key).expect("32 bytes, within curve order");
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    // Unused in practice for now.
    let api = Arc::new(BuildApiContextInner {
        router: Mutex::new(Some(Router::new())),
        openapi: Default::default(),
    });

    let ctx = Arc::new(degen_party::Context {
        config: Arc::new(degen_party::Conf::default()),
        client,
        crypto: degen_party::CryptoContext {
            secp,
            secret_key,
            public_key,
        }
        .into(),
        api,
        data_directory: config.data_directory.clone(),
        board_game: ContractName::new(config.contracts.board_game.clone()),
        crash_game: ContractName::new(config.contracts.crash_game.clone()),
    });

    tracing::info!("Setting up modules");

    // Initialize modules
    let mut handler = ModulesHandler::new(&bus).await;

    //setup_rollup_execution(ctx.clone(), &mut handler)
    //    .await
    //    .expect("Failed to setup rollup execution");

    handler.build_module::<DebugAnalyzer>(ctx.clone()).await?;
    handler
        .build_module::<DAListener>(DAListenerConf {
            data_directory: config.data_directory.clone(),
            da_read_from: config.da_read_from.clone(),
            start_block: Some(sdk::BlockHeight(config.start_block)),
        })
        .await?;

    tracing::info!("Starting modules");

    // Run forever
    handler.start_modules().await?;
    handler.exit_process().await?;

    Ok(())
}
