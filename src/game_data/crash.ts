import { reactive } from "vue";
import { BaseWebSocketService } from "../utils/base-websocket";
import { boardGameService, gameState } from "./game_data";

export interface ChainEvent {
    MinigameEnded?: {
        final_results: Array<[bigint, number]>;
    };
}

export type CrashGameCommand =
    | {
          type: "Initialize";
          payload: { players: Array<[bigint, string]> };
      }
    | {
          type: "PlaceBet";
          payload: { player_id: bigint; amount: number };
      }
    | {
          type: "CashOut";
          payload: { player_id: bigint };
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
}

export interface CrashGameState {
    minigame: CrashGameMinigameState | null;
}

export const crashGameState = reactive({
    minigame: {
        is_running: false,
        current_multiplier: 1,
        waiting_for_start: false,
        active_bets: {},
    } as CrashGameMinigameState | null,
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
                    crashGameState.minigame = state.current_minigame;
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
        if (!gameState.playerId) return;
        // TODO: Let players handle this
        const players = gameState.game?.players.length || 0;
        this.send({
            type: "CrashGame",
            payload: {
                type: "PlaceBet",
                payload: {
                    player_id: gameState.playerId,
                    amount,
                },
            },
        });
        // temp hack
        /*
        for (let i = 0; i < players; i++) {
            this.send({
                type: "CrashGame",
                payload: {
                    type: "PlaceBet",
                    payload: {
                        player_id: gameState.game?.players[i].id,
                        amount,
                    },
                },
            });
        }*/
    }
    cashOut() {
        if (!gameState.playerId) return;
        this.send({
            type: "CrashGame",
            payload: {
                type: "CashOut",
                payload: {
                    player_id: gameState.playerId,
                },
            },
        });
    }
    returnToBoard() {
        this.send({
            type: "CrashGame",
            payload: {
                type: "End",
                payload: null,
            },
        });
    }
}

export const crashGameService = new CrashGameService();
