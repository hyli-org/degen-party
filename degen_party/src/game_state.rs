use crate::{
    crash_game::CrashGameCommand, AuthenticatedMessage, InboundWebsocketMessage,
    OutboundWebsocketMessage,
};
use anyhow::{bail, Context, Result};
use crash_game::ChainActionBlob;
use hyle_modules::{
    bus::{command_response::Query, BusClientSender, SharedMessageBus},
    module_bus_client, module_handle_messages,
    modules::{
        websocket::{WsBroadcastMessage, WsInMessage},
        Module,
    },
};
use sdk::{verifiers::Secp256k1Blob, ZkContract};
use sdk::{BlobTransaction, ContractName, Identity};
use sdk::{ContractAction, StructuredBlobData};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, sync::Arc, time::Duration};
use tokio::time::sleep;
use board_game::{
    game::{GameAction, GameEvent, GamePhase, GameState},
    GameActionBlob,
};

use crate::fake_lane_manager::InboundTxMessage;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum GameStateCommand {
    SubmitAction { action: GameAction },
    SendState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum GameStateEvent {
    StateUpdated {
        state: Option<GameState>,
        events: Vec<GameEvent>,
    },
    MinigameEnded {
        contract_name: ContractName,
        final_results: Vec<(Identity, i32)>,
    },
}

#[derive(Clone)]
pub struct QueryGameState;

module_bus_client! {
pub struct GameStateBusClient {
    sender(GameStateCommand),
    sender(CrashGameCommand),
    sender(InboundTxMessage),
    sender(WsBroadcastMessage<OutboundWebsocketMessage>),
    receiver(Query<QueryGameState, GameState>),
    receiver(GameStateCommand),
    receiver(InboundTxMessage),
    receiver(WsInMessage<AuthenticatedMessage<InboundWebsocketMessage>>),
}
}

pub struct GameStateModule {
    bus: GameStateBusClient,
    state: Option<GameState>,
}

impl Module for GameStateModule {
    type Context = Arc<crate::Context>;

    async fn build(bus: SharedMessageBus, _ctx: Self::Context) -> Result<Self> {
        let bus = GameStateBusClient::new_from_bus(bus.new_handle()).await;
        Ok(Self { bus, state: None })
    }

    async fn run(&mut self) -> Result<()> {
        self.bus.send(InboundTxMessage::RegisterContract((
            ContractName::from("board_game"),
            GameState::default().commit(),
        )))?;

        // Wait for the contract to be registered.
        // TODO: fix this
        sleep(Duration::from_secs(1)).await;
        self.state = Some(GameState::default());

        module_handle_messages! {
            on_bus self.bus,
            command_response<QueryGameState, GameState> _ => {
                match &self.state {
                    Some(state) => Ok(state.clone()),
                    None => Err(anyhow::anyhow!("Game not initialized"))
                }
            }
            listen<WsInMessage<AuthenticatedMessage<InboundWebsocketMessage>>> msg => {
                let AuthenticatedMessage {
                    message,
                    signature,
                    public_key,
                    message_id,
                    signed_data,
                } = msg.message;
                if let InboundWebsocketMessage::GameState(event) = message {
                    if let Err(e) = self.handle_user_message(event, signature, public_key, message_id, signed_data).await {
                        tracing::warn!("Error handling event: {:?}", e);
                    }
                }
            }
            listen<InboundTxMessage> msg => {
                if let InboundTxMessage::NewTransaction(tx) = msg {
                    self.handle_tx(tx).await?;
                }
            }
        };

        Ok(())
    }
}

impl GameStateModule {
    async fn handle_user_message(
        &mut self,
        event: GameStateCommand,
        signature: String,
        public_key: String,
        message_id: String,
        signed_data: String,
    ) -> Result<()> {
        match event {
            GameStateCommand::SubmitAction { action } => {
                self.handle_submit_action(action, signature, public_key, message_id, signed_data)
                    .await
            }
            GameStateCommand::SendState => self.handle_send_state().await,
        }
    }

    async fn handle_submit_action(
        &mut self,
        action: GameAction,
        signature: String,
        public_key: String,
        message_id: String,
        signed_data: String,
    ) -> Result<()> {
        let mut blobs = vec![];

        let uuid_128: u128 = uuid::Uuid::parse_str(&message_id)?.as_u128();

        match &action {
            GameAction::StartMinigame => {
                let players: Vec<(Identity, String, Option<u64>)> = self
                    .state
                    .as_ref()
                    .context("Game not initialized")?
                    .players
                    .iter()
                    .map(|p| (p.id.clone(), p.name.clone(), Some(p.coins as u64)))
                    .collect();
                // TODO: conceptually, we should perhaps skip this one ?
                blobs.push(GameActionBlob(uuid_128, action.clone()).as_blob(
                    "board_game".into(),
                    None,
                    None,
                ));
                // TODO: we should make sure that our current state is synchronised
                if let GamePhase::MinigameStart(minigame_type) =
                    &self.state.as_ref().context("Game not initialized")?.phase
                {
                    if minigame_type.0 == "crash_game" {
                        blobs.push(
                            ChainActionBlob(
                                uuid_128,
                                crash_game::ChainAction::InitMinigame { players },
                            )
                            .as_blob("crash_game".into(), None, None),
                        );
                    } else {
                        bail!("Unsupported minigame type: {}", minigame_type);
                    }
                } else {
                    bail!("Not ready to start a game");
                }
            }
            GameAction::EndMinigame { result: _ } => {}
            _ => {
                // Submit the action as a blob
                // With a random UUID to avoid hash collisions
                blobs.push(GameActionBlob(uuid_128, action.clone()).as_blob(
                    "board_game".into(),
                    None,
                    None,
                ));
            }
        }

        // Add identity blob
        let identity = format!("{public_key}@secp256k1");
        blobs.push(
            Secp256k1Blob::new(
                Identity::from(identity.clone()),
                signed_data.as_bytes(),
                &public_key,
                &signature,
            )?
            .as_blob(),
        );
        let tx = BlobTransaction::new(identity, blobs);
        self.bus.send(InboundTxMessage::NewTransaction(tx))?;

        // The state will be updated when we receive the transaction confirmation
        // through the InboundTxMessage receiver

        Ok(())
    }

    async fn handle_send_state(&mut self) -> Result<()> {
        if let Some(state) = &self.state {
            self.bus.send(WsBroadcastMessage {
                message: OutboundWebsocketMessage::GameStateEvent(GameStateEvent::StateUpdated {
                    state: Some(state.clone()),
                    events: vec![],
                }),
            })?;
        }
        Ok(())
    }

    async fn handle_tx(&mut self, tx: BlobTransaction) -> Result<()> {
        // Transaction confirmed, now we can update the state
        for blob in &tx.blobs {
            if blob.contract_name != ContractName::from("board_game") {
                continue;
            }

            // parse as structured blob of gameactionblob
            let t = StructuredBlobData::<GameActionBlob>::try_from(blob.data.clone());
            if let Ok(StructuredBlobData::<GameActionBlob> { parameters, .. }) = t {
                tracing::debug!("Received blob: {:?}", parameters);
                let events = self.apply_action(&tx.identity, &parameters).await?;
                self.bus.send(WsBroadcastMessage {
                    message: OutboundWebsocketMessage::GameStateEvent(
                        GameStateEvent::StateUpdated {
                            state: Some(self.state.clone().unwrap()),
                            events,
                        },
                    ),
                })?;
            } else {
                tracing::warn!("Failed to parse blob as GameActionBlob");
            }
        }
        Ok(())
    }

    async fn apply_action(
        &mut self,
        caller: &Identity,
        blob: &GameActionBlob,
    ) -> Result<Vec<GameEvent>> {
        // Apply action optimistically to local state
        match &mut self.state {
            Some(state) => {
                let events = state.process_action(caller, blob.0, blob.1.clone())?;
                tracing::debug!("Applied action {:?}, got events: {:?}", blob.1, events);
                Ok(events)
            }
            None => Err(anyhow::anyhow!("Game not initialized")),
        }
    }
}
