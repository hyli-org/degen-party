<script setup lang="ts">
import { ref, computed, onMounted, watch, onUnmounted } from "vue";
import type { GameState } from "../game_data/game_data";
import { boardGameService, gameState, isCurrentPlayer, getLocalPlayerId } from "../game_data/game_data";
import BettingPhase from "./BettingPhase.vue";
import { isAnimationPlayed, markAnimationPlayed } from "./animState";

// Computed properties
const currentGame = computed<GameState>(() => {
    return gameState.game as GameState;
});

const showGameOver = computed(() => {
    if (!currentGame.value) return false;
    if (currentGame.value.phase === "RewardsDistribution" || currentGame.value.phase === "GameOver") return true;
    return false;
});

const youLost = computed(() => {
    if (!currentGame.value) return false;
    const localPlayerId = getLocalPlayerId();
    const localPlayer = currentGame.value.players.find((p) => p.id === localPlayerId);
    return localPlayer && localPlayer.coins === 0 && currentGame.value.phase !== "Registration";
});

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

const lastInteractionTime = computed(() => {
    return gameState.game?.round_started_at || Date.now();
});
const shouldShowTimeoutModal = computed(() => {
    if (!currentGame.value) return false;
    const now = Date.now();
    const timeoutDuration = 5 * 60 * 1000; // 5 minutes
    return now - lastInteractionTime.value > timeoutDuration && currentGame.value.phase !== "Registration";
});
const endGame = async () => {
    await boardGameService.endGame();
};
</script>

<template>
    <div class="relative flex w-full min-h-[100vh]">
        <BettingPhase v-if="currentGame" />
        <div
            v-if="shouldShowTimeoutModal"
            class="absolute inset-0 flex flex-col items-center justify-center bg-black/70 z-50"
        >
            <div
                class="bg-white rounded-3xl shadow-2xl p-12 flex flex-col items-center gap-8 min-w-[350px] max-w-[90vw]"
            >
                <div class="text-4xl font-extrabold text-red-600 mb-2">Session Timed Out</div>
                <div class="text-lg mb-4">The game has been idle for too long.</div>
                <button
                    @click="endGame"
                    class="mt-6 px-8 py-3 rounded-xl font-bold text-lg border-4 border-blue-600 shadow-md bg-gradient-to-b from-blue-400 to-blue-700 text-white hover:-translate-y-1 hover:shadow-lg transition-all cursor-pointer"
                >
                    Back to lobby
                </button>
            </div>
        </div>

        <div
            v-if="showGameOver || (youLost && !isAnimationPlayed('youLostSpectate'))"
            class="absolute inset-0 flex flex-col items-center justify-center bg-black/60 z-50"
        >
            <div
                class="bg-white rounded-3xl shadow-2xl p-12 flex flex-col items-center gap-8 min-w-[350px] max-w-[90vw]"
            >
                <div v-if="showGameOver" class="text-5xl font-extrabold text-[#FFD700] drop-shadow-lg mb-2">
                    GAME ENDED
                </div>
                <div v-else class="text-5xl font-extrabold text-[#FFD700] drop-shadow-lg mb-2">YOU LOST</div>

                <div v-if="youLost" class="text-2xl font-bold text-[#8B0000] mb-4">
                    You lost! 😢 Better luck next time!
                </div>
                <div v-else-if="allLost" class="text-2xl font-bold text-[#8B0000] mb-4">
                    Everyone lost! No one has any coins left. 😵
                </div>
                <div v-else class="text-2xl font-bold text-[#8B0000] mb-4">
                    Winner: <span class="text-green-600">{{ winner?.name }}</span> 🏆 (+100 coins!)
                </div>
                <div v-if="!youLost" class="w-full">
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
                    v-if="!youLost"
                    @click="gameState.isInLobby = true"
                    class="mt-6 px-8 py-3 rounded-xl font-bold text-lg border-4 border-[#FFD700] shadow-md bg-gradient-to-b from-[#4DAAFF] to-[#0077CC] text-white hover:-translate-y-1 hover:shadow-lg transition-all"
                >
                    Back to lobby
                </button>
                <button
                    v-else
                    @click="markAnimationPlayed('youLostSpectate')"
                    class="mt-6 px-8 py-3 rounded-xl font-bold text-lg border-4 border-[#FFD700] shadow-md bg-gradient-to-b from-[#4DAAFF] to-[#0077CC] text-white hover:-translate-y-1 hover:shadow-lg transition-all"
                >
                    Spectate
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
