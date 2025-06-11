<script setup lang="ts">
import { ref, computed, watch, watchEffect, onMounted, onBeforeUnmount } from "vue";
import { boardGameService, gameState } from "../game_data/game_data";
import { walletState } from "../utils/wallet";
import Chat from "../utils/Chat.vue";
import Header from "./Header.vue";

const playerName = ref(walletState?.wallet?.username ?? "Player");
const hasJoined = ref(false);
const status = ref("");

const lastInteractionTime = computed(() => {
    return gameState.game?.round_started_at || 0;
});

watchEffect(() => {
    hasJoined.value = !!gameState.game?.players.find((player) => player.name === walletState?.wallet?.username);
});

watch(
    () => walletState.sessionKey,
    (newSessionKey) => {
        if (newSessionKey && playerName.value === "Player") {
            playerName.value = walletState.wallet.username;
        }
    },
);

const gameIsOngoing = computed(() => {
    return gameState.game && gameState.game.phase !== "GameOver" && gameState.game.phase !== "Registration";
});

const timeLeft = ref(60);
let ticker;
onMounted(() => {
    timeLeft.value = Math.max(0, Math.round(60 - (Date.now() - lastInteractionTime.value) / 1000));
    ticker = setInterval(() => {
        timeLeft.value = Math.max(0, Math.round(60 - (Date.now() - lastInteractionTime.value) / 1000));
    }, 1000);
});
onBeforeUnmount(() => {
    clearInterval(ticker);
});

const canStartGame = computed(() => {
    if (!gameState.game) return false;
    return (slotsRemaining.value === 0 || timeLeft.value === 0) && gameState.game.phase === "Registration";
});

const registeredPlayers = computed(() => {
    if (!gameState.game) return [];
    return gameState.game.players;
});

const slotsRemaining = computed(() => {
    if (!gameState.game) return 0;
    return gameState.game.max_players - gameState.game.players.length;
});

const initAndJoinGame = async () => {
    if (!playerName.value) {
        alert("Please enter your name");
        return;
    }

    gameState.playerName = playerName.value;

    status.value = "game";
    // Create the game.
    await boardGameService.initGame();

    // Wait a bit for game to be created
    await new Promise((resolve) => setTimeout(resolve, 200));

    status.value = "register";

    // Register the player
    await boardGameService.registerPlayer(playerName.value);
    hasJoined.value = true;

    status.value = "done";

    // Temp Hack
    //for (let i = 0; i < playerCount.value - 1; i++) {
    //    await boardGameService.registerPlayer(`Ghost Player ${i + 1}`);
    //}
    //boardGameService.registerPlayer("Ghost Player 1");
    //boardGameService.registerPlayer("Ghost Player 2");
    //boardGameService.registerPlayer("Ghost Player 3");
};

const joinGame = async () => {
    if (!playerName.value) {
        alert("Please enter your name");
        return;
    }

    status.value = "register";

    gameState.playerName = playerName.value;
    await boardGameService.registerPlayer(playerName.value);
    hasJoined.value = true;
    status.value = "done";
};

const startGame = async () => {
    await boardGameService.startGame();
};

const reset = async () => {
    await boardGameService.reset();
};
</script>

<template>
    <div class="relative flex w-full min-h-[100vh] flex-col">
        <Header></Header>
        <div class="w-screen flex-1 bg-[#1A0C3B] flex flex-col md:flex-row">
            <!-- The orange trail section -->
            <div
                class="flex flex-col justify-center w-full md:w-[420px] bg-[#3B1C0C] border-b-4 md:border-b-0 md:border-r-4 mb-8 md:mb-0 border-[#FFC636] p-8 text-[#FFC636] shadow-2xl"
            >
                <h1 class="text-3xl font-bold mb-4 text-[#FFC636]">The Orange Trail</h1>
                <p class="mb-4 text-lg">
                    Welcome, Hylians! In <span class="font-bold">The Orange Trail</span>, you and your crew must cross
                    the perilous Martian desert to reach the colony. <br /><br />
                    <span class="italic hidden hmd:block"
                        >Will you thrive on Mars, or become another tale lost to the red sands?</span
                    >
                </p>
                <ul class="list-disc pl-6 text-base space-y-2">
                    <li>Connect your wallet and enter your name to join.</li>
                    <li>Each day, bet your resources - wisely!</li>
                    <li>Face a random hazard</li>
                    <li>Reach the colony to win!</li>
                </ul>
            </div>
            <!-- Main game/chat UI -->
            <div class="flex-1 flex">
                <div class="flex flex-col items-center justify-around w-full h-full">
                    <div class="w-full max-w-md bg-[#2A1C4B] rounded-xl m-8 p-8 border-6 border-[#FFC636] shadow-2xl">
                        <div v-if="!walletState.sessionKey">
                            <h1 class="text-3xl font-bold text-[#FFC636]">Connect your wallet</h1>
                            <p class="text-gray-400">Please connect your wallet to play the game.</p>
                        </div>
                        <div v-else-if="!gameIsOngoing && gameState.game?.phase === 'GameOver'" class="space-y-6">
                            <div class="space-y-2">
                                <label class="block text-[#FFC636]">Your Name</label>
                                <input
                                    v-model="playerName"
                                    type="text"
                                    class="w-full px-4 py-2 rounded-lg bg-[#1A0C3B] border-2 border-[#FFC636] text-white"
                                    placeholder="Enter your name"
                                />
                            </div>

                            <button
                                @click="initAndJoinGame"
                                :disabled="status !== ''"
                                class="w-full py-3 rounded-lg bg-[#FFC636] text-[#1A0C3B] font-bold hover:bg-[#FFD666] transition-colors disabled:opacity-50"
                            >
                                Create & Join Game
                            </button>

                            <p v-if="status === 'game'" class="text-red-500">Creating game...</p>
                            <p v-if="status === 'register'" class="text-red-500">Registering player...</p>
                        </div>

                        <div v-else-if="!gameIsOngoing" class="space-y-6">
                            <div v-if="!hasJoined && slotsRemaining > 0" class="space-y-4">
                                <div class="space-y-2">
                                    <label class="block text-[#FFC636]">Your Name</label>
                                    <input
                                        v-model="playerName"
                                        type="text"
                                        class="w-full px-4 py-2 rounded-lg bg-[#1A0C3B] border-2 border-[#FFC636] text-white"
                                        placeholder="Enter your name"
                                    />
                                </div>

                                <button
                                    @click="joinGame"
                                    :disabled="!playerName || status !== ''"
                                    class="w-full py-3 rounded-lg bg-[#FFC636] text-[#1A0C3B] font-bold hover:bg-[#FFD666] transition-colors disabled:opacity-50"
                                >
                                    Join Game
                                </button>

                                <p v-if="status === 'game'" class="text-red-500">Creating game...</p>
                                <p v-if="status === 'register'" class="text-red-500">Registering player...</p>
                            </div>

                            <div class="space-y-4">
                                <h2 class="text-2xl font-bold text-[#FFC636]">
                                    Players (Up to {{ slotsRemaining }} can join)
                                </h2>
                                <ul class="space-y-2">
                                    <li
                                        v-for="player in registeredPlayers"
                                        :key="player.id"
                                        class="px-4 py-2 bg-[#1A0C3B] rounded-lg flex items-center justify-between"
                                    >
                                        <span>{{ player.name }}</span>
                                        <span class="text-[#FFC636]">Ready!</span>
                                    </li>
                                </ul>

                                <button
                                    v-if="hasJoined"
                                    @click="startGame"
                                    :disabled="!canStartGame"
                                    class="w-full py-3 rounded-lg bg-[#FFC636] text-[#1A0C3B] font-bold hover:bg-[#FFD666] transition-colors disabled:opacity-50"
                                >
                                    {{ canStartGame ? "Start Game!" : `Waiting ${timeLeft}s for more players` }}
                                </button>
                            </div>
                        </div>
                        <div v-else-if="gameIsOngoing">
                            <h2 class="text-2xl font-bold text-[#FFC636]">Current players</h2>
                            <ul class="space-y-2">
                                <li
                                    v-for="player in registeredPlayers"
                                    :key="player.id"
                                    class="px-4 py-2 bg-[#1A0C3B] rounded-lg flex items-center justify-between"
                                >
                                    <span>{{ player.name }}</span>
                                </li>
                            </ul>

                            <button
                                @click="reset"
                                :disabled="!playerName"
                                class="w-full mt-8 py-3 rounded-lg bg-[#36C6FF] text-[#1A0C3B] font-bold hover:bg-[#D666FF] transition-colors disabled:opacity-50"
                            >
                                Start a new game
                            </button>
                        </div>
                    </div>
                    <!--<div class="hidden md:flex xl:hidden items-center justify-center px-8 text-white">
                        <Chat class="bg-[#2A1C4B] rounded-xl shadow-2xl min-h-[calc(min(400px,100vh))]" />
                    </div>
                    -->
                </div>
                <!--
                <div class="hidden xl:flex items-center justify-center h-full px-8 text-white">
                    <Chat class="bg-[#2A1C4B] rounded-xl shadow-2xl min-h-[calc(min(600px,100vh))]" />
                </div>
                -->
            </div>
        </div>
    </div>
</template>
