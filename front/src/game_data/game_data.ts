import { reactive } from "vue";
import { BaseWebSocketService } from "../utils/base-websocket";
import { authService } from "./auth";

export interface MinigameResult {
    contract_name: string;
    player_results: Array<PlayerMinigameResult>;
}

export interface PlayerMinigameResult {
    player_id: string;
    coins_delta: number;
    stars_delta: number;
}

export interface CrashGameChainEvent {
    MinigameEnded?: {
        final_results: Array<[bigint, number]>;
    };
}

export type Space = "Blue" | "Red" | "Event" | "MinigameSpace" | "Star" | "Finish";

export interface Board {
    spaces: Space[];
    size: number;
}

export interface Player {
    id: string;
    name: string;
    position: number;
    coins: number;
    stars: number;
    used_uuids: Array<bigint>;
}

export type GamePhase =
    | "Registration"
    | "Rolling"
    | "Moving"
    | { MinigameStart: string }
    | { MinigamePlay: string }
    | "TurnEnd"
    | "GameOver";

export type GameAction =
    | { EndGame: null }
    | {
          Initialize: {
              player_count: number;
              board_size: number;
              minigames: string[];
              random_seed: number;
          };
      }
    | { RegisterPlayer: { name: string; identity: string } }
    | { StartGame: null }
    | { RollDice: null }
    | { StartMinigame: null }
    | { EndMinigame: { result: MinigameResult } }
    | { EndTurn: null };

export type GameEvent =
    | { DiceRolled: { player_id: string; value: number } }
    | { PlayerMoved: { player_id: string; new_position: number } }
    | { CoinsChanged: { player_id: string; amount: number } }
    | { StarsChanged: { player_id: string; amount: number } }
    | { MinigameReady: { minigame_type: string } }
    | { MinigameStarted: { minigame_type: string } }
    | { MinigameEnded: { result: MinigameResult } }
    | { TurnEnded: { next_player: string } }
    | { GameEnded: { winner_id: string; final_stars: number; final_coins: number } }
    | { GameInitialized: { player_count: number; board_size: number; random_seed: number } }
    | { PlayerRegistered: { name: string; player_id: string } }
    | { GameStarted: null };

export type GameStateCommand =
    | {
          type: "SubmitAction";
          payload: { action: GameAction };
      }
    | {
          type: "SendState";
          payload: null;
      };

export type GameStateEvent =
    | {
          type: "StateUpdated";
          payload: { state: GameState | null; events: GameEvent[] };
      }
    | {
          type: "MinigameStarted";
          payload: { minigame_type: string };
      }
    | {
          type: "MinigameEnded";
          payload: { result: MinigameResult };
      };

export interface GameState {
    players: Player[];
    current_turn: number;
    board: Board;
    phase: GamePhase;
    max_players: number;
    minigames: string[];
    dice: { min: number; max: number; seed: number };
    backend_identity: string;
    last_interaction_time: bigint;
    lane_id: string;
}

export const gameState = reactive({
    game: null as GameState | null,
    running_minigame: null as string | null,
    isInLobby: true,
});

class BoardGameService extends BaseWebSocketService {
    onStateUpdated: ((state: { state: GameState | null; events: GameEvent[] }) => void) | null = null;

    constructor() {
        super();
        this.send(
            {
                type: "GameState",
                payload: {
                    type: "SendState",
                    payload: null,
                },
            },
            "SendState",
        );
    }

    protected override onMessage(data: any) {
        if (data.type === "GameStateEvent") {
            const event = data.payload;
            if (event.type === "StateUpdated") {
                gameState.game = event.payload.state;
                if (gameState.game?.players.length === 0) gameState.isInLobby = true;
                else if (
                    gameState.isInLobby &&
                    gameState.game?.phase !== "Registration" &&
                    gameState.game?.phase !== "GameOver"
                )
                    gameState.isInLobby = false;
                if (this.onStateUpdated) {
                    this.onStateUpdated(event.payload);
                }
                for (const e of event.payload.events) {
                    if (e instanceof Object && "MinigameReady" in e) {
                        this.send(
                            {
                                type: "GameState",
                                payload: {
                                    type: "SubmitAction",
                                    payload: {
                                        action: {
                                            StartMinigame: null,
                                        },
                                    },
                                },
                            },
                            "StartMinigame",
                        );
                    } else if (e instanceof Object && "MinigameStarted" in e) {
                        gameState.running_minigame = e.MinigameStarted.minigame_type;
                        console.log("Minigame started", e.MinigameStarted.minigame_type);
                    } else if (e instanceof Object && "MinigameEnded" in e) {
                        gameState.running_minigame = null;
                        console.log("Minigame ended", e.MinigameEnded.result);
                    }
                }
            }
        }
    }

    async sendAction(action: GameAction) {
        if (!gameState.game) return;
        await this.send(
            {
                type: "GameState",
                payload: {
                    type: "SubmitAction",
                    payload: { action },
                },
            },
            `${Object.keys(action)[0]}`,
        );
    }

    async initGame(config: { playerCount: number; boardSize: number }) {
        await this.send(
            {
                type: "GameState",
                payload: {
                    type: "SubmitAction",
                    payload: {
                        action: {
                            Initialize: {
                                player_count: +config.playerCount,
                                board_size: +config.boardSize,
                                minigames: [], // will be overwritten by the server
                                random_seed: 7, // will be overwritten by the server
                            },
                        },
                    },
                },
            },
            "Initialize",
        );
    }

    async registerPlayer(name: string) {
        await this.send(
            {
                type: "GameState",
                payload: {
                    type: "SubmitAction",
                    payload: {
                        action: {
                            RegisterPlayer: {
                                name,
                                identity: getLocalPlayerId(),
                            },
                        },
                    },
                },
            },
            "RegisterPlayer",
        );
    }

    async startGame() {
        await this.send(
            {
                type: "GameState",
                payload: {
                    type: "SubmitAction",
                    payload: {
                        action: { StartGame: null },
                    },
                },
            },
            "StartGame",
        );
    }

    async reset() {
        await this.send(
            {
                type: "GameState",
                payload: {
                    type: "SubmitAction",
                    payload: {
                        action: { EndGame: null },
                    },
                },
            },
            "EndGame",
        );
    }

    async send(message: { type: "GameState"; payload: GameStateCommand }, data_to_sign: string = "") {
        await super.send(message, data_to_sign);
    }
}

export const boardGameService = new BoardGameService();
boardGameService.connect().catch(console.error);

export function getLocalPlayerId(): string {
    return `${authService.getSessionKey() || authService.generateSessionKey()}@secp256k1`;
}

export function isCurrentPlayer(id: string): boolean {
    if (!gameState.game) return false;
    return id === gameState.game.players[gameState.game.current_turn % gameState.game.players.length]?.id;
}

export function playerColor(id: string): string {
    if (!gameState.game) return "#000000";
    const player = gameState.game.players.find((p) => p.id === id);
    const colors = ["#E52521", "#00A651", "#F699CD", "#009BDE"];
    return player ? colors[gameState.game.players.indexOf(player) % colors.length] : "#000000";
}

export function playerAvatar(id: string): string {
    if (!gameState.game) return "❓";
    const player = gameState.game.players.find((p) => p.id === id);
    if (!player) return "❓";
    const avatars = ["👨‍🔧", "🤡", "🥷", "🥶"];
    return avatars[gameState.game.players.indexOf(player) % avatars.length];
}

// Sample party game player data
export const DEFAULT_PLAYERS: Player[] = [
    {
        id: "1",
        name: "Mario",
        coins: 87,
        position: 23,
        stars: 1,
        used_uuids: [],
    },
    {
        id: "2",
        name: "Luigi",
        coins: 64,
        position: 18,
        stars: 2,
        used_uuids: [],
    },
    {
        id: "3",
        name: "Peach",
        coins: 103,
        position: 27,
        stars: 3,
        used_uuids: [],
    },
    {
        id: "4",
        name: "Toad",
        coins: 52,
        position: 15,
        stars: 0,
        used_uuids: [],
    },
];
