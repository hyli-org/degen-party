use anyhow::{Context, Result};
use axum::Router;
use clap::{command, Parser};
use client_sdk::rest_client::NodeApiHttpClient;
use degen_party::{
    ensure_registration::EnsureRegistration, fake_lane_manager::FakeLaneManager,
    AuthenticatedMessage, Conf, InboundWebsocketMessage, OutboundWebsocketMessage,
};
use hyle_modules::{
    bus::{metrics::BusMetrics, SharedMessageBus},
    modules::{
        da_listener::{DAListener, DAListenerConf},
        websocket::WebSocketModule,
        BuildApiContextInner, ModulesHandler,
    },
    utils::logger::setup_tracing,
};
use sdk::ContractName;
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use std::{
    env,
    sync::{Arc, Mutex},
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(long, default_value = "config.toml")]
    pub config_file: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config = Conf::new(args.config_file).context("Failed to load config")?;

    setup_tracing(&config.log_format, "degen_party".to_string()).context("setting up tracing")?;

    // Ensure the data directory exists
    if !config.data_directory.exists() {
        std::fs::create_dir_all(&config.data_directory).context(format!(
            "Failed to create data directory: {}",
            config.data_directory.display()
        ))?;
    }

    tracing::info!("Starting app with config: {:?}", &config);

    let config = Arc::new(config);

    let bus = SharedMessageBus::new(BusMetrics::global("rollup".to_string()));

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
        config: config.clone(),
        client,
        crypto: degen_party::CryptoContext {
            secp: secp.clone(),
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

    handler
        .build_module::<EnsureRegistration>(ctx.clone())
        .await?;

    handler
        .build_module::<WebSocketModule<AuthenticatedMessage<InboundWebsocketMessage>, OutboundWebsocketMessage>>(
            config.websocket.clone(),
        )
        .await?;
    handler.build_module::<FakeLaneManager>(ctx.clone()).await?;

    handler
        .build_module::<DAListener>(DAListenerConf {
            data_directory: config.data_directory.clone(),
            da_read_from: config.da_read_from.clone(),
            start_block: Some(sdk::BlockHeight(config.start_block)),
            timeout_client_secs: 10,
        })
        .await?;

    degen_party::rollup_execution::setup_rollup_execution(ctx.clone(), &mut handler).await?;
    if config.run_prover {
        tracing::info!("Setting up auto provers");
        degen_party::proving::setup_auto_provers(ctx.clone(), &mut handler).await?;
    }

    tracing::info!("Starting modules");

    // Run forever
    handler.start_modules().await?;
    handler.exit_process().await?;

    Ok(())
}
