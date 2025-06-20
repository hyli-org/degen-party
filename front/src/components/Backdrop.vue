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

function easeInOutS(t: number): number {
    // Smoothstep S-curve: 3t^2 - 2t^3
    return t <= 0 ? 0 : t >= 1 ? 1 : t * t * (3 - 2 * t);
}

const percentTravelled = computed(() => {
    if (!currentGame.value) return 0;
    if (!isAnimationPlayed("SpinWheel")) return 0;
    const linear = Math.min(1, (animState.timeInRound - getAnimationPlayedTime("SpinWheel")) / 3);
    return easeInOutS(linear);
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

// Spiral positioning for avatars
function getSpiralPosition(idx: number, total: number) {
    // Spiral parameters
    const centerX = 50; // percent
    const centerY = 50; // percent
    const spiralTurns = 1.5; // how many turns in the spiral
    const minRadius = 8; // px
    const maxRadius = 16; // px, will scale down if more players
    const scale = Math.max(0.5, 1 - (total - 1) * 0.12); // scale down if more players
    const t = total > 1 ? idx / (total - 1) : 0.5;
    const angle = 2 * Math.PI * spiralTurns * t - Math.PI / 2;
    const radius = minRadius + (maxRadius - minRadius) * t * scale;
    const trailComponent = (animState.currentRoundIndex + percentTravelled.value) * TRAIL_HEIGHT;

    // Special case: 2 players side-by-side
    if (total === 2) {
        const offset = 16; // px, horizontal offset
        return {
            left: `calc(${centerX}% + ${idx === 0 ? -offset : offset}px)`,
            top: `calc(${centerY}% - ${trailComponent}px)`,
            scale: scale,
        };
    }

    return {
        left: `calc(${centerX}% + ${Math.cos(angle) * radius}px)`,
        top: `calc(${centerY}% + ${Math.sin(angle) * radius - trailComponent}px)`,
        scale: scale,
    };
}
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
                class="avatar z-50 spiral-avatar"
                :style="getSpiralPosition(idx, avatars.length)"
            >
                <p class="avatar-sprite spiral-wiggle" :style="{ backgroundColor: playerColor(avatar.id) }">
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

            <!-- lg:left-1/2 lg:-translate-x-1/2 lg:translate-y-[4em] lg:w-[300px] -->
            <div
                v-for="marker in eventMarkers"
                :key="marker.round"
                :class="
                    `event-box transition-all duration-500 absolute 
                        left-1/2 -translate-x-1/2 -translate-y-[9em] w-[calc(100%-2em)]
                            sm:left-12 sm:w-[calc(50%-8rem)] sm:-translate-y-1/2 sm:translate-x-0
                            xl:translate-x-0 xl:left-12 xl:w-[calc(50%-8rem)] xl:-translate-y-1/2
                        p-4 bg-white/80 rounded-lg shadow-lg z-10` +
                    (marker.rel > animState.currentRoundIndex + 1 ||
                    (marker.rel === animState.currentRoundIndex + 1 && percentTravelled < 0.3)
                        ? ' ghost'
                        : '')
                "
                :style="{
                    top: `calc(50% - ${marker.rel * TRAIL_HEIGHT}px)`,
                }"
            >
                <p class="event-title">{{ roundOutcome(marker.round).title }}</p>
                <p class="event-desc whitespace-pre-line">{{ roundOutcome(marker.round).description }}</p>
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
    background: url("../assets/mars.jpg") repeat center center;
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
}
.avatar-sprite {
    width: 30px;
    height: 30px;
    text-align: center;
    border-radius: 50%;
    border: 2px solid #fff5;
    box-shadow: 0 2px 8px #0008;
}
.spiral-avatar {
    transition: transform 0.5s;
}
@keyframes spiral-wiggle {
    0% {
        transform: translate(-50%, -50%) scale(1) rotate(-7deg);
    }
    50% {
        transform: translate(-50%, -50%) scale(1.05) rotate(7deg);
    }
    100% {
        transform: translate(-50%, -50%) scale(1) rotate(-7deg);
    }
}
.spiral-wiggle {
    animation: spiral-wiggle 1.2s infinite alternate;
}
</style>
