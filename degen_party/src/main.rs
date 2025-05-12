use anyhow::{Context, Result};
use clap::{command, Parser};
use client_sdk::{helpers::test::TxExecutorTestProver, rest_client::NodeApiHttpClient};
use degen_party::{
    crash_game::CrashGameModule,
    fake_lane_manager::{BoardGameExecutor, CrashGameExecutor, FakeLaneManager},
    game_state::GameStateModule,
    AuthenticatedMessage, InboundWebsocketMessage, OutboundWebsocketMessage,
};
use hyle_modules::{
    bus::{metrics::BusMetrics, SharedMessageBus},
    modules::{
        da_listener::{DAListener, DAListenerConf},
        prover::{AutoProver, AutoProverCtx},
        websocket::WebSocketModule,
        ModulesHandler,
    },
    utils::{conf, logger::setup_tracing},
};
use sdk::BlockHeight;
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

    let bus = SharedMessageBus::new(BusMetrics::global("rollup".to_string()));
    let ctx = Arc::new(degen_party::Context {});

    tracing::info!("Setting up modules");

    // Initialize modules
    let mut handler = ModulesHandler::new(&bus).await;

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

    let client = Arc::new(NodeApiHttpClient::new("http://localhost:4321".to_string())?);

    #[cfg(not(feature = "fake_proofs"))]
    let board_game_prover = Arc::new(client_sdk::helpers::SP1Prover::new(
        contracts::ZKPROGRAM_ELF,
    ));
    #[cfg(feature = "fake_proofs")]
    let board_game_prover = Arc::new(TxExecutorTestProver::new(BoardGameExecutor::default()));

    handler
        .build_module::<AutoProver<BoardGameExecutor>>(
            AutoProverCtx {
                data_directory: config.data_directory.clone(),
                start_height: BlockHeight(0),
                prover: board_game_prover,
                contract_name: "board_game".into(),
                node: client.clone(),
            }
            .into(),
        )
        .await?;

    #[cfg(not(feature = "fake_proofs"))]
    let crash_game_prover = Arc::new(client_sdk::helpers::SP1Prover::new(
        contracts::CRASH_GAME_ELF,
    ));
    #[cfg(feature = "fake_proofs")]
    let crash_game_prover = Arc::new(TxExecutorTestProver::new(CrashGameExecutor::default()));

    handler
        .build_module::<AutoProver<BoardGameExecutor>>(
            AutoProverCtx {
                data_directory: config.data_directory.clone(),
                start_height: BlockHeight(0),
                prover: crash_game_prover,
                contract_name: "crash_game".into(),
                node: client,
            }
            .into(),
        )
        .await?;
    tracing::info!("Starting modules");

    // Run forever
    handler.start_modules().await?;
    handler.exit_process().await?;

    Ok(())
}
