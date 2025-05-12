use anyhow::{Context, Result};
use clap::{command, Parser};
use degen_party::{
    crash_game::CrashGameModule, fake_lane_manager::FakeLaneManager, game_state::GameStateModule,
    AuthenticatedMessage, InboundWebsocketMessage, OutboundWebsocketMessage,
};
use hyle_modules::{
    bus::{metrics::BusMetrics, SharedMessageBus},
    modules::{
        websocket::{WebSocketModule, WebSocketModuleCtx},
        ModulesHandler,
    },
    utils::{conf, logger::setup_tracing},
};
use std::sync::Arc;

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

    // Initialize the message bus
    let bus = SharedMessageBus::new(BusMetrics::global("rollup".to_string()));

    // Create common context
    let ctx = Arc::new(degen_party::Context {
        bus: bus.new_handle(),
        config: config.clone(),
    });

    tracing::info!("Setting up modules");

    // Initialize modules
    let mut handler = ModulesHandler::new(&bus).await;

    handler.build_module::<GameStateModule>(ctx.clone()).await?;
    handler.build_module::<CrashGameModule>(ctx.clone()).await?;
    handler
        .build_module::<WebSocketModule<AuthenticatedMessage<InboundWebsocketMessage>, OutboundWebsocketMessage>>(
            WebSocketModuleCtx {
                bus: ctx.bus.new_handle(),
                config: config.websocket.clone().into(),
            },
        )
        .await?;
    handler.build_module::<FakeLaneManager>(ctx.clone()).await?;

    tracing::info!("Starting modules");

    // Run forever
    handler.start_modules().await?;
    handler.exit_process().await?;

    Ok(())
}
