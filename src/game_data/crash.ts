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
          type: "PlaceBet";
          payload: { player_id: string; amount: number };
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

export interface CrashGameMinigameState {
    is_running: boolean;
    current_multiplier: number;
    waiting_for_start: boolean;
    active_bets: Record<string, { amount: number; cashed_out_at?: number | null }>;
    players: Record<string, { id: string; name: string; coins: number }>;
}

export interface CrashGameState {
    minigame: CrashGameMinigameState;
}

export const crashGameState = reactive({
    minigame: null as CrashGameMinigameState | null,
});

class CrashGameService extends BaseWebSocketService {
    protected override onOpen(): void {
        // No initial action needed
    }

    protected override onMessage(data: any) {
        console.log("Crash game service received data", data);
        if (data.type === "CrashGame") {
            const event = data.payload;
            if (event.type === "StateUpdated") {
                const state = event.payload.state;
                if (state) {
                    console.log("Crash game state updated", state.current_minigame);
                    crashGameState.minigame = state.minigame;
                } else {
                    console.log("Crash game state cleared");
                }
                for (const chainEvent of event.payload.events) {
                    if (chainEvent.BetPlaced) {
                        console.log("Bet placed:", chainEvent.BetPlaced);
                    }
                    if (chainEvent.PlayerCashedOut) {
                        console.log("Player cashed out:", chainEvent.PlayerCashedOut);
                    }
                }
            }
        }
    }

    placeBet(amount: number) {
        this.send({
            type: "CrashGame",
            payload: {
                type: "PlaceBet",
                payload: {
                    player_id: getLocalPlayerId(),
                    amount,
                },
            },
        });
    }

    cashOut() {
        this.send({
            type: "CrashGame",
            payload: {
                type: "CashOut",
                payload: {
                    player_id: getLocalPlayerId(),
                },
            },
        });
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
