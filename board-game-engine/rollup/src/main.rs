use anyhow::Result;
use hyle::{
    bus::{metrics::BusMetrics, SharedMessageBus},
    utils::modules::ModulesHandler,
};
use rollup::{
    crash_game::CrashGameModule, game_state::GameStateModule, websocket::WebSocketModule,
};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "board_game_engine=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize the message bus
    let bus = SharedMessageBus::new(BusMetrics::global("rollup".to_string()));

    // Create common context
    let ctx = Arc::new(rollup::Context {
        bus: bus.new_handle(),
    });

    tracing::info!("Setting up modules");

    // Initialize modules
    let mut handler = ModulesHandler::new(&bus).await;
    handler.build_module::<GameStateModule>(ctx.clone()).await?;
    handler.build_module::<CrashGameModule>(ctx.clone()).await?;
    handler.build_module::<WebSocketModule>(ctx.clone()).await?;

    tracing::info!("Starting modules");

    // Run forever
    handler.start_modules().await?;
    handler.exit_process().await?;

    Ok(())
}
