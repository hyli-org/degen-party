<script setup lang="ts">
import { ref, computed, nextTick } from "vue";
import { gameState, getLocalPlayerId, boardGameService, isCurrentPlayer, isObserver } from "../game_data/game_data";
import { spinAngle, isAnimationPlayed } from "./animState";
import { watchEffect } from "vue";

const currentGame = computed(() => gameState.game);

const currentState = computed(() => {
    if (!currentGame.value) return "Other";
    // reactivity
    isAnimationPlayed("BettingTimeUp");
    if (currentGame.value.phase != "WheelSpin") {
        if (currentGame.value.phase == "Betting") {
            if (Date.now() - currentGame.value.round_started_at > 30 * 1000) {
            } else {
                return "Betting";
            }
        } else {
            return currentGame.value.phase;
        }
    }
    return "Waiting";
});

const spinWheel = () => {
    boardGameService.sendAction({ SpinWheel: null });
};
</script>

<template>
    <div class="relative flex flex-col items-center transition-all transition-duration-500">
        <div class="relative w-[256px] h-[256px]">
            <img
                :style="`transform: rotate(${spinAngle}rad)`"
                class="w-[256px] h-[256px] absolute"
                src="/src/assets/wheel_itself.png"
            />
            <img class="w-[256px] h-[256px] absolute" src="/src/assets/wheel_backdrop.png" />
        </div>
        <button
            v-if="currentState != 'Betting' && !isAnimationPlayed('SpinWheel') && !isObserver()"
            @click="spinWheel"
            class="spinWheelButton"
        >
            Spin the Wheel
        </button>
    </div>
</template>

<style scoped>
.spinWheelButton {
    padding-left: 2rem;
    padding-right: 2rem;
    padding-top: 1rem;
    padding-bottom: 1rem;
    border-radius: 0.75rem;
    font-weight: bold;
    font-size: 1.5rem;
    border-width: 4px;
    border-color: #fff;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
    transition-property: all;
    transition-duration: 150ms;
    background: linear-gradient(to bottom, #ff4d4d, #cc0000);
    color: #fff;
}

.spinWheelButton:not(:disabled):hover {
    transform: translateY(-0.25rem);
    box-shadow: 0 10px 15px rgba(0, 0, 0, 0.15);
    outline: none;
    /* background and color remain the same as default */
}

.spinWheelButton:disabled {
    background: #9ca3af; /* Tailwind gray-400 */
    color: #374151; /* Tailwind gray-700 */
    cursor: not-allowed;
}
</style>
