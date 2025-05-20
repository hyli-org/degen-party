import { reactive } from "vue";
import { BaseWebSocketService } from "../utils/base-websocket";
import { boardGameService, gameState, getLocalPlayerId } from "./game_data";
import { authService } from "./auth";

export interface ChainEvent {
    MinigameEnded?: {
        final_results: Array<[string, number]>;
    };
}

export type CrashGameCommand =
    | {
          type: "Initialize";
          payload: { players: Array<[string, string]> };
      }
    | {
          type: "CashOut";
          payload: { player_id: string };
      }
    | {
          type: "Start";
          payload: null;
      }
    | {
          type: "End";
          payload: null;
      };

export type CrashGameEvent = {
    type: "StateUpdated";
    payload: { state: CrashGameState | null; events: ChainEvent[] };
};

export interface CrashGameMinigameVerifiableState {
    state: "WaitingForStart" | "Running" | "Crashed";
    players: Record<string, { id: string; name: string; bet: number; cashed_out_at?: number }>;
}

export interface CrashGameMinigameBackendState {
    current_multiplier: number;
    game_setup_time: number | null;
    game_start_time: number | null;
    current_time: number | null;
}

export interface CrashGameState {
    minigame_verifiable: CrashGameMinigameVerifiableState;
    minigame_backend: CrashGameMinigameBackendState;
}

export const crashGameState = reactive({
    minigame_verifiable: null as CrashGameMinigameVerifiableState | null,
    minigame_backend: null as CrashGameMinigameBackendState | null,
});

class CrashGameService extends BaseWebSocketService {
    protected override onMessage(data: any) {
        console.log("Crash game service received data", data);
        if (data.type === "CrashGame") {
            const event = data.payload;
            if (event.type === "StateUpdated") {
                const state = event.payload.state;
                if (state) {
                    console.log("Crash game state updated", state.current_minigame);
                    crashGameState.minigame_verifiable = state.minigame_verifiable;
                    crashGameState.minigame_backend = state.minigame_backend;
                } else {
                    console.log("Crash game state cleared");
                }
            }
        }
    }

    cashOut() {
        this.send(
            {
                type: "CrashGame",
                payload: {
                    type: "CashOut",
                    payload: {
                        player_id: getLocalPlayerId(),
                    },
                },
            },
            "CashOut",
        );
    }

    returnToBoard() {
        this.send(
            {
                type: "CrashGame",
                payload: {
                    type: "End",
                    payload: null,
                },
            },
            "EndMinigame",
        );
    }

    async send(message: any, data_to_sign: string = "") {
        await super.send(message, data_to_sign);
    }
}

export const crashGameService = new CrashGameService();
