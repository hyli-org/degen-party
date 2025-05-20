<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import type { GameState, Player, GameEvent, GamePhase } from "../game_data/game_data";
import GridBoard from "./GridBoard.vue";
import DiceModal from "./DiceModal.vue";
import { boardGameService, gameState, isCurrentPlayer, getLocalPlayerId } from "../game_data/game_data";
import { wsState } from "../utils/shared-websocket";
import BettingPhase from "./BettingPhase.vue";
import WheelSpinPhase from "./WheelSpinPhase.vue";
import Backdrop from "./Backdrop.vue";

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
</script>

<template>
    <div class="relative w-full h-full">
        <div
            class="absolute top-0 w-full z-1000 font-['Luckiest_Guy'] text-[4.5rem] text-[#ffd700] text-center mx-auto text-shadow-[_-2px_-2px_0_#e6a100,_2px_-2px_0_#e6a100,_-2px_2px_0_#e6a100,_2px_2px_0_#e6a100,_4px_4px_0_#b87d00,_6px_6px_0_#8b5e00] rotate-[-2deg] transition-transform duration-300 ease-in-out tracking-wide font-normal antialiased uppercase hover:scale-102 hover:rotate-[-2deg]"
        >
            DEGEN PARTY
        </div>

        <BettingPhase v-if="currentGame" />
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
