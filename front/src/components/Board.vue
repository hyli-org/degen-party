<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import type { GameState, Player, GameEvent, GamePhase } from "../game_data/game_data";
import GridBoard from "./GridBoard.vue";
import DiceModal from "./DiceModal.vue";
import { boardGameService, gameState, isCurrentPlayer, getLocalPlayerId } from "../game_data/game_data";
import { wsState } from "../utils/shared-websocket";
import BettingPhase from "./BettingPhase.vue";
import Backdrop from "./Backdrop.vue";

// Computed properties
const currentGame = computed<GameState>(() => {
    return gameState.game as GameState;
});

const showGameOver = computed(() => currentGame.value && currentGame.value.phase === "GameOver");
const playersSorted = computed(() => {
    if (!currentGame.value) return [];
    // Sort by coins descending, then by name
    return [...currentGame.value.players].sort((a, b) => b.coins - a.coins || a.name.localeCompare(b.name));
});
const winner = computed(() => playersSorted.value[0]);
const allLost = computed(() => {
    if (!currentGame.value) return false;
    return currentGame.value.players.every((p) => p.coins === 0);
});
</script>

<template>
    <div class="relative w-full h-full">
        <div
            class="absolute top-0 w-full z-1000 font-['Luckiest_Guy'] text-[4.5rem] text-[#ffd700] text-center mx-auto text-shadow-[_-2px_-2px_0_#e6a100,_2px_-2px_0_#e6a100,_-2px_2px_0_#e6a100,_2px_2px_0_#e6a100,_4px_4px_0_#b87d00,_6px_6px_0_#8b5e00] rotate-[-2deg] transition-transform duration-300 ease-in-out tracking-wide font-normal antialiased uppercase hover:scale-102 hover:rotate-[-2deg]"
        >
            DEGEN PARTY
        </div>

        <BettingPhase v-if="currentGame" />

        <div v-if="showGameOver" class="absolute inset-0 flex flex-col items-center justify-center bg-black/60 z-50">
            <div
                class="bg-white rounded-3xl shadow-2xl p-12 flex flex-col items-center gap-8 min-w-[350px] max-w-[90vw]"
            >
                <div class="text-5xl font-extrabold text-[#FFD700] drop-shadow-lg mb-2">GAME ENDED</div>
                <div v-if="allLost" class="text-2xl font-bold text-[#8B0000] mb-4">
                    Everyone lost! No one has any coins left. 😵
                </div>
                <div v-else class="text-2xl font-bold text-[#8B0000] mb-4">
                    Winner: <span class="text-green-600">{{ winner?.name }}</span> 🏆 (+100 coins!)
                </div>
                <div class="w-full">
                    <div class="text-lg font-bold text-gray-700 mb-2">Final Standings:</div>
                    <ol class="list-decimal pl-6 space-y-2">
                        <li v-for="(player, idx) in playersSorted" :key="player.id" class="flex items-center gap-3">
                            <span
                                class="font-bold text-xl"
                                :class="{
                                    'text-green-600': idx === 0 && !allLost,
                                    'text-gray-500': idx !== 0 || allLost,
                                }"
                                >{{ player.name }}</span
                            >
                            <span class="text-lg">- {{ player.coins }} 🪙</span>
                            <span v-if="idx === 0 && !allLost" class="ml-2 text-2xl">🏆</span>
                        </li>
                    </ol>
                </div>
                <button
                    @click="boardGameService.reset()"
                    class="mt-6 px-8 py-3 rounded-xl font-bold text-lg border-4 border-[#FFD700] shadow-md bg-gradient-to-b from-[#4DAAFF] to-[#0077CC] text-white hover:-translate-y-1 hover:shadow-lg transition-all"
                >
                    Restart Game
                </button>
            </div>
        </div>
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
