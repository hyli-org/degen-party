use anyhow::{Context, Result};
use clap::{command, Parser};
use client_sdk::rest_client::NodeApiHttpClient;
use degen_party::{
    crash_game::CrashGameModule, ensure_registration::EnsureRegistration,
    fake_lane_manager::FakeLaneManager, game_state::GameStateModule, AuthenticatedMessage,
    InboundWebsocketMessage, OutboundWebsocketMessage,
};
use hyle_modules::{
    bus::{metrics::BusMetrics, SharedMessageBus},
    modules::{
        da_listener::{DAListener, DAListenerConf},
        websocket::WebSocketModule,
        ModulesHandler,
    },
    utils::{conf, logger::setup_tracing},
};
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use std::{env, sync::Arc};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(long, default_value = "config.toml")]
    pub config_file: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config =
        conf::Conf::new(args.config_file, None, Some(true)).context("reading config file")?;

    setup_tracing(
        &config.log_format,
        format!("{}(nopkey)", config.id.clone(),),
    )
    .context("setting up tracing")?;

    tracing::info!("Starting app with config: {:?}", &config);
    let config = Arc::new(config);

    let bus = SharedMessageBus::new(BusMetrics::global("rollup".to_string()));

    let client = Arc::new(NodeApiHttpClient::new("http://localhost:4321".to_string())?);

    let secp = Secp256k1::new();
    let secret_key =
        hex::decode(env::var("DEGEN_PARTY_BACKEND_PKEY").unwrap_or(
            "0000000000000001000000000000000100000000000000010000000000000001".to_string(),
        ))
        .expect("DEGEN_PARTY_BACKEND_PKEY must be a hex string");
    let secret_key = SecretKey::from_slice(&secret_key).expect("32 bytes, within curve order");
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    //let sig = secp.sign_ecdsa(message, &secret_key);
    //assert!(secp.verify_ecdsa(message, &sig, &public_key).is_ok());

    let ctx = Arc::new(degen_party::Context {
        client,
        crypto: degen_party::CryptoContext {
            secp: secp.clone(),
            secret_key,
            public_key,
        }
        .into(),
        data_directory: config.data_directory.clone(),
    });

    tracing::info!("Setting up modules");

    // Initialize modules
    let mut handler = ModulesHandler::new(&bus).await;

    handler
        .build_module::<EnsureRegistration>(ctx.clone())
        .await?;

    handler.build_module::<GameStateModule>(ctx.clone()).await?;
    handler.build_module::<CrashGameModule>(ctx.clone()).await?;
    handler
        .build_module::<WebSocketModule<AuthenticatedMessage<InboundWebsocketMessage>, OutboundWebsocketMessage>>(
            config.websocket.clone().into(),
        )
        .await?;
    handler.build_module::<FakeLaneManager>(ctx.clone()).await?;

    handler
        .build_module::<DAListener>(DAListenerConf {
            data_directory: config.data_directory.clone(),
            da_read_from: config.da_read_from.clone(),
            start_block: None,
        })
        .await?;

    degen_party::proving::setup_auto_provers(ctx.clone(), &mut handler).await?;

    tracing::info!("Starting modules");

    // Run forever
    handler.start_modules().await?;
    handler.exit_process().await?;

    Ok(())
}
