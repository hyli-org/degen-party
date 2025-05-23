use crate::{
    crash_game::CrashGameCommand, fake_lane_manager::ConfirmedBlobTransaction,
    AuthenticatedMessage, CryptoContext, InboundWebsocketMessage, OutboundWebsocketMessage,
};
use anyhow::{bail, Context, Result};
use board_game::{
    game::{GameAction, GameEvent, GamePhase, GameState},
    GameActionBlob,
};
use client_sdk::rest_client::NodeApiClient;
use crash_game::ChainActionBlob;
use hyle_modules::{
    bus::{command_response::Query, BusClientSender, SharedMessageBus},
    module_bus_client, module_handle_messages,
    modules::{
        websocket::{WsBroadcastMessage, WsInMessage},
        Module,
    },
};
use sdk::{verifiers::Secp256k1Blob, Blob, BlobIndex};
use sdk::{BlobTransaction, ContractName, Identity};
use sdk::{ContractAction, StructuredBlobData};
use secp256k1::Message;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fmt::Debug,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

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
        board_game: ContractName,
        crash_game: ContractName,
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
    sender(BlobTransaction),
    sender(WsBroadcastMessage<OutboundWebsocketMessage>),
    receiver(Query<QueryGameState, GameState>),
    receiver(GameStateCommand),
    receiver(ConfirmedBlobTransaction),
    receiver(WsInMessage<AuthenticatedMessage<InboundWebsocketMessage>>),
}
}

pub struct GameStateModule {
    bus: GameStateBusClient,
    crypto: Arc<CryptoContext>,
    board_game: ContractName,
    crash_game: ContractName,
    state: Option<GameState>,
}

impl Module for GameStateModule {
    type Context = Arc<crate::Context>;

    async fn build(bus: SharedMessageBus, ctx: Self::Context) -> Result<Self> {
        let bus = GameStateBusClient::new_from_bus(bus.new_handle()).await;

        Ok(Self {
            bus,
            crypto: ctx.crypto.clone(),
            board_game: ctx.board_game.clone(),
            crash_game: ctx.crash_game.clone(),
            state: Some(borsh::from_slice(
                &ctx.client
                    .get_contract(ctx.board_game.clone())
                    .await?
                    .state
                    .0,
            )?),
        })
    }

    async fn run(&mut self) -> Result<()> {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
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
                    identity,
                    uuid,
                    identity_blobs
                } = msg.message;
                if let InboundWebsocketMessage::GameState(event) = message {
                    if let Err(e) = self.handle_user_message(event, identity, &uuid, identity_blobs).await {
                        tracing::warn!("Error handling event: {:?}", e);
                    }
                }
            }
            listen<ConfirmedBlobTransaction> tx => {
                if let Err(err) = self.handle_tx(tx.0).await {
                    tracing::info!("Error handling transaction: {:?}", err);
                }
            }
            _ = tick.tick() => {
                self.on_tick().await?;
            }
        };

        Ok(())
    }
}

impl GameStateModule {
    async fn handle_user_message(
        &mut self,
        event: GameStateCommand,
        identity: Identity,
        uuid: &str,
        identity_blobs: Vec<Blob>,
    ) -> Result<()> {
        match event {
            GameStateCommand::SubmitAction { action } => {
                self.handle_submit_action(action, identity, uuid, identity_blobs)
                    .await
            }
            GameStateCommand::SendState => self.handle_send_state().await,
        }
    }

    async fn handle_submit_action(
        &mut self,
        action: GameAction,
        identity: Identity,
        uuid: &str,
        identity_blobs: Vec<Blob>,
    ) -> Result<()> {
        let mut blobs = vec![];

        let uuid_128: u128 = uuid::Uuid::parse_str(uuid)?.as_u128();

        tracing::warn!("Handling action: {:?}", action);

        match &action {
            GameAction::EndMinigame { result: _ } => {
                bail!("EndMinigame cannot be called directly");
            }
            GameAction::StartMinigame { .. } => {
                match &self.state.as_ref().context("Game not initialized")?.phase {
                    GamePhase::StartMinigame(minigame_type)
                    | GamePhase::FinalMinigame(minigame_type) => {
                        if minigame_type != &self.crash_game {
                            bail!("Not the right minigame");
                        }
                        let Some(state) = &self.state else {
                            bail!("Game not initialized");
                        };
                        tracing::warn!("Starting minigame: {:?}", minigame_type);
                        // TODO ensure we are synchronized correctly.
                        blobs.push(
                            GameActionBlob(
                                uuid_128,
                                GameAction::StartMinigame {
                                    minigame: minigame_type.clone(),
                                    players: state.get_minigame_setup(),
                                },
                            )
                            .as_blob(
                                self.board_game.clone(),
                                Some(BlobIndex(1)),
                                None,
                            ),
                        );
                        blobs.push(
                            ChainActionBlob(
                                uuid_128,
                                crash_game::ChainAction::InitMinigame {
                                    players: state.get_minigame_setup(),
                                },
                            )
                            .as_blob(
                                self.crash_game.clone(),
                                None,
                                Some(vec![BlobIndex(0)]),
                            ),
                        );
                    }
                    _ => {
                        bail!("Not ready to start a game");
                    }
                }
            }
            GameAction::EndGame => {
                let tx = self.create_backend_tx(action.clone())?;
                self.bus.send(tx)?;
                return Ok(());
            }
            GameAction::Initialize {
                player_count,
                minigames,
                random_seed,
            } => {
                blobs.push(
                    GameActionBlob(
                        uuid_128,
                        GameAction::Initialize {
                            player_count: *player_count,
                            minigames: vec![self.crash_game.clone().0],
                            random_seed: uuid_128 as u64,
                        },
                    )
                    .as_blob(self.board_game.clone(), None, None),
                );
            }
            _ => {
                blobs.push(GameActionBlob(uuid_128, action.clone()).as_blob(
                    self.board_game.clone(),
                    None,
                    None,
                ));
            }
        }

        blobs.extend(identity_blobs);

        // Add identity blob
        let tx = BlobTransaction::new(identity, blobs);
        self.bus.send(tx)?;

        // The state will be updated when we receive the transaction confirmation
        // through the InboundTxMessage receiver

        Ok(())
    }

    fn create_backend_tx(&self, action: GameAction) -> Result<BlobTransaction> {
        let identity = Identity::new(format!("{}@secp256k1", self.crypto.public_key));
        let uuid = uuid::Uuid::new_v4();
        let data = format!(
            "{}:{}",
            uuid,
            match action {
                GameAction::EndGame => "EndGame",
                GameAction::SpinWheel => "SpinWheel",
                _ => unreachable!(),
            }
        )
        .as_bytes()
        .to_vec();
        let mut hasher = Sha256::new();
        hasher.update(data.clone());
        let message_hash: [u8; 32] = hasher.finalize().into();
        let signature = self
            .crypto
            .secp
            .sign_ecdsa(Message::from_digest(message_hash), &self.crypto.secret_key);
        Ok(BlobTransaction::new(
            identity.clone(),
            vec![
                Secp256k1Blob::new(
                    identity,
                    &data,
                    &self.crypto.public_key.to_string(),
                    &signature.to_string(),
                )?
                .as_blob(),
                GameActionBlob(uuid.as_u128(), action).as_blob(self.board_game.clone(), None, None),
            ],
        ))
    }

    async fn on_tick(&mut self) -> Result<()> {
        if let Some(state) = &self.state {
            if state.phase == GamePhase::Betting {
                let likely_timed_out = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
                    > state.round_started_at + 40 * 1000;
                if likely_timed_out {
                    let tx = self.create_backend_tx(GameAction::SpinWheel)?;
                    self.bus.send(tx)?;
                }
            }
        }
        Ok(())
    }

    async fn handle_send_state(&mut self) -> Result<()> {
        if let Some(state) = &self.state {
            self.bus.send(WsBroadcastMessage {
                message: OutboundWebsocketMessage::GameStateEvent(GameStateEvent::StateUpdated {
                    state: Some(state.clone()),
                    events: vec![],
                    board_game: self.board_game.clone(),
                    crash_game: self.crash_game.clone(),
                }),
            })?;
        }
        Ok(())
    }

    async fn handle_tx(&mut self, tx: BlobTransaction) -> Result<()> {
        // Transaction confirmed, now we can update the state
        for blob in &tx.blobs {
            if blob.contract_name != self.board_game.clone() {
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
                            board_game: self.board_game.clone(),
                            crash_game: self.crash_game.clone(),
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
        // Apply action optimistically to local state.
        // For the most part this is very safe as we are operating in rollup mode,
        // but the timestamp is very optimistic.
        match &mut self.state {
            Some(state) => {
                let time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
                let events = state.process_action(caller, blob.0, blob.1.clone(), time)?;
                state.last_interaction_time = time;
                tracing::debug!("Applied action {:?}, got events: {:?}", blob.1, events);
                Ok(events)
            }
            None => Err(anyhow::anyhow!("Game not initialized")),
        }
    }
}
