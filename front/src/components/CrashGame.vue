<template>
    <div class="crash-game">
        <div class="game-title">CRASH GAME</div>

        <div class="game-container card relative">
            <canvas ref="gameCanvas" class="game-canvas"></canvas>

            <div v-if="gameEnded" class="game-over">
                <div class="crash-title">CRASHED AT</div>
                <div class="crash-value">{{ currentMultiplier.toFixed(2) }}x</div>
                <div class="crash-face">💥</div>
            </div>

            <div v-else-if="!gameStarted" class="game-waiting">
                <div class="waiting-text">GAME STARTING SOON!</div>
                <div class="players-bet">Players Ready: {{ playersWhoBet }}</div>
            </div>

            <div v-else-if="gameStarted" class="current-multiplier">
                <div class="multiplier-value">
                    {{ currentMultiplier.toFixed(2) }}<span class="multiplier-x">x</span>
                </div>
                <div class="current-payout">Current Payout</div>
                <div v-if="betAmount > 0" class="profit-display">
                    +{{ (betAmount * currentMultiplier - betAmount).toFixed(2) }} coins!
                </div>
            </div>

            <div v-if="hasPlayerCashedOut" class="cashed-out-overlay">
                <div class="cashout-content">
                    <div class="cashout-multiplier">CASHED OUT AT {{ playerCashedOutAt.toFixed(2) }}x</div>
                    <div class="cashout-profit">
                        <span class="profit-label">PROFIT:</span>
                        <span class="profit-amount">
                            🪙
                            {{ Math.floor(betAmount * playerCashedOutAt - betAmount) }}
                        </span>
                    </div>
                </div>
            </div>

            <div v-if="gameStarted || gameEnded" class="current-time">
                <span class="time-label">TIME:</span> {{ calculateGameTime.toFixed(1) }}s
            </div>
        </div>

        <div class="game-controls card">
            <div class="controls-wrapper">
                <div class="bet-controls">
                    <div class="bet-label">COINS TO WAGER</div>
                    <div class="bet-input-wrapper">
                        <div class="currency-symbol">🪙</div>
                        <input
                            type="number"
                            v-model="betAmount"
                            :disabled="gameStarted || !canPlaceBet"
                            min="1"
                            class="bet-input"
                            style="padding-right: 40px"
                        />
                    </div>
                    <div class="bet-quick-amounts">
                        <button class="quick-amount" @click="betAmount = 5">
                            5 <span class="points-text">COINS</span>
                        </button>
                        <button class="quick-amount" @click="betAmount = 10">
                            10 <span class="points-text">COINS</span>
                        </button>
                        <button class="quick-amount" @click="betAmount = 25">
                            25 <span class="points-text">COINS</span>
                        </button>
                        <button class="quick-amount" @click="betAmount = 50">
                            50 <span class="points-text">COINS</span>
                        </button>
                    </div>
                </div>

                <button
                    v-if="!gameStarted && !gameEnded"
                    class="action-button bet-action"
                    @click="handleActionButton"
                    :disabled="!canPlaceBet"
                >
                    <span class="btn-text"> <span class="btn-icon">🎲</span> PLACE BETS! </span>
                </button>
                <button
                    v-else-if="gameStarted && !hasPlayerCashedOut && !gameEnded"
                    class="action-button cashout-action"
                    @click="handleActionButton"
                >
                    <span class="btn-text"> <span class="btn-icon">🚀</span> BLAST OFF! </span>
                </button>
                <button v-else class="action-button next-action" @click="handleActionButton">
                    <span class="btn-text"> <span class="btn-icon">🎮</span> BACK TO BOARD </span>
                </button>
            </div>
        </div>

        <ConfettiEffect :active="showConfetti" :duration="3000" />

        <div v-if="showFinalResults" class="final-results-modal">
            <div class="final-results-content">
                <div class="final-results-title">FINAL RESULTS</div>
                <div class="final-results-list">
                    <div v-for="result in finalResults" :key="result.playerId" class="result-item">
                        <div class="player-name">{{ result.playerName }}</div>
                        <div :class="['result-amount', result.profit >= 0 ? 'profit' : 'loss']">
                            {{ result.profit >= 0 ? "+" : "" }}{{ result.profit }} 🪙
                        </div>
                    </div>
                </div>
                <button v-if="gameEnded" class="action-button next-action" @click="handleActionButton">
                    <span class="btn-text"> <span class="btn-icon">🎮</span> BACK TO BOARD </span>
                </button>
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, onUnmounted, defineEmits, watch, watchEffect } from "vue";
import ConfettiEffect from "./ConfettiEffect.vue";
import { crashGameService, crashGameState } from "../game_data/crash";
import { gameState, getLocalPlayerId } from "../game_data/game_data";
import { addBackgroundEffects, Cashout, drawFlightPath } from "./CrashGameHelper";

// Define emits for party game integration
const emits = defineEmits(["win", "lose"]);

// Game canvas
const gameCanvas = ref<HTMLCanvasElement | null>(null);
const ctx = ref<CanvasRenderingContext2D | null>(null);

// Animation state
const lastTimestamp = ref(0);
const animationFrameId = ref(0);
const introAnimationActive = ref(false);
const introProgress = ref(0);
const rocketAnimationOffset = ref(0);

const calculateGameTime = computed(() => Date.now() / 1000 - gameStartTime.value);
const handleActionButton = () => {
    if (!gameStarted.value) {
        crashGameService.placeBet(betAmount.value);
    } else if (!gameEnded.value) {
        crashGameService.cashOut();
    } else {
        console.log("Game ended, returning to board...");
        crashGameService.returnToBoard();
    }
};

/*
// Temp hack: make it look like the game is on and we bet
crashGameState.minigame = {
    is_running: true,
    current_multiplier: 1.4,
    waiting_for_start: false,
    active_bets: {
        [getLocalPlayerId()]: { amount: 34, cashed_out_at: null },
    },
};
*/

const gameStartTime = ref(0);
watchEffect(() => {
    if (!gameStartTime.value && crashGameState.minigame?.is_running) gameStartTime.value = Date.now() / 1000;
});

const gameStarted = computed(() => !crashGameState.minigame?.waiting_for_start);
const gameEnded = computed(() => gameStarted.value && !crashGameState.minigame?.is_running);
const currentMultiplier = computed(() => crashGameState.minigame?.current_multiplier ?? 1);

// Update computed property to show ratio of players who bet
const playersWhoBet = computed(() => {
    const activeBets = Object.keys(crashGameState.minigame?.active_bets || {}).length;
    const totalPlayers = gameState.game?.players.length || 0;
    return `${activeBets}/${totalPlayers}`;
});

const canPlaceBet = computed(() => !gameStarted.value);

const hasPlayerCashedOut = computed(() => {
    return !!crashGameState.minigame?.active_bets?.[getLocalPlayerId()]?.cashed_out_at;
});
const playerCashedOutAt = computed(() => {
    return crashGameState.minigame?.active_bets?.[getLocalPlayerId()]?.cashed_out_at ?? 0;
});

const cashouts = computed(() => {
    const cashoutList: Cashout[] = [];
    if (crashGameState.minigame?.active_bets) {
        for (const [playerId, bet] of Object.entries(crashGameState.minigame.active_bets)) {
            if (bet.cashed_out_at) {
                const player = gameState.game?.players.find((p) => p.id.toString() === playerId);
                cashoutList.push({
                    playerId,
                    amount: bet.amount,
                    multiplier: bet.cashed_out_at,
                    playerName: player?.name || "Unknown Player",
                });
            }
        }
    }
    return cashoutList;
});

const betAmount = ref(10);

// Confetti state
const showConfetti = ref(false);

// Add ship image loading at the top of the script section
const shipImage = new Image();
shipImage.src = "/ship.svg";

// Initialize canvas and start the game loop
onMounted(() => {
    if (gameCanvas.value) {
        ctx.value = gameCanvas.value.getContext("2d");
        resizeCanvas();
        window.addEventListener("resize", resizeCanvas);
        animationFrameId.value = requestAnimationFrame(gameLoop);
    }
});

// Clean up on component unmount
onUnmounted(() => {
    window.removeEventListener("resize", resizeCanvas);
    cancelAnimationFrame(animationFrameId.value);
});

// Resize canvas to fit container
function resizeCanvas() {
    if (!gameCanvas.value || !gameCanvas.value.parentElement) return;

    const parent = gameCanvas.value.parentElement;

    // Set the canvas dimensions to match the parent container size
    gameCanvas.value.width = parent.clientWidth;
    gameCanvas.value.height = parent.clientHeight;

    // Ensure the canvas is properly sized with a small delay
    // This helps when the container might still be adjusting its own size
    setTimeout(() => {
        if (gameCanvas.value && gameCanvas.value.parentElement) {
            gameCanvas.value.width = gameCanvas.value.parentElement.clientWidth;
            gameCanvas.value.height = gameCanvas.value.parentElement.clientHeight;
            render();
        }
    }, 100);

    // Initial render
    render();
}

// Main game loop - now just handles rendering, not game state updates
function gameLoop(timestamp: number) {
    const deltaTime = timestamp - lastTimestamp.value;
    lastTimestamp.value = timestamp;

    // Handle intro animation locally
    if (introAnimationActive.value) {
        introProgress.value = Math.min(1, introProgress.value + deltaTime / 1000);
        if (introProgress.value >= 1) {
            introAnimationActive.value = false;
        }
    }

    // Update rocket animation
    rocketAnimationOffset.value = Math.sin(Date.now() / 200) * 1.5;

    // Render the current frame
    render();

    // Continue the loop
    animationFrameId.value = requestAnimationFrame(gameLoop);
}

// Render the current frame
function render() {
    if (!ctx.value || !gameCanvas.value) return;

    // Clear canvas
    ctx.value.clearRect(0, 0, gameCanvas.value.width, gameCanvas.value.height);

    // Add subtle glow to background for casino ambiance
    addBackgroundEffects(ctx as any, gameCanvas as any, gameEnded);

    // Draw flight path and rocket
    drawFlightPath(
        gameCanvas as any,
        ctx as any,
        currentMultiplier,
        shipImage,
        introAnimationActive,
        introProgress,
        cashouts,
    );
}

// Add new interface for final results
interface FinalResult {
    playerId: string;
    playerName: string;
    profit: number;
}

// Add new refs for final results
const showFinalResults = ref(false);
const finalResults = ref<FinalResult[]>([]);

// Watch for game end to show final results
watch(gameEnded, (newValue) => {
    if (newValue) {
        // Add timeout to show modal after crash animation
        setTimeout(() => {
            // Calculate final results for each player
            const results: FinalResult[] = [];

            if (crashGameState.minigame?.active_bets) {
                for (const [playerId, bet] of Object.entries(crashGameState.minigame.active_bets)) {
                    const player = gameState.game?.players.find((p) => p.id.toString() === playerId);
                    if (!player) continue;

                    let profit = 0;
                    if (bet.cashed_out_at) {
                        // Player cashed out successfully
                        profit = Math.floor(bet.amount * bet.cashed_out_at - bet.amount);
                    } else {
                        // Player didn't cash out, lost their bet
                        profit = -bet.amount;
                    }

                    results.push({
                        playerId,
                        playerName: player.name,
                        profit,
                    });
                }
            }

            // Sort by profit (highest to lowest)
            results.sort((a, b) => b.profit - a.profit);
            finalResults.value = results;
            showFinalResults.value = true;
        }, 3000); // 3 second delay to show crash animation
    }
});
</script>

<style scoped>
@import url("https://fonts.googleapis.com/css2?family=Luckiest+Guy&display=swap");

:root {
    --primary-color: #ffd700;
    --secondary-color: #e6a100;
    --accent-color: #ffa048;
    --text-shadow: -2px -2px 0 white, 2px -2px 0 white, -2px 2px 0 white, 2px 2px 0 white;
    --box-shadow: 0 8px 0 rgba(0, 0, 0, 0.3);
    --border-radius: 20px;
    --font-primary: "Luckiest Guy", cursive;
    --font-secondary: "Baloo 2", cursive;
    --font-tertiary: "Fredoka", sans-serif;
}

/* Base Layout */
.crash-game {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    width: 100%;
    max-width: 1200px;
    margin: 0 auto;
    height: 100%;
    padding: 1rem;
    position: relative;
}

/* Common Card Styles */
.card {
    position: relative;
    width: 100%;
    aspect-ratio: 16/9;
    background: #1a237e;
    border-radius: 20px;
    overflow: hidden;
    border: 6px solid #ffd700;
    box-shadow:
        0 10px 30px rgba(0, 0, 0, 0.3),
        0 0 20px rgba(255, 215, 0, 0.3);
}

/* Game Title */
.game-title {
    font-family: var(--font-primary);
    font-size: 4.5rem;
    color: var(--primary-color);
    text-align: center;
    margin: 0 auto -20px;
    text-shadow:
        -2px -2px 0 var(--secondary-color),
        2px -2px 0 var(--secondary-color),
        -2px 2px 0 var(--secondary-color),
        2px 2px 0 var(--secondary-color),
        4px 4px 0 #b87d00,
        6px 6px 0 #8b5e00;
    transform: rotate(-2deg);
    transition: transform 0.3s ease;
    letter-spacing: 1px;
    font-weight: 400;
    -webkit-font-smoothing: antialiased;
    text-transform: uppercase;
}

.game-title:hover {
    transform: scale(1.02) rotate(-2deg);
}

/* Game Container */
.game-container {
    position: relative;
    width: 100%;
    aspect-ratio: 16/9;
    overflow: hidden;
}

.game-canvas {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    z-index: 2;
}

/* Common Overlay Styles */
.game-over,
.game-waiting,
.current-multiplier,
.overlay {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    z-index: 4;
    background: linear-gradient(135deg, #ff6b6b, #ff8e8e);
    border: 6px solid #ffd700;
    border-radius: 20px;
    padding: 2rem 3rem;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
    animation: float 3s ease-in-out infinite;
    backdrop-filter: blur(5px);
}

/* Game Over Specific Styles */
.game-over {
    background: linear-gradient(135deg, #ff4757, #ff6b81);
    border-color: #ffd700;
}

.crash-title {
    font-family: var(--font-primary);
    font-size: 3rem;
    color: white;
    text-shadow:
        3px 3px 0 rgba(0, 0, 0, 0.3),
        0 0 10px rgba(255, 215, 0, 0.5);
    margin-bottom: 1rem;
    animation: bounce 0.5s ease infinite;
}

.crash-value {
    font-family: var(--font-primary);
    font-size: 5rem;
    color: #ffd700;
    text-shadow:
        3px 3px 0 rgba(0, 0, 0, 0.3),
        0 0 20px rgba(255, 215, 0, 0.7);
    margin-bottom: 1rem;
}

.crash-face {
    font-size: 4rem;
    animation: shake 0.5s ease infinite;
}

/* Game Waiting Specific Styles */
.game-waiting {
    background: linear-gradient(135deg, #2ecc71, #27ae60);
    border-color: #ffd700;
}

.waiting-text {
    font-family: var(--font-primary);
    font-size: 2.5rem;
    color: white;
    text-shadow: 2px 2px 0 rgba(0, 0, 0, 0.3);
    margin-bottom: 1rem;
    text-align: center;
}

.players-bet {
    font-family: var(--font-secondary);
    font-size: 1.5rem;
    color: #fff;
    background: rgba(0, 0, 0, 0.2);
    padding: 0.5rem 1.5rem;
    border-radius: 20px;
    margin-bottom: 1rem;
    border: 4px solid rgba(255, 215, 0, 0.3);
}

.countdown-timer {
    font-family: var(--font-primary);
    font-size: 4rem;
    color: #ffd700;
    text-shadow: 3px 3px 0 rgba(0, 0, 0, 0.3);
    animation: countdown 1s ease infinite;
}

/* Current Multiplier Specific Styles */
.current-multiplier {
    background: linear-gradient(135deg, #4834d4, #686de0);
    border-color: #ffd700;
}

.multiplier-value {
    font-family: var(--font-primary);
    font-size: 4.5rem;
    color: #ffd700;
    text-shadow:
        3px 3px 0 rgba(0, 0, 0, 0.3),
        0 0 20px rgba(255, 215, 0, 0.7);
    margin-bottom: 0.5rem;
    animation: glow 2s ease-in-out infinite;
}

.multiplier-x {
    font-size: 3rem;
    margin-left: 0.2rem;
}

.current-payout {
    font-family: var(--font-secondary);
    font-size: 1.2rem;
    color: white;
    text-shadow: 1px 1px 0 rgba(0, 0, 0, 0.3);
    margin-bottom: 0.5rem;
}

.profit-display {
    font-family: var(--font-secondary);
    font-size: 1.5rem;
    color: #2ecc71;
    text-shadow: 1px 1px 0 rgba(0, 0, 0, 0.3);
}

/* New Animations */
@keyframes float {
    0%,
    100% {
        transform: translate(-50%, -50%) translateY(0);
    }
    50% {
        transform: translate(-50%, -50%) translateY(-10px);
    }
}

@keyframes bounce {
    0%,
    100% {
        transform: scale(1);
    }
    50% {
        transform: scale(1.05);
    }
}

@keyframes shake {
    0%,
    100% {
        transform: rotate(0deg);
    }
    25% {
        transform: rotate(-10deg);
    }
    75% {
        transform: rotate(10deg);
    }
}

@keyframes countdown {
    0%,
    100% {
        transform: scale(1);
    }
    50% {
        transform: scale(1.1);
    }
}

@keyframes glow {
    0%,
    100% {
        text-shadow:
            3px 3px 0 rgba(0, 0, 0, 0.3),
            0 0 20px rgba(255, 215, 0, 0.7);
    }
    50% {
        text-shadow:
            3px 3px 0 rgba(0, 0, 0, 0.3),
            0 0 30px rgba(255, 215, 0, 0.9);
    }
}

/* Game Controls */
.game-controls {
    width: 100%;
    background: linear-gradient(to bottom, #ff9c38, #ff6f1e);
    border: 6px solid #ffd700;
    border-radius: 20px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
    padding: 1.2rem;
    margin-bottom: 1.5rem;
}

.controls-wrapper {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1.5rem;
}

/* Bet Controls */
.bet-controls {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
}

.bet-input-wrapper {
    display: flex;
    position: relative;
    align-items: center;
}

.currency-symbol {
    position: absolute;
    left: 1rem;
    color: #222266;
    font-size: 1.2rem;
    z-index: 2;
}

.bet-input {
    flex: 1;
    padding: 0.8rem 3rem;
    font-size: 1.2rem;
    font-weight: 700;
    border: 4px solid #ffd700;
    border-radius: 20px;
    background: #f0f4ff;
    color: #222266;
    box-shadow:
        inset 0 0 0 4px rgba(84, 209, 255, 0.3),
        0 4px 0 rgba(0, 0, 0, 0.2);
    font-family: var(--font-secondary);
}

/* Quick Amount Buttons */
.bet-quick-amounts {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.8rem;
}

.quick-amount {
    flex: 1;
    padding: 0.5rem 0.3rem;
    font-size: 1rem;
    background: #54d1ff;
    color: white;
    border: 4px solid #ffd700;
    border-radius: 20px;
    font-weight: 700;
    font-family: var(--font-secondary);
    box-shadow: 0 4px 0 rgba(0, 0, 0, 0.2);
    text-shadow: 1px 1px 0 rgba(0, 0, 0, 0.3);
    transition: all 0.2s ease-in-out;
    cursor: pointer;
}

.quick-amount:hover {
    transform: translateY(-3px);
    box-shadow:
        0 7px 0 rgba(0, 0, 0, 0.2),
        0 0 20px rgba(84, 209, 255, 0.4);
    background: #6ad8ff;
}

.quick-amount:active {
    transform: translateY(2px);
    box-shadow: 0 2px 0 rgba(0, 0, 0, 0.2);
}

/* Action Buttons */
.action-button {
    min-width: 180px;
    height: 70px;
    font-size: 1.3rem;
    text-transform: uppercase;
    letter-spacing: 1px;
    border-radius: 20px;
    font-weight: 800;
    font-family: var(--font-secondary);
    position: relative;
    overflow: hidden;
    border: 4px solid #ffd700;
    box-shadow:
        0 8px 0 rgba(0, 0, 0, 0.3),
        0 0 20px rgba(255, 215, 0, 0.3);
    text-shadow: 2px 2px 0 rgba(0, 0, 0, 0.3);
    padding: 0.2rem 1.5rem;
    cursor: pointer;
    transition: all 0.2s ease-in-out;
}

.action-button:hover {
    transform: translateY(-4px) scale(1.02);
    box-shadow:
        0 12px 0 rgba(0, 0, 0, 0.3),
        0 0 30px rgba(255, 215, 0, 0.5);
}

.action-button:active {
    transform: translateY(2px);
    box-shadow:
        0 4px 0 rgba(0, 0, 0, 0.3),
        0 0 10px rgba(255, 215, 0, 0.2);
}

.bet-action {
    background: linear-gradient(to bottom, #7c5cff, #6045de);
}

.bet-action:hover {
    background: linear-gradient(to bottom, #8d71ff, #715af0);
}

.cashout-action {
    background: linear-gradient(to bottom, #53e37c, #36c55d);
    animation: pulse 0.7s infinite alternate;
}

.cashout-action:hover {
    background: linear-gradient(to bottom, #65f58e, #44d76b);
}

.next-action {
    background: linear-gradient(to bottom, #54d1ff, #30a8d5);
}

.next-action:hover {
    background: linear-gradient(to bottom, #6adbff, #42b6e0);
}

/* Animations */
@keyframes pulse {
    0% {
        transform: scale(1);
        box-shadow: 0 0 15px rgba(77, 255, 77, 0.5);
    }
    50% {
        transform: scale(1.05);
        box-shadow: 0 0 25px rgba(77, 255, 77, 0.8);
    }
    100% {
        transform: scale(1);
        box-shadow: 0 0 15px rgba(77, 255, 77, 0.5);
    }
}

@keyframes wiggle {
    0% {
        transform: rotate(-5deg);
    }
    100% {
        transform: rotate(5deg);
    }
}

/* Responsive Design */
@media (max-width: 768px) {
    .controls-wrapper {
        flex-direction: column;
        gap: 1.5rem;
    }

    .action-button {
        width: 100%;
    }
}

.cashed-out-overlay {
    position: absolute;
    top: 20px;
    left: 20px;
    background: rgba(0, 0, 0, 0.7);
    border: 6px solid #ffd700;
    border-radius: 20px;
    padding: 1.5rem;
    color: white;
    z-index: 10;
    text-align: center;
    animation: pulse 2s infinite;
    box-shadow:
        0 10px 30px rgba(0, 0, 0, 0.3),
        0 0 20px rgba(255, 215, 0, 0.3);
    backdrop-filter: blur(5px);
}

.cashout-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
}

.cashout-multiplier {
    font-size: 1.2rem;
    font-weight: bold;
    color: #4dff4d;
    text-shadow: 0 0 5px rgba(77, 255, 77, 0.7);
}

.cashout-profit {
    display: flex;
    flex-direction: column;
    align-items: center;
    font-size: 0.9rem;
}

.profit-label {
    color: #ffffff;
}

.profit-amount {
    font-weight: bold;
    color: gold;
    font-size: 1.1rem;
}

.current-time {
    position: absolute;
    bottom: 1rem;
    left: 50%;
    transform: translateX(-50%);
    font-size: 1rem;
    color: white;
    font-family: var(--font-tertiary);
    background: rgba(34, 34, 102, 0.7);
    padding: 0.4rem 1.5rem;
    border-radius: 20px;
    border: 4px solid #ffd700;
    z-index: 2;
    box-shadow: 0 4px 0 rgba(0, 0, 0, 0.2);
    font-weight: 600;
}

.time-label {
    font-weight: 700;
    color: #54d1ff;
    margin-right: 0.3rem;
}

.final-results-modal {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.8);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 1000;
    animation: fadeIn 0.3s ease-out;
}

.final-results-content {
    background: linear-gradient(to bottom, #ff9c38, #ff6f1e);
    border: 4px solid white;
    border-radius: 20px;
    padding: 2rem;
    width: 90%;
    max-width: 600px;
    max-height: 80vh;
    overflow-y: auto;
    box-shadow: 0 8px 0 rgba(0, 0, 0, 0.3);
    animation: slideUp 0.3s ease-out;
}

.final-results-title {
    font-family: var(--font-secondary);
    font-size: 2.5rem;
    color: white;
    text-align: center;
    margin-bottom: 1.5rem;
    text-shadow: 2px 2px 0 rgba(0, 0, 0, 0.3);
}

.final-results-list {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    margin-bottom: 2rem;
}

.result-item {
    background: rgba(255, 255, 255, 0.9);
    border: 3px solid white;
    border-radius: 12px;
    padding: 1rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    box-shadow: 0 4px 0 rgba(0, 0, 0, 0.2);
}

.player-name {
    font-family: var(--font-secondary);
    font-size: 1.2rem;
    color: #333;
    font-weight: 700;
}

.result-amount {
    font-family: var(--font-secondary);
    font-size: 1.4rem;
    font-weight: 800;
}

.result-amount.profit {
    color: #53e37c;
}

.result-amount.loss {
    color: #ff5252;
}

@keyframes fadeIn {
    from {
        opacity: 0;
    }
    to {
        opacity: 1;
    }
}

@keyframes slideUp {
    from {
        transform: translateY(20px);
        opacity: 0;
    }
    to {
        transform: translateY(0);
        opacity: 1;
    }
}
</style>
