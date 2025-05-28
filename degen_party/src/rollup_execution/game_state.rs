use anyhow::{bail, Result};
use board_game::{
    game::{GameAction as BoardGameAction, GameEvent, GamePhase, GameState},
    GameActionBlob,
};
use crash_game::ChainActionBlob;
use hyle_modules::{bus::BusClientSender, modules::websocket::WsBroadcastMessage};
use sdk::{
    verifiers::Secp256k1Blob, Blob, BlobIndex, BlobTransaction, ContractAction, ContractName,
    Identity,
};
use secp256k1::Message;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fmt::Debug, ops::Deref, vec};

use crate::{proving::BoardGameExecutor, OutboundWebsocketMessage};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum GameStateCommand {
    SubmitAction { action: BoardGameAction },
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

impl super::RollupExecutor {
    pub(super) fn get_board_game(&self) -> &board_game::game::GameState {
        &self
            .contracts
            .get(&self.board_game)
            .expect("Board game not initialized")
            .deref()
            .as_any()
            .downcast_ref::<BoardGameExecutor>()
            .expect("Board game state is not of the expected type")
            .state
    }

    pub(super) async fn handle_user_message(
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
        action: BoardGameAction,
        identity: Identity,
        uuid: &str,
        identity_blobs: Vec<Blob>,
    ) -> Result<()> {
        let mut blobs = vec![];

        let uuid_128: u128 = uuid::Uuid::parse_str(uuid)?.as_u128();

        tracing::warn!("Handling action: {:?}", action);

        match &action {
            BoardGameAction::EndMinigame { result: _ } => {
                bail!("EndMinigame cannot be called directly");
            }
            BoardGameAction::StartMinigame { .. } => {
                match &self.get_board_game().phase {
                    GamePhase::StartMinigame(minigame_type)
                    | GamePhase::FinalMinigame(minigame_type) => {
                        if minigame_type != &self.crash_game {
                            bail!("Not the right minigame");
                        }
                        tracing::warn!("Starting minigame: {:?}", minigame_type);
                        // TODO ensure we are synchronized correctly.
                        blobs.push(
                            GameActionBlob(
                                uuid_128,
                                BoardGameAction::StartMinigame {
                                    minigame: minigame_type.clone(),
                                    players: self.get_board_game().get_minigame_setup(),
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
                                    players: self.get_board_game().get_minigame_setup(),
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
            BoardGameAction::EndGame => {
                let tx = self.create_backend_tx(action.clone())?;
                self.bus.send(tx)?;
                return Ok(());
            }
            BoardGameAction::Initialize { .. } => {
                blobs.push(
                    GameActionBlob(
                        uuid_128,
                        BoardGameAction::Initialize {
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

    fn create_backend_tx(&self, action: BoardGameAction) -> Result<BlobTransaction> {
        let identity = Identity::new(format!("{}@secp256k1", self.crypto.public_key));
        let uuid = uuid::Uuid::new_v4();
        let data = format!(
            "{}:{}",
            uuid,
            match action {
                BoardGameAction::EndGame => "EndGame",
                BoardGameAction::SpinWheel => "SpinWheel",
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

    pub(super) async fn board_game_on_tick(&mut self) -> Result<()> {
        let state = self.get_board_game();
        if state.phase == GamePhase::Betting {
            let likely_timed_out = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
                > state.round_started_at + 40 * 1000;
            if likely_timed_out {
                let tx = self.create_backend_tx(BoardGameAction::SpinWheel)?;
                self.bus.send(tx)?;
            }
        }
        Ok(())
    }

    async fn handle_send_state(&mut self) -> Result<()> {
        let state = self.get_board_game();
        self.bus.send(WsBroadcastMessage {
            message: OutboundWebsocketMessage::GameStateEvent(GameStateEvent::StateUpdated {
                state: Some(state.clone()),
                events: vec![],
                board_game: self.board_game.clone(),
                crash_game: self.crash_game.clone(),
            }),
        })?;
        Ok(())
    }
}
