<script setup lang="ts">
import { computed, ref, watchEffect } from "vue";
import { gameState, playerAvatar, boardGameService, playerColor } from "../game_data/game_data";
import {
    animState,
    currentRoundEvents,
    getAnimationPlayedTime,
    isAnimationPlayed,
    markAnimationPlayed,
    markAnimationPlayedIn,
    roundOutcome,
} from "./animState";

const currentGame = computed(() => gameState.game);
const avatars = computed(() => currentGame.value?.players || []);

const TRAIL_LENGTH = 10; // 10 rounds
const TRAIL_HEIGHT = 256; // px, for 1.0 progress

interface EventMarker {
    round: number;
    outcome: number | undefined;
    rel: number; // relative shift from current round
}

const percentTravelled = computed(() => {
    if (!currentGame.value) return 0;
    if (!isAnimationPlayed("SpinWheel")) return 0;
    return Math.min(1, (animState.timeInRound - getAnimationPlayedTime("SpinWheel")) / 3);
});

function outcomeIcon(outcome: number) {
    if (outcome === 0) return "🌞";
    if (outcome === 1) return "⛈️";
    if (outcome === 2) return "☣️";
    return "🚀";
}

// End of round timers
watchEffect(() => {
    if (currentRoundEvents.value.outcome > 2) {
        if (!isAnimationPlayed("GoToMinigame") && percentTravelled.value > 0.99) {
            markAnimationPlayedIn("GoToMinigame", 0.5);
        }
        if (isAnimationPlayed("GoToMinigame") && !isAnimationPlayed("WentToMinigame")) {
            markAnimationPlayed("WentToMinigame");
            gameState.isInMinigame = true;
        }
        return;
    }
    if (animState.currentRoundIndex >= TRAIL_LENGTH - 1) {
        if (!isAnimationPlayed("StartFinalGame") && percentTravelled.value > 0.99) {
            markAnimationPlayedIn("StartFinalGame", 1.0, () => {
                gameState.isInMinigame = true;
            });
        }
        return;
    }
    if (!isAnimationPlayed("EndRound") && percentTravelled.value > 0.99) {
        markAnimationPlayedIn("EndRound", 1.0);
    }
    if (isAnimationPlayed("EndRound") && !isAnimationPlayed("EndedRound")) {
        markAnimationPlayed("EndedRound");
        animState.currentRoundIndex++;
    }
});

// Build a list of all event markers (past, current, future)
const eventMarkers = computed<EventMarker[]>(() => {
    const markers: EventMarker[] = [];
    for (let i = 0; i < TRAIL_LENGTH; i++) {
        const entry = animState.eventHistory[i];
        markers.push({
            round: i,
            outcome: entry ? entry.outcome : undefined,
            rel: i + 1,
        });
    }
    return markers;
});

const containerShift = computed(() => ({
    transform: `translateY(${(animState.currentRoundIndex + percentTravelled.value) * TRAIL_HEIGHT}px)`,
}));
</script>

<template>
    <div class="backdrop">
        <div
            class="mars-bg"
            :style="{
                backgroundPosition: `0px ${percentTravelled * TRAIL_HEIGHT}px`,
            }"
        ></div>
        <div class="w-full" :style="containerShift">
            <div class="trail"></div>
            <!-- Player avatar bobbing down the map-->
            <div
                v-for="(avatar, idx) in avatars"
                :key="avatar.id"
                class="avatar z-50"
                :style="{
                    top: `calc(50% - ${(animState.currentRoundIndex + percentTravelled) * TRAIL_HEIGHT}px)`,
                }"
            >
                <p class="avatar-sprite" :style="{ backgroundColor: playerColor(avatar.id) }">
                    {{ playerAvatar(avatar.id) }}
                </p>
            </div>
            <!-- Render all event markers -->
            <div
                v-for="marker in eventMarkers"
                :key="marker.round"
                class="event-marker"
                :class="{
                    current: marker.rel === 0,
                    ghost:
                        marker.rel > animState.currentRoundIndex + 1 ||
                        (marker.rel === animState.currentRoundIndex + 1 && percentTravelled < 0.3),
                }"
                :style="{
                    top: `calc(50% - ${marker.rel * TRAIL_HEIGHT}px)`,
                }"
            >
                <span v-if="marker.outcome !== undefined">{{ outcomeIcon(marker.outcome) }}</span>
                <span v-else class="ghost">?</span>
            </div>

            <div
                v-for="marker in eventMarkers"
                :key="marker.round"
                :class="
                    `event-box transition-all duration-500 absolute left-12 w-[calc(50%-8rem)] -translate-y-1/2 p-4 bg-white/80 rounded-lg shadow-lg z-10` +
                    (marker.rel > animState.currentRoundIndex + 1 ||
                    (marker.rel === animState.currentRoundIndex + 1 && percentTravelled < 0.3)
                        ? ' ghost'
                        : '')
                "
                :style="{
                    top: `calc(50% - ${marker.rel * TRAIL_HEIGHT}px)`,
                }"
            >
                <div class="event-title">{{ roundOutcome(marker.round).title }}</div>
                <div class="event-desc whitespace-pre">{{ roundOutcome(marker.round).description }}</div>
            </div>
            <!-- City at the end -->
            <div
                class="absolute left-1/2 z-40"
                :style="{
                    top: `calc(50% - ${TRAIL_LENGTH * TRAIL_HEIGHT}px)`,
                    transform: 'translate(-50%, -75%)',
                    pointerEvents: 'none',
                }"
            >
                <!--<span style="font-size: 4rem; line-height: 1; filter: drop-shadow(0 4px 16px #0008)">🏙️</span>-->
                <img
                    src="../assets/city.png"
                    alt="City"
                    class="w-[512px] h-[512px] filter drop-shadow-[0_4px_16px_#0008]"
                />
            </div>
        </div>
    </div>
</template>

<style scoped>
.event-marker {
    position: absolute;
    left: 50%;
    width: 92px;
    height: 92px;
    transform: translateX(-50%);
    background: white;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 3rem;
    opacity: 1;
    z-index: 10;
    transition:
        opacity 1s,
        filter 0.2s,
        top 0.4s cubic-bezier(0.68, -0.55, 0.27, 1.55);
    animation: bounce-in 0.7s cubic-bezier(0.68, -0.55, 0.27, 1.55);
    animation-fill-mode: forwards;
}
.event-marker.current {
    opacity: 1;
    filter: drop-shadow(0 0 8px #ffd700);
    font-size: 2.5rem;
}
.ghost {
    display: none;
    opacity: 0;
}
.event-marker.has-outcome {
    font-weight: bold;
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
}
.trail {
    position: absolute;
    left: 50%;
    bottom: -100vh;
    transform: translateX(-50%);
    z-index: 10;
    width: 72px;
    height: 10000px;
    background: repeating-linear-gradient(to top, #fff3, #fff3 32px, #b84a39 32px, #b84a39 96px);
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
