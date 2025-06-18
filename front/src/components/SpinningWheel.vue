<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from "vue";
import { gameState, getLocalPlayerId, boardGameService, isCurrentPlayer, isObserver } from "../game_data/game_data";
import {
    animState,
    currentRoundEvents,
    markAnimationPlayed,
    isAnimationPlayed,
    getAnimationPlayedTime,
} from "./animState";
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

const wheelOptions = [
    { label: "Quiet day", color: "#36C6FF", outcome: 0 },
    { label: "Minigame", color: "#FF4D4D", outcome: 3 },
    { label: "Fumble", color: "#00C49A", outcome: 1 },
    { label: "Minigame", color: "#FF4D4D", outcome: 4 },
    { label: "All or Nothing", color: "#FFB347", outcome: 2 },
    //{ label: "Minigame", color: "#FF4D4D", outcome: 5 },
];

const canvasRef = ref<HTMLCanvasElement | null>(null);
const spinning = ref(false);
const spinAngle = ref(0); // in radians
const targetAngle = ref(0); // in radians
const spinDuration = 2; // seconds
const lastOutcome = ref<number | null>(null);

function drawWheel(angle: number) {
    angle -= Math.PI / 6 + Math.PI / 2; // Rotate to point at the top

    const canvas = canvasRef.value;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const size = canvas.width;
    ctx.clearRect(0, 0, size, size);
    const center = size / 2;
    const radius = size / 2 - 10;
    const sliceAngle = (2 * Math.PI) / wheelOptions.length;

    // Draw slices
    for (let i = 0; i < wheelOptions.length; i++) {
        ctx.save();
        ctx.beginPath();
        ctx.moveTo(center, center);
        ctx.arc(center, center, radius, angle + i * sliceAngle, angle + (i + 1) * sliceAngle);
        ctx.closePath();
        ctx.fillStyle = wheelOptions[i].color;
        ctx.fill();
        ctx.restore();
    }

    // Draw labels
    ctx.save();
    ctx.translate(center, center);
    ctx.font = "bold 12px sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    for (let i = 0; i < wheelOptions.length; i++) {
        const theta = angle + (i + 0.5) * sliceAngle;
        ctx.save();
        ctx.rotate(theta);
        ctx.translate(radius * 0.65, 0);
        ctx.fillStyle = "#222";
        ctx.fillText(wheelOptions[i].label, 0, 0);
        ctx.restore();
    }
    ctx.restore();

    // Draw center circle
    ctx.save();
    ctx.beginPath();
    ctx.arc(center, center, 40, 0, 2 * Math.PI);
    ctx.fillStyle = "#fff";
    ctx.shadowColor = "#FFD700";
    ctx.shadowBlur = 10;
    ctx.fill();
    ctx.restore();

    // Draw pointer
    ctx.save();
    //ctx.rotate(-Math.PI / 2);
    ctx.translate(center, center);
    ctx.beginPath();
    ctx.moveTo(0, -radius + 20);
    ctx.lineTo(-22, -radius - 20);
    ctx.lineTo(22, -radius - 20);
    ctx.closePath();
    ctx.fillStyle = "#444";
    ctx.shadowColor = "#0004";
    ctx.shadowBlur = 4;
    ctx.fill();
    ctx.restore();
}

function animateSpin() {
    if (!spinning.value) return;
    const elapsed = animState.timeInRound - getAnimationPlayedTime("SpinWheel");
    let t = Math.min(1, elapsed / spinDuration);
    // Ease out cubic
    t = 1 - Math.pow(1 - t, 3);
    spinAngle.value = targetAngle.value * t;
    drawWheel(spinAngle.value);
    if (t < 1) {
        requestAnimationFrame(animateSpin);
    } else {
        spinning.value = false;
        spinAngle.value = targetAngle.value;
        drawWheel(spinAngle.value);
    }
}

function startSpinAnimation(outcome: number) {
    markAnimationPlayed("SpinWheel");
    // The wheel should land so that the outcome slice is at the top (pointer)
    const sliceAngle = (2 * Math.PI) / wheelOptions.length;
    // Add several full spins for effect
    const fullSpins = 3;
    const outcomeIndex = wheelOptions.findIndex((option) => option.outcome === outcome);
    const outcomeAngle = sliceAngle * outcomeIndex;
    targetAngle.value = fullSpins * 2 * Math.PI - outcomeAngle;
    spinning.value = true;
    animateSpin();
    lastOutcome.value = outcome;
}

watchEffect(() => {
    const outcome = currentRoundEvents.value?.outcome;
    console.log("Outcome changed", outcome);
    if (outcome === undefined || outcome === -1 || isAnimationPlayed("SpinWheel")) return;
    spinAngle.value = 0;
    startSpinAnimation(outcome);
});

onMounted(() => {
    nextTick(() => {
        drawWheel(spinAngle.value);
    });
});

function spinWheel() {
    boardGameService.sendAction({ SpinWheel: null });
}
</script>

<template>
    <div class="relative flex flex-col items-center transition-all transition-duration-500">
        <canvas ref="canvasRef" width="256" height="256" class="mb-4" />
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
