<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from "vue";
import { gameState, getLocalPlayerId, boardGameService, isCurrentPlayer } from "../game_data/game_data";
import { animState } from "./animState";

const currentGame = computed(() => gameState.game);
const localPlayerId = getLocalPlayerId();

const currentPlayer = computed(() => {
    if (!currentGame.value) return null;
    return currentGame.value.players[currentGame.value.current_turn % currentGame.value.players.length];
});
const isMyTurn = computed(() => isCurrentPlayer(localPlayerId));

const wheelOptions = [
    { label: "Quiet day", color: "#36C6FF", outcome: 0 },
    { label: "Minigame", color: "#FF4D4D", outcome: 3 },
    { label: "Fumble", color: "#00C49A", outcome: 1 },
    { label: "Minigame", color: "#FF4D4D", outcome: 4 },
    { label: "All or Nothing", color: "#FFB347", outcome: 2 },
    { label: "Minigame", color: "#FF4D4D", outcome: 5 },
];

const canvasRef = ref<HTMLCanvasElement | null>(null);
const spinning = ref(false);
const spinAngle = ref(0); // in radians
const targetAngle = ref(0); // in radians
const spinStartTime = ref(0);
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
    ctx.font = "bold 18px sans-serif";
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
    const now = performance.now();
    const elapsed = (now - spinStartTime.value) / 1000;
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
    // The wheel should land so that the outcome slice is at the top (pointer)
    const sliceAngle = (2 * Math.PI) / wheelOptions.length;
    // Add several full spins for effect
    const fullSpins = 3;
    const outcomeIndex = wheelOptions.findIndex((option) => option.outcome === outcome);
    const outcomeAngle = sliceAngle * outcomeIndex;
    targetAngle.value = fullSpins * 2 * Math.PI - outcomeAngle;
    spinStartTime.value = performance.now();
    spinning.value = true;
    animateSpin();
    lastOutcome.value = outcome;
}

watch(
    () => [animState.nextEvent, animState.nextRound],
    (_) => {
        spinAngle.value = 0;
        console.log("Spinning wheel to outcome ", animState.nextEvent);
        startSpinAnimation(animState.nextEvent!);
    },
);

onMounted(() => {
    nextTick(() => {
        drawWheel(spinAngle.value);
    });
});

function spinWheel() {
    if (!isMyTurn.value) return;
    boardGameService.sendAction({ SpinWheel: null });
}
</script>

<template>
    <div class="relative flex flex-col items-center">
        <canvas ref="canvasRef" width="340" height="340" class="mb-4" />
        <div v-if="currentGame?.phase !== 'WheelSpin'" class="text-[#FFD700] font-bold text-lg mt-2">Placing bets</div>
        <div v-else-if="!isMyTurn" class="text-[#FFD700] font-bold text-lg mt-2">Waiting for current player...</div>
        <button
            v-else
            @click="spinWheel"
            class="px-8 py-4 rounded-xl font-bold text-2xl border-4 border-white shadow-md transition-all duration-150 hover:-translate-y-1 hover:shadow-lg focus:outline-none bg-gradient-to-b from-[#FF4D4D] to-[#CC0000] text-white"
        >
            Spin the Wheel
        </button>
    </div>
</template>
