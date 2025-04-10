use board_game_engine::{
    game::{GameEvent, GameState},
    GameActionBlob,
};
use hyle_contract_sdk::{ContractName, Identity, StructuredBlobData};
use hyle_model::BlobTransaction;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct GameStateManager {
    state: Option<GameState>,
}

#[wasm_bindgen]
impl GameStateManager {
    #[wasm_bindgen]
    pub fn initialize(&mut self, player_count: u32, board_size: u32) -> Result<JsValue, JsError> {
        let new_state = GameState::new(
            player_count as usize,
            board_size as usize,
            7, // Fixed seed for now, could be made configurable
        );
        self.state = Some(new_state.clone());

        // Convert state to JS value
        Ok(serde_wasm_bindgen::to_value(&new_state)?)
    }

    #[wasm_bindgen]
    pub fn get_state(&self) -> Result<JsValue, JsError> {
        match &self.state {
            Some(state) => Ok(serde_wasm_bindgen::to_value(state)?),
            None => Err(JsError::new("Game not initialized")),
        }
    }

    #[wasm_bindgen]
    pub fn process_transaction(&mut self, tx_data: JsValue) -> Result<JsValue, JsError> {
        let tx: BlobTransaction = serde_wasm_bindgen::from_value(tx_data)?;
        let mut events = Vec::new();

        for blob in &tx.blobs {
            if blob.contract_name != ContractName::from("board_game") {
                continue;
            }

            // Parse blob as GameActionBlob
            if let Ok(StructuredBlobData::<GameActionBlob> { parameters, .. }) =
                StructuredBlobData::<GameActionBlob>::try_from(blob.data.clone())
            {
                let new_events = self
                    .apply_action(&tx.identity, &parameters)
                    .map_err(|e| JsError::new(&e.to_string()))?;
                events.extend(new_events);
            }
        }

        // Return both updated state and events
        #[derive(Serialize)]
        struct StateUpdate {
            state: Option<GameState>,
            events: Vec<GameEvent>,
        }

        let update = StateUpdate {
            state: self.state.clone(),
            events,
        };

        Ok(serde_wasm_bindgen::to_value(&update)?)
    }

    fn apply_action(
        &mut self,
        caller: &Identity,
        blob: &GameActionBlob,
    ) -> anyhow::Result<Vec<GameEvent>> {
        match &mut self.state {
            Some(state) => {
                let events = state.process_action(caller, blob.0, blob.1.clone())?;
                Ok(events)
            }
            None => Err(anyhow::anyhow!("Game not initialized")),
        }
    }
}

// Initialize panic hook for better error messages
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}
