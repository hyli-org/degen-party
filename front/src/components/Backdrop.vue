<script setup lang="ts">
import { computed, ref, watchEffect } from "vue";
import { gameState, playerAvatar, boardGameService, playerColor } from "../game_data/game_data";
import { animState, nextRoundEvent } from "./animState";

const currentGame = computed(() => gameState.game);
const avatars = computed(() => currentGame.value?.players || []);
const currentRound = computed(() => currentGame.value?.round ?? 0);

const percentTravelled = computed(() => {
    if (!currentGame.value) return 0;
    return Math.min(3, animState.wheelSpinTime || 0);
});

// Make the trail move up and down
const trailStyle = computed(() => ({
    transform: `translate(-50%, ${(percentTravelled.value * 500) / 3}px)`,
}));
const eventStyle = computed(() => ({
    transform: `translate(0, ${-500 + (percentTravelled.value * 500) / 3}px)`,
}));
</script>

<template>
    <div class="backdrop">
        <div
            class="mars-bg"
            :style="{
                backgroundPosition: `0px ${(percentTravelled * 500) / 3}px`,
            }"
        ></div>
        <div class="trail-container">
            <div class="trail" :style="trailStyle"></div>
        </div>
        <div v-if="nextRoundEvent && percentTravelled > 1.2" :style="eventStyle" class="absolute z-20 left-1/2 top-1/2">
            <div class="randomevent bg-white px-4 rounded-xl shadow-lg text-[3rem]">
                <template v-if="animState.nextEvent === 0">🌞</template>
                <template v-else-if="animState.nextEvent === 1">⛈️</template>
                <template v-else-if="animState.nextEvent === 2">☣️</template>
                <template v-else>🚀</template>
            </div>
        </div>
        <div v-for="(avatar, idx) in avatars" :key="avatar.id" class="avatar z-50">
            <p class="avatar-sprite" :style="{ backgroundColor: playerColor(avatar.id) }">
                {{ playerAvatar(avatar.id) }}
            </p>
        </div>
        <!-- Show a big city emoji at the end of the path if the next round is round 10 -->
        <div
            v-if="currentRound === 10"
            class="city-emoji absolute left-1/2 bottom-0 z-50 flex justify-center items-end"
            style="transform: translateX(-50%); width: 100vw; pointer-events: none"
        >
            <span style="font-size: 10rem; line-height: 1; filter: drop-shadow(0 4px 16px #0008)">🏙️</span>
        </div>
        <div
            v-show="nextRoundEvent && percentTravelled > 1.5"
            class="event-box transition-all duration-500 absolute left-0 p-8 bg-white/80 rounded-lg shadow-lg z-10"
        >
            <div class="event-title">{{ nextRoundEvent?.title }}</div>
            <div class="event-desc">{{ nextRoundEvent?.description }}</div>
        </div>
    </div>
</template>

<style scoped>
.randomevent {
    animation: bounce-in 0.7s cubic-bezier(0.68, -0.55, 0.27, 1.55);
    opacity: 0;
    animation-fill-mode: forwards;
}

@keyframes bounce-in {
    0% {
        transform: scale(0.7) translate(-50%, -50%);
        opacity: 0;
    }
    60% {
        transform: scale(1.1) translate(-50%, -50%);
        opacity: 1;
    }
    80% {
        transform: scale(0.95) translate(-50%, -50%);
    }
    100% {
        transform: scale(1) translate(-50%, -50%);
        opacity: 1;
    }
}
.backdrop {
    overflow: hidden;
    background: #1a1a2a;
    display: flex;
    align-items: center;
    justify-content: center;
}
.mars-bg {
    position: absolute;
    inset: 0;
    background: url("../assets/mars.png") repeat center center;
    background-size: 256px;
    opacity: 0.7;
    z-index: 1;
}
.trail-container {
    position: relative;
    width: 120px;
    height: 100vh;
    z-index: 2;
    display: flex;
    justify-content: center;
}
.trail {
    position: absolute;
    left: 50%;
    top: 0;
    width: 24px;
    height: 200vh;
    background: repeating-linear-gradient(to top, #fff3, #fff3 8px, #b84a39 8px, #b84a39 32px);
    border-radius: 12px;
}
.avatar {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
}
.avatar-sprite {
    width: 30px;
    height: 30px;
    text-align: center;
    border-radius: 50%;
    border: 2px solid #fff5;
    box-shadow: 0 2px 8px #0008;
}
</style>
