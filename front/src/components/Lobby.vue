<script setup lang="ts">
import { ref, computed } from "vue";
import { boardGameService, gameState } from "../game_data/game_data";
import { walletState } from "../utils/wallet";
import { authService } from "../game_data/auth";

const playerName = ref("Player");
const playerCount = ref(4);
const hasJoined = ref(false);
const password = ref("");

const status = ref("");

const canStartGame = computed(() => {
    if (!gameState.game) return false;
    return gameState.game.phase === "Registration";
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

    /*
    password: string,
    expiration: number,
    whitelist: string[],
    onWalletEvent?: WalletEventCallback,
    onError?: WalletErrorCallback,
    */
    status.value = "sess_key";
    const { sessionKey } = await walletState.registerSessionKey(password.value, 60 * 60 * 24, [
        "board_game_2",
        "crash_game_2",
    ]);
    authService.reload(sessionKey.privateKey, sessionKey.publicKey);

    status.value = "game";
    // Create the game.
    await boardGameService.initGame({
        playerCount: playerCount.value,
    });

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

    status.value = "sess_key";
    const { sessionKey } = await walletState.registerSessionKey(password.value, 60 * 60 * 24, [
        "board_game_2",
        "crash_game_2",
    ]);
    authService.reload(sessionKey.privateKey, sessionKey.publicKey);

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
    <div class="flex flex-col items-center justify-center min-h-screen bg-[#1A0C3B] text-white p-8">
        <div class="w-full max-w-md bg-[#2A1C4B] rounded-xl p-8 border-6 border-[#FFC636] shadow-2xl">
            <div v-if="!gameState.game || gameState.game.phase === 'GameOver'" class="space-y-6">
                <div class="space-y-2">
                    <label class="block text-[#FFC636]">Your Name</label>
                    <input
                        v-model="playerName"
                        type="text"
                        class="w-full px-4 py-2 rounded-lg bg-[#1A0C3B] border-2 border-[#FFC636] text-white"
                        placeholder="Enter your name"
                    />
                </div>

                <div class="space-y-2">
                    <label class="block text-[#FFC636]">Password</label>
                    <input
                        v-model="password"
                        type="password"
                        class="w-full px-4 py-2 rounded-lg bg-[#1A0C3B] border-2 border-[#FFC636] text-white"
                        placeholder="Enter password"
                    />
                </div>

                <div class="space-y-2">
                    <label class="block text-[#FFC636]">Number of Players</label>
                    <select
                        v-model="playerCount"
                        class="w-full px-4 py-2 rounded-lg bg-[#1A0C3B] border-2 border-[#FFC636] text-white"
                    >
                        <option value="1">1 Player</option>
                        <option value="2">2 Players</option>
                        <option value="3">3 Players</option>
                        <option value="4">4 Players</option>
                    </select>
                </div>

                <button
                    @click="initAndJoinGame"
                    :disabled="status !== ''"
                    class="w-full py-3 rounded-lg bg-[#FFC636] text-[#1A0C3B] font-bold hover:bg-[#FFD666] transition-colors disabled:opacity-50"
                >
                    Create & Join Game
                </button>

                <p v-if="status === 'sess_key'" class="text-red-500">Creating session key...</p>
                <p v-if="status === 'game'" class="text-red-500">Creating game...</p>
                <p v-if="status === 'register'" class="text-red-500">Registering player...</p>
            </div>

            <div v-else class="space-y-6">
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

                    <div class="space-y-2">
                        <label class="block text-[#FFC636]">Password</label>
                        <input
                            v-model="password"
                            type="password"
                            class="w-full px-4 py-2 rounded-lg bg-[#1A0C3B] border-2 border-[#FFC636] text-white"
                            placeholder="Enter password"
                        />
                    </div>

                    <button
                        @click="joinGame"
                        :disabled="!playerName || status !== ''"
                        class="w-full py-3 rounded-lg bg-[#FFC636] text-[#1A0C3B] font-bold hover:bg-[#FFD666] transition-colors disabled:opacity-50"
                    >
                        Join Game
                    </button>

                    <p v-if="status === 'sess_key'" class="text-red-500">Creating session key...</p>
                    <p v-if="status === 'game'" class="text-red-500">Creating game...</p>
                    <p v-if="status === 'register'" class="text-red-500">Registering player...</p>
                </div>

                <div class="space-y-4">
                    <h2 class="text-2xl font-bold text-[#FFC636]">Players ({{ slotsRemaining }} slots remaining)</h2>
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
                        {{ canStartGame ? "Start Game!" : "Waiting for players..." }}
                    </button>
                    <button
                        v-else
                        @click="reset"
                        :disabled="!playerName"
                        class="w-full py-3 rounded-lg bg-[#36C6FF] text-[#1A0C3B] font-bold hover:bg-[#D666FF] transition-colors disabled:opacity-50"
                    >
                        Start a new game
                    </button>
                </div>
            </div>
        </div>
    </div>
</template>
