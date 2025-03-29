import { reactive } from "vue";
import { BaseWebSocketService } from "../utils/base-websocket";

export interface MinigameResult {
    contract_name: string;
    player_results: Array<{
        player_id: bigint;
        coins_delta: number;
        stars_delta: number;
    }>;
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
}

export type GamePhase =
    | "Registration"
    | "Rolling"
    | "Moving"
    | "SpaceEffect"
    | "MinigameStart"
    | "MinigamePlay"
    | "TurnEnd"
    | "GameOver";

export type GameAction =
    | { RegisterPlayer: { name: string } }
    | { StartGame: null }
    | { RollDice: null }
    | { MovePlayer: { player_id: string; spaces: number } }
    | { ApplySpaceEffect: { player_id: string } }
    | { StartMinigame: { minigame_type: string } }
    | {
          EndMinigame: {
              result: {
                  contract_name: string;
                  player_results: { player_id: bigint; coins_delta: number; stars_delta: number }[];
              };
          };
      }
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
    | { PlayerRegistered: { name: string; player_id: string } }
    | "GameStarted";

export interface GameState {
    id: string;
    players: Player[];
    current_turn: number;
    board: Board;
    phase: GamePhase;
    max_players: number;
}

export const gameState = reactive({
    game: null as GameState | null,
    isInLobby: true,
    playerId: "1",
    playerName: "Player 1",
});

export type GameStateEvent =
    | {
          type: "ActionSubmitted";
          payload: { action: GameAction };
      }
    | {
          type: "Reset";
          payload: null;
      }
    | {
          type: "Initialize";
          payload: { player_count: number; board_size: number };
      }
    | {
          type: "SendState";
          payload: null;
      }
    | {
          type: "StateUpdated";
          payload: { state: GameState | null; events: GameEvent[] };
      }
    | {
          type: "MinigameStarted";
          payload: { contract_name: string };
      }
    | {
          type: "MinigameEnded";
          payload: { result: MinigameResult };
      };

class BoardGameService extends BaseWebSocketService {
    onStateUpdated: ((state: { state: GameState | null; events: GameEvent[] }) => void) | null = null;

    protected override onOpen(): void {}

    protected override onMessage(data: any) {
        if (data.type === "GameState") {
            const event = data.payload;
            if (event.type === "StateUpdated") {
                gameState.game = event.payload.state;
                gameState.isInLobby = !gameState.game || gameState.game.phase === "Registration";
                if (this.onStateUpdated) {
                    this.onStateUpdated(event.payload);
                }
                for (const e of event.payload.events) {
                    if (e instanceof Object && "PlayerRegistered" in e) {
                        if (e.PlayerRegistered.name === gameState.playerName) {
                            console.log("Registered as player", e.PlayerRegistered.player_id);
                            gameState.playerId = `${e.PlayerRegistered.player_id}`;
                        }
                    } else if (e instanceof Object && "MinigameReady" in e) {
                        this.send({
                            type: "GameState",
                            payload: {
                                type: "ActionSubmitted",
                                payload: {
                                    action: {
                                        StartMinigame: {
                                            minigame_type: e.MinigameReady.minigame_type,
                                        },
                                    },
                                },
                            },
                        });
                    }
                }
            } else if (event.type === "MinigameStarted") {
                console.log("Minigame started", event.payload.contract_name);
            }
        }
    }

    async sendAction(action: GameAction) {
        if (!gameState.game) return;
        await this.send({
            type: "GameState",
            payload: {
                type: "ActionSubmitted",
                payload: { action },
            },
        });
    }

    async reset() {
        await this.send({
            type: "GameState",
            payload: {
                type: "Reset",
                payload: null,
            },
        });
    }

    async initGame(config: { playerCount: number; boardSize: number }) {
        await this.send({
            type: "GameState",
            payload: {
                type: "Initialize",
                payload: {
                    player_count: +config.playerCount,
                    board_size: +config.boardSize,
                },
            },
        });
    }

    async registerPlayer(name: string) {
        await this.send({
            type: "GameState",
            payload: {
                type: "ActionSubmitted",
                payload: {
                    action: { RegisterPlayer: { name } },
                },
            },
        });
    }

    async startGame() {
        await this.send({
            type: "GameState",
            payload: {
                type: "ActionSubmitted",
                payload: {
                    action: { StartGame: null },
                },
            },
        });
    }
}

export const boardGameService = new BoardGameService();
boardGameService.connect().catch(console.error);

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
    },
    {
        id: "2",
        name: "Luigi",
        coins: 64,
        position: 18,
        stars: 2,
    },
    {
        id: "3",
        name: "Peach",
        coins: 103,
        position: 27,
        stars: 3,
    },
    {
        id: "4",
        name: "Toad",
        coins: 52,
        position: 15,
        stars: 0,
    },
];
