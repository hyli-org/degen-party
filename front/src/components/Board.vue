<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import type { GameState, Space, Player, GameEvent, GamePhase } from "../game_data/game_data";
import GridBoard from "./GridBoard.vue";
import DiceModal from "./DiceModal.vue";
import { boardGameService, gameState, isCurrentPlayer, getLocalPlayerId } from "../game_data/game_data";
import { wsState } from "../utils/shared-websocket";

// Game events
const gameEvents = ref<string[]>([]);
const showDiceModal = ref(false);
const lastDiceRoll = ref<number>(0);

// Computed properties
const currentGame = computed<GameState>(() => {
    return gameState.game as GameState;
});

const isCurrentPlayersTurn = computed(() => {
    if (gameState.isInLobby) return false;
    return isCurrentPlayer(getLocalPlayerId());
});

onMounted(() => {
    boardGameService.onStateUpdated = (payload) => {
        for (const event of payload.events) {
            handleGameEvent(event);
        }
    };
});

// Add event handling
function handleGameEvent(event: GameEvent) {
    let eventText = "";
    if (event == "GameStarted") {
        eventText = "🎮 Game started!";
    } else if ("DiceRolled" in event) {
        const player = currentGame.value.players.find((p) => p.id === event.DiceRolled.player_id);
        eventText = `${player?.name || "Unknown"} rolled a ${event.DiceRolled.value}! 🎲`;
        lastDiceRoll.value = event.DiceRolled.value;
        showDiceModal.value = true;
    } else if ("PlayerMoved" in event) {
        const player = currentGame.value.players.find((p) => p.id === event.PlayerMoved.player_id);
        const space = currentGame.value.board.spaces[event.PlayerMoved.new_position];
        eventText = `${player?.name || "Unknown"} landed on ${space} space!`;
    } else if ("CoinsChanged" in event) {
        const player = currentGame.value.players.find((p) => p.id === event.CoinsChanged.player_id);
        eventText = `${player?.name || "Unknown"} ${event.CoinsChanged.amount > 0 ? "gained" : "lost"} ${Math.abs(event.CoinsChanged.amount)} coins! ${event.CoinsChanged.amount > 0 ? "🪙" : "💸"}`;
    } else if ("StarsChanged" in event) {
        const player = currentGame.value.players.find((p) => p.id === event.StarsChanged.player_id);
        eventText = `${player?.name || "Unknown"} ${event.StarsChanged.amount > 0 ? "gained" : "lost"} ${Math.abs(event.StarsChanged.amount)} stars! ⭐`;
    } else if ("MinigameStarted" in event) {
        eventText = `🎮 ${event.MinigameStarted.minigame_type} started!`;
    } else if ("TurnEnded" in event) {
        const nextPlayer = currentGame.value.players.find((p) => p.id === event.TurnEnded.next_player);
        eventText = `Next turn: ${nextPlayer?.name || "Unknown"} ▶️`;
    } else if ("GameEnded" in event) {
        const winner = currentGame.value.players.find((p) => p.id === event.GameEnded.winner_id);
        eventText = `🏆 ${winner?.name || "Unknown"} won with ${event.GameEnded.final_stars} stars and ${event.GameEnded.final_coins} coins!`;
    } else if ("PlayerRegistered" in event) {
        const player = currentGame.value.players.find((p) => p.id === event.PlayerRegistered.player_id);
        eventText = `${player?.name || "Unknown"} joined the game! 👋`;
    } else {
        eventText = JSON.stringify(event);
    }
    gameEvents.value.push(eventText);
    // Keep only the last 3 events
    if (gameEvents.value.length > 3) {
        gameEvents.value.shift();
    }
}

// Methods
function spaceTypeClass(space: Space): string {
    return (
        {
            Blue: "blue-space",
            Red: "red-space",
            Event: "event-space",
            MinigameSpace: "minigame-space",
            Star: "star-space",
        }[space] || ""
    );
}

function spaceLabel(space: Space): string {
    return (
        {
            Blue: "+3 🪙",
            Red: "-3 🪙",
            Event: "?",
            MinigameSpace: "🎮",
            Star: "⭐",
        }[space] || ""
    );
}

function getSpaceStyle(index: number) {
    const boardSize = currentGame.value.board.size;
    const angle = (index / boardSize) * 2 * Math.PI;
    const radius = 200; // Adjust based on board container size
    const x = Math.cos(angle) * radius;
    const y = Math.sin(angle) * radius;

    return {
        transform: `translate(${x}px, ${y}px)`,
    };
}

function hasPlayerOnSpace(spaceIndex: number): boolean {
    return currentGame.value.players.some((player) => player.position === spaceIndex);
}

function playersOnSpace(spaceIndex: number): Player[] {
    return currentGame.value.players.filter((player) => player.position === spaceIndex);
}

function getPlayerColor(playerId: string): string {
    const colors = ["#FF0000", "#00FF00", "#0000FF", "#FFFF00"];
    return colors[Number(playerId) % colors.length];
}

// Game actions
async function rollDice() {
    try {
        lastDiceRoll.value = 0; // Reset the value
        await boardGameService.sendAction({ RollDice: null });
        // The handleGameEvent function will receive the DiceRolled event and show the modal
    } catch (error) {
        console.error("Failed to roll dice:", error);
        showDiceModal.value = false;
    }
}

async function endTurn() {
    try {
        await boardGameService.sendAction({ EndTurn: null });
    } catch (error) {
        console.error("Failed to end turn:", error);
    }
}
</script>

<template>
    <div class="flex flex-col gap-6 w-full max-w-[1400px] mx-auto h-full p-4 relative">
        <div
            class="font-['Luckiest_Guy'] text-[4.5rem] text-[#ffd700] text-center mx-auto -mb-5 text-shadow-[_-2px_-2px_0_#e6a100,_2px_-2px_0_#e6a100,_-2px_2px_0_#e6a100,_2px_2px_0_#e6a100,_4px_4px_0_#b87d00,_6px_6px_0_#8b5e00] rotate-[-2deg] transition-transform duration-300 ease-in-out tracking-wide font-normal antialiased uppercase hover:scale-102 hover:rotate-[-2deg]"
        >
            DEGEN PARTY
        </div>

        <div
            class="relative w-full bg-[#8B0000] rounded-[30px] overflow-hidden border-8 border-[#ffa048] shadow-[inset_0_0_20px_rgba(0,0,0,0.3),_0_10px_20px_rgba(0,0,0,0.2)] before:content-[''] before:absolute before:inset-0 before:border-8 before:border-[#d35400] before:rounded-[24px] before:shadow-[inset_0_0_15px_rgba(0,0,0,0.4)] before:pointer-events-none before:z-5 after:content-[''] after:absolute after:-inset-[6px] after:bg-[repeating-linear-gradient(45deg,_#e67e22,_#e67e22_15px,_#d35400_15px,_#d35400_30px)] after:rounded-[30px] after:-z-1"
        >
            <div class="flex flex-1 p-5">
                <!-- Game Board -->
                <div class="relative flex-2 flex justify-center bg-[#8B0000] rounded-lg mr-5 overflow-auto">
                    <GridBoard
                        v-if="!gameState.isInLobby"
                        :spaces="currentGame.board.spaces"
                        :players="currentGame.players"
                        :size="currentGame.board.size"
                    />
                    <div v-else class="flex items-center justify-center w-full h-full text-[#FFC636] text-2xl">
                        Game not initialized
                    </div>
                </div>

                <!-- Game Info -->
                <div class="flex-1 flex flex-col gap-5 bg-[#8B0000] rounded-lg p-5">
                    <div class="text-2xl text-[#FFC636] text-center p-2.5 bg-[#6B0000] rounded-lg">
                        <template v-if="!wsState.connected">
                            {{ wsState.connectionStatus }}
                        </template>
                        <template v-else-if="gameState.isInLobby"> Waiting for game to start... </template>
                        <template v-else-if="currentGame.phase === 'GameOver'"> 🎉 Game Over! 🎉 </template>
                        <template v-else> Phase: {{ currentGame.phase }} </template>
                    </div>

                    <!-- Game Controls -->
                    <div class="flex gap-2.5 justify-center">
                        <template v-if="wsState.connected && !gameState.isInLobby && currentGame.phase !== 'GameOver'">
                            <button
                                v-if="currentGame.phase === 'Rolling'"
                                @click="rollDice"
                                :disabled="!isCurrentPlayersTurn"
                                class="min-w-[180px] h-[70px] text-[1.3rem] uppercase tracking-wide rounded-[15px] font-extrabold font-['Baloo_2'] relative overflow-hidden border-4 border-white shadow-[0_8px_0_rgba(0,0,0,0.3)] text-shadow-[2px_2px_0_rgba(0,0,0,0.3)] px-6 py-1 cursor-pointer transition-all duration-200 ease-in-out hover:-translate-y-[3px] hover:shadow-[0_11px_0_rgba(0,0,0,0.3)] active:translate-y-1 active:shadow-[0_4px_0_rgba(0,0,0,0.3)] disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:transform-none disabled:hover:shadow-[0_8px_0_rgba(0,0,0,0.3)] disabled:active:transform-none disabled:active:shadow-[0_8px_0_rgba(0,0,0,0.3)]"
                                :class="{
                                    'bg-gradient-to-b from-[#FF4D4D] to-[#CC0000]': isCurrentPlayersTurn,
                                    'bg-gradient-to-b from-[#999999] to-[#666666]': !isCurrentPlayersTurn,
                                }"
                            >
                                <span
                                    class="flex items-center justify-center gap-2"
                                    :class="{ 'opacity-50': !isCurrentPlayersTurn }"
                                >
                                    <span
                                        class="text-[1.4rem]"
                                        :class="{ 'animate-[wiggle_1s_infinite_alternate]': isCurrentPlayersTurn }"
                                        >🎲</span
                                    >
                                    ROLL DICE
                                </span>
                            </button>
                        </template>
                    </div>

                    <div
                        class="bg-white/95 border-4 border-white rounded-[20px] p-4 relative shadow-[0_8px_0_rgba(0,0,0,0.2),_inset_0_0_0_3px_rgba(255,77,77,0.3)] overflow-hidden flex-1 min-h-[150px] flex flex-col"
                    >
                        <div
                            class="font-['Baloo_2'] text-base font-extrabold text-[#8B0000] uppercase tracking-wide mb-2 -rotate-1 inline-block relative pb-1"
                        >
                            Last Events
                        </div>
                        <div
                            class="flex flex-col gap-2 flex-1 overflow-y-auto pr-2 min-h-0 [&::-webkit-scrollbar]:w-2 [&::-webkit-scrollbar-track]:bg-black/10 [&::-webkit-scrollbar-track]:rounded [&::-webkit-scrollbar-thumb]:bg-[#FF4D4D] [&::-webkit-scrollbar-thumb]:rounded [&::-webkit-scrollbar-thumb:hover]:bg-[#CC0000]"
                        >
                            <div
                                v-for="(event, index) in gameEvents.slice().reverse()"
                                :key="index"
                                class="p-3 bg-[#FF4D4D] text-white font-bold font-['Baloo_2'] border-2 border-white rounded-lg shadow-[0_3px_0_rgba(0,0,0,0.2)] text-shadow-[1px_1px_0_rgba(0,0,0,0.3)] relative overflow-hidden text-base leading-relaxed mb-0.3 transition-all duration-300 hover:scale-[1.02]"
                            >
                                {{ event }}
                            </div>
                        </div>
                    </div>

                    <button
                        v-if="currentGame.phase === 'GameOver'"
                        class="min-w-[180px] h-[70px] text-[1.3rem] uppercase tracking-wide rounded-[15px] font-extrabold font-['Baloo_2'] relative overflow-hidden bg-gradient-to-b from-[#FF4D4D] to-[#CC0000] border-4 border-white shadow-[0_8px_0_rgba(0,0,0,0.3)] text-shadow-[2px_2px_0_rgba(0,0,0,0.3)] px-6 py-1 cursor-pointer transition-all duration-200 ease-in-out hover:-translate-y-[3px] hover:shadow-[0_11px_0_rgba(0,0,0,0.3)] active:translate-y-1 active:shadow-[0_4px_0_rgba(0,0,0,0.3)]"
                    >
                        <span class="flex items-center justify-center gap-2">
                            <span class="text-[1.4rem] animate-[wiggle_1s_infinite_alternate]">🔄</span>
                            PLAY AGAIN
                        </span>
                    </button>
                </div>
            </div>
        </div>

        <!-- Add the DiceModal component -->
        <DiceModal :is-open="showDiceModal" :dice-result="lastDiceRoll" @close="showDiceModal = false" />
    </div>
</template>

<style>
@keyframes wiggle {
    0% {
        transform: rotate(-5deg);
    }
    100% {
        transform: rotate(5deg);
    }
}
</style>
