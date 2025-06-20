<script setup lang="ts">
import { ref, computed, onMounted, watch, onUnmounted } from "vue";
import {
    gameState,
    playerColor,
    playerAvatar,
    getLocalPlayerId,
    boardGameService,
    isCurrentPlayer,
    isObserver,
} from "../game_data/game_data";
import Backdrop from "./Backdrop.vue";
import { addIdentityToMessage } from "../game_data/auth";
import PlayerBar from "./PlayerBar.vue";
import { animState, isAnimationPlayed, markAnimationPlayed, markAnimationPlayedIn } from "./animState";
import SpinningWheel from "./SpinningWheel.vue";
import { playLoopingSound } from "../utils/audio";
import Chat from "../utils/Chat.vue";

////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////
// Betting stuff
////////////////////////////////////////////////////////////////////////
const BET_OPTIONS = [5, 10, 25, 50];
const placingBet = ref(false);
const customBet = ref(25);
const customBetError = ref("");

const currentGame = computed(() => gameState.game);
const localPlayerId = getLocalPlayerId();

const bets = computed(() => currentGame.value?.bets || {});
const players = computed(() => currentGame.value?.players || []);

const hasBet = computed(() => bets.value[localPlayerId] !== undefined);
const localPlayer = computed(() => players.value.find((p) => p.id === localPlayerId));

const allOrNothing = computed(() => currentGame.value?.all_or_nothing);

function placeBet(amount: number) {
    if (placingBet.value || hasBet.value) return;
    if (allOrNothing.value) {
        boardGameService.sendAction({ PlaceBet: { amount: localPlayer.value!.coins } }).finally(() => {
            placingBet.value = false;
        });
        return;
    }
    placingBet.value = true;
    boardGameService.sendAction({ PlaceBet: { amount } }).finally(() => {
        placingBet.value = false;
    });
    markAnimationPlayedIn("DoneBetting", 0.8);
}

function validateCustomBet(): boolean {
    const value = Number(customBet.value);
    if (!localPlayer.value) return false;
    if (!value || isNaN(value) || value <= 0) {
        customBetError.value = "Enter a positive number";
        return false;
    }
    if (value > localPlayer.value.coins) {
        customBetError.value = "Not enough coins";
        return false;
    }
    customBetError.value = "";
    return true;
}

function placeCustomBet() {
    if (!validateCustomBet() || placingBet.value || hasBet.value) return;
    placeBet(Number(customBet.value));
}

function setCustomBet(amount: number) {
    if (hasBet.value || placingBet.value || (localPlayer.value && amount > localPlayer.value.coins)) return;
    customBet.value = amount;
    validateCustomBet();
}

const betScreenActive = computed(() => {
    return !isObserver() && !isAnimationPlayed("DoneBetting");
});

const timer = ref(30);
const timerInterval = ref<number | null>(null);

// --- TICKER SOUND EFFECT (Web Audio API) ---
let tickerHandle: { stop: () => void; setVolume: (v: number) => void; setRate: (r: number) => void } | null = null;
let tickerActive = false;

function startTicker() {
    if (tickerHandle) tickerHandle.stop();
    let volume = getTickerVolume();
    let rate = getTickerRate();
    tickerHandle = playLoopingSound("tick", volume, rate);
    tickerActive = true;
}

function stopTicker() {
    tickerActive = false;
    if (tickerHandle) {
        tickerHandle.stop();
        tickerHandle = null;
    }
}

function getTickerVolume() {
    if (timer.value <= 3) return 0.7;
    if (timer.value <= 10) return 0.7;
    if (timer.value <= 20) return 0.4;
    return 0.0;
}

function getTickerRate() {
    if (timer.value <= 4) return 2.0;
    if (timer.value <= 10) return 1.5;
    return 1.3;
}

watch(
    () => timer.value,
    (newVal, oldVal) => {
        if (betScreenActive.value && newVal > 0 && !tickerActive) {
            startTicker();
        }
        if ((!betScreenActive.value || newVal <= 0 || hasBet.value) && tickerActive) {
            stopTicker();
        }
        // Adjust volume and rate live if ticker is active
        if (tickerActive && tickerHandle) {
            tickerHandle.setVolume(getTickerVolume());
            tickerHandle.setRate(getTickerRate());
        }
    },
    { immediate: true },
);

onMounted(() => {
    timer.value = 30;
    if (timerInterval.value) clearInterval(timerInterval.value);
    timerInterval.value = setInterval(() => {
        timer.value = Math.round(Math.max(0, 30 - (Date.now() - currentGame.value!.round_started_at) / 1000) * 10) / 10;
        if (timer.value <= 0 && !isAnimationPlayed("BettingTimeUp")) {
            timer.value = 0;
            markAnimationPlayedIn("BettingTimeUp", 0.5);
        }
    }, 100) as unknown as number;
});

watch(
    () => currentGame.value?.phase,
    (phase) => {
        if (typeof phase === "object" && "FinalMinigame" in phase) {
            markAnimationPlayed("FinalMinigameRound");
            markAnimationPlayed("SpinWheel");
        }
    },
    {
        immediate: true,
    },
);

onUnmounted(() => {
    stopTicker();
});
</script>

<template>
    <div class="flex flex-col w-full">
        <div class="flex h-full w-full">
            <div class="flex-1 relative">
                <div
                    class="absolute top-0 px-8 z-1000 font-['Luckiest_Guy'] text-[2.5rem] sm:text-[4.5rem] text-[#ffd700] w-full text-center sm:text-left mx-auto text-shadow-[_-2px_-2px_0_#e6a100,_2px_-2px_0_#e6a100,_-2px_2px_0_#e6a100,_2px_2px_0_#e6a100,_4px_4px_0_#b87d00,_6px_6px_0_#8b5e00] rotate-[-2deg] transition-transform duration-300 ease-in-out tracking-wide font-normal antialiased uppercase hover:scale-102 hover:rotate-[-2deg]"
                >
                    ORANGE TRAIL
                    <img
                        src="/src/assets/trail_truck.png"
                        alt="Orange Trail Truck"
                        class="inline-block h-[4.5rem] ml-4"
                    />
                </div>

                <Backdrop class="absolute w-full h-full" />

                <!-- Left-side items -->
                <div class="absolute top-24 left-0 h-[calc(100%-6rem)] flex">
                    <div class="hidden xl:hidden flex-col items-start justify-between gap-4 p-4">
                        <div
                            v-show="!isAnimationPlayed('FinalMinigameRound')"
                            :class="`z-10 bg-widget p-8
                    rounded-[2rem] flex flex-col items-center justify-center gap-6 min-w-[320px] ${betScreenActive ? ' hidden md:flex ' : ''}`"
                        >
                            <SpinningWheel />
                        </div>
                    </div>
                </div>
                <!-- Right-side items -->
                <div class="absolute top-0 right-0 h-full flex">
                    <div class="flex xl:flex flex-col items-end justify-end md:justify-between gap-4 p-4">
                        <div
                            v-show="!isAnimationPlayed('FinalMinigameRound')"
                            :class="`z-10 bg-widget p-8
                    rounded-[2rem] flex flex-col items-center justify-center gap-6 min-w-[320px] ${betScreenActive ? ' hidden md:flex ' : ''}`"
                        >
                            <SpinningWheel />
                        </div>
                        <!-- Bet Controls & Timer -->
                        <div
                            v-if="betScreenActive"
                            :class="`z-10 bg-widget p-8 rounded-[2rem] flex flex-col items-center justify-center gap-4
                    min-w-[320px] mx-auto transition-all transition-duration-500 ${betScreenActive ? '' : ' hidden md:flex '}`"
                        >
                            <div v-if="!hasBet && timer > 0" class="relative mb-4">
                                <div class="text-2xl font-bold text-white mb-2">
                                    Place your bet. Only {{ timer }}s left!
                                </div>
                                <div
                                    v-if="allOrNothing"
                                    class="text-red-500 font-bold text-lg mt-4 flex justify-between items-center"
                                >
                                    ALL OR NOTHING!
                                    <button
                                        @click="placeCustomBet"
                                        :disabled="hasBet || placingBet || !validateCustomBet()"
                                        class="px-4 py-2 rounded-xl font-bold text-lg border-4 border-white shadow-md transition-all duration-150 hover:-translate-y-1 hover:shadow-lg focus:outline-none bg-gradient-to-b from-[#FF4D4D] to-[#CC0000] text-white"
                                    >
                                        Bet {{ localPlayer?.coins }} 💰
                                    </button>
                                </div>
                                <template v-else>
                                    <div class="flex items-stretch gap-3">
                                        <button
                                            v-for="option in BET_OPTIONS"
                                            :key="option"
                                            :disabled="
                                                hasBet ||
                                                placingBet ||
                                                (localPlayer &&
                                                    (option > localPlayer.coins ||
                                                        (allOrNothing && option !== localPlayer.coins)))
                                            "
                                            @click="setCustomBet(option)"
                                            class="px-6 py-3 rounded-xl font-bold text-lg border-4 border-white shadow-md transition-all duration-150 hover:-translate-y-1 hover:shadow-lg focus:outline-none"
                                            :class="{
                                                'bg-gradient-to-b from-[#4DAAFF] to-[#0077CC] text-white': !hasBet,
                                                'bg-gradient-to-b from-[#999999] to-[#666666] text-white opacity-50':
                                                    hasBet ||
                                                    placingBet ||
                                                    (localPlayer &&
                                                        (option > localPlayer.coins ||
                                                            (allOrNothing && option !== localPlayer.coins))),
                                            }"
                                        >
                                            {{ option }} 🪙
                                        </button>
                                    </div>
                                    <div class="flex gap-3 mt-3">
                                        <!-- Custom bet input -->
                                        <input
                                            v-model="customBet"
                                            type="number"
                                            min="1"
                                            :max="localPlayer?.coins || 1"
                                            :disabled="hasBet || placingBet || allOrNothing"
                                            placeholder="Custom"
                                            class="flex-1 px-3 py-2 rounded-xl border-2 border-[#ffd700] bg-white text-[#8B0000] font-bold text-lg focus:outline-none"
                                            @input="validateCustomBet"
                                        />
                                        <button
                                            @click="placeCustomBet"
                                            :disabled="hasBet || placingBet || !validateCustomBet()"
                                            class="px-4 py-2 rounded-xl font-bold text-lg border-4 border-white shadow-md transition-all duration-150 hover:-translate-y-1 hover:shadow-lg focus:outline-none bg-gradient-to-b from-[#FF4D4D] to-[#CC0000] text-white disabled:from-[#999999] disabled:to-[#777] disabled:opacity-50"
                                        >
                                            Bet
                                        </button>
                                    </div>
                                </template>
                                <div
                                    v-if="customBetError && !hasBet"
                                    class="text-red-400 text-center font-bold text-sm mb-2 absolute bottom-0 translate-y-[150%] w-full"
                                >
                                    {{ customBetError }}
                                </div>
                            </div>
                            <div v-else-if="hasBet" class="text-green-400 font-bold text-2xl">Bet placed!</div>
                            <div v-else class="text-red-400 font-bold text-2xl">Time's up! You lost 10 coins.</div>
                        </div>
                    </div>
                    <Chat
                        class="hidden lg:flex relative z-10 bg-widget p-2 flex-1 flex-col rounded-[2rem] m-4 ml-0 overflow-y-scroll"
                    />
                </div>
            </div>
        </div>
        <PlayerBar class="hidden hmd:flex max-h-[200px] overflow-x-scroll">
            <template #default="{ player }">
                <div v-if="bets[player.id] !== undefined" class="text-green-300 font-bold text-xs">
                    Bet: {{ bets[player.id] }} 🪙
                </div>
                <div v-else class="text-gray-300 italic text-xs">No bet yet</div>
            </template>
        </PlayerBar>
    </div>
</template>

<style scoped>
@keyframes wiggle {
    0% {
        transform: rotate(-7deg);
    }
    100% {
        transform: rotate(7deg);
    }
}
.animate-avatar {
    animation: wiggle 1.2s infinite alternate;
}

.bg-widget {
    background-image: url("/src/assets/text.jpg");
    background-size: cover;
    background-position: top left;

    border: 4px solid #ffa048;
}
</style>
