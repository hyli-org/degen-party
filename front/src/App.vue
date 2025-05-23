<script setup lang="ts">
import {
    isCurrentPlayer,
    gameState,
    DEFAULT_PLAYERS,
    playerColor,
    playerAvatar,
    getLocalPlayerId,
} from "./game_data/game_data";
import { ref, computed } from "vue";
import { useRoute } from "vue-router";
import Lobby from "./components/Lobby.vue";
import { wsState } from "./utils/shared-websocket";

import { addIdentityToMessage } from "./game_data/auth";
import { HyliWalletElement } from "wallet-wrapper";
import { TestnetChatElement } from "hyli-testnet-chat";
import { watchEffect } from "vue";
import { animState } from "./components/animState";
import { onWalletReady, walletConfig, sessionKeyConfig } from "./utils/wallet";

customElements.define("testnet-chat", TestnetChatElement);
customElements.define("hyli-wallet", HyliWalletElement);

const route = useRoute();
const routeFullPath = ref(route.fullPath);
watchEffect(() => {
    console.log("Route changed:", route.fullPath);
    routeFullPath.value = route.fullPath;
});

const showChat = ref(false);
const toggleChat = () => {
    showChat.value = !showChat.value;
};
const nodeUrl = window.location.hostname === "localhost" ? "http://localhost:4321" : "https://node.testnet.hyli.org";
const indexerUrl =
    window.location.hostname === "localhost" ? "http://localhost:4321" : "https://indexer.testnet.hyli.org";

const players = computed(() => {
    if (!gameState?.game?.players?.length) return DEFAULT_PLAYERS;
    return gameState.game.players;
});

const currentTurn = computed(() => {
    if (!gameState.game) return 1;
    return gameState.game.round + 1;
});

const countdown = ref(60); // Seconds left in mini-game
const isGameOver = ref(false);

const returnToLobby = () => {
    gameState.isInLobby = true;
};

const connectionStatusColor = computed(() => {
    if (wsState.connected) return "bg-green-500";
    if (wsState.connectionStatus.includes("Reconnecting")) return "bg-yellow-500";
    return "bg-red-500";
});
</script>

<template>
    <div class="relative flex h-screen w-full flex-col overflow-hidden">
        <div class="stars-background"></div>
        <div class="clouds-background"></div>
        <div class="grass-overlay"></div>

        <header
            class="relative z-10 flex items-center justify-between border-b-[5px] border-white px-6 py-3 shadow-lg"
            style="background: linear-gradient(to bottom, #ff7a7a, var(--mario-red))"
        >
            <div class="flex items-center gap-2">
                <span class="text-4xl mushroom-icon">🍄</span>
                <h1 class="game-logo -rotate-2 text-4xl font-extrabold uppercase tracking-wider text-white">
                    Degen Party
                </h1>
                <span class="text-4xl mushroom-icon">🍄</span>
            </div>

            <div class="flex items-center gap-4">
                <button
                    @click="toggleChat"
                    class="px-4 py-2 rounded-full border-3 border-white bg-black/20 font-bold text-white hover:bg-black/30 transition-colors"
                >
                    <span v-if="showChat">Hide Chat</span>
                    <span v-else>Show Chat</span>
                </button>
                <hyli-wallet
                    :config="walletConfig"
                    :sessionKeyConfig="sessionKeyConfig"
                    :providers="['password', 'google']"
                    :toto="sessionKeyConfig?.whitelist?.[0]"
                    @walletUpdate="onWalletReady"
                ></hyli-wallet>
                <div
                    class="connection-status flex items-center gap-2 px-4 py-2 rounded-full border-3 border-white bg-black/20"
                >
                    <div class="flex items-center gap-2">
                        <div class="h-3 w-3 rounded-full animate-pulse" :class="connectionStatusColor"></div>
                        <span class="text-white text-sm font-bold">
                            {{ wsState.connectionStatus }}
                        </span>
                    </div>
                </div>
                <template v-if="!gameState.isInLobby">
                    <button
                        @click="returnToLobby"
                        class="px-4 py-2 rounded-full border-3 border-white bg-black/20 font-bold text-white hover:bg-black/30 transition-colors"
                    >
                        Return to Lobby
                    </button>
                    <div
                        class="turn-counter rounded-full border-3 border-white bg-black/20 px-4 py-2 font-bold text-white"
                    >
                        Turn
                        <span class="font-baloo text-2xl font-extrabold text-[var(--star-yellow)]">
                            {{ animState.currentRoundIndex + 1 }}
                        </span>
                    </div>
                </template>
            </div>
        </header>

        <testnet-chat
            v-if="showChat"
            class="fixed top-[5rem] right-0 bg-white rounded-[20px]"
            :nickname="getLocalPlayerId()"
            :processBlobTx="addIdentityToMessage"
            :node_url="nodeUrl"
            :indexer_url="indexerUrl"
        ></testnet-chat>

        <RouterView :key="routeFullPath" />
    </div>
</template>

<style>
@import url("https://fonts.googleapis.com/css2?family=Baloo+2:wght@400;500;600;700;800&family=Fredoka:wght@300;400;500;600;700&display=swap");

:root {
    --primary: #ff3a88;
    --primary-light: #ff6aaa;
    --mario-red: #e52521;
    --yoshi-green: #6fbd43;
    --peach-pink: #f699cd;
    --luigi-green: #00a651;
    --bowser-yellow: #fbd000;
    --toad-blue: #009bde;
    --mario-blue: #0e67b4;
    --sand-path: #e8d1a0;
    --grass-light: #8fe767;
    --grass-dark: #6fbd43;
    --sky-blue: #7ecef4;
    --star-yellow: #ffdd55;
    --main-bg: #87ceeb;
    --path-color: #f4d56a;
    --grass-color: #42b045;
    --shadow-color: rgba(0, 0, 0, 0.3);
}

body {
    margin: 0;
    padding: 0;
    background: var(--main-bg);
    color: #333;
    font-family: "Fredoka", sans-serif;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
    line-height: 1.6;
}

.stars-background {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background-image: url("data:image/svg+xml,%3Csvg width='100' height='100' viewBox='0 0 100 100' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M50 0 L52 47 L100 50 L52 53 L50 100 L48 53 L0 50 L48 47 Z' fill='%23FFD700' fill-opacity='0.1'/%3E%3C/svg%3E");
    background-size: 100px 100px;
    z-index: -3;
    animation: twinkling 10s linear infinite;
}

.clouds-background {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background-image:
        url("data:image/svg+xml,%3Csvg width='200' height='100' viewBox='0 0 200 100' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M30 40 C10 40 0 60 20 70 C0 80 10 100 30 90 C50 100 70 80 50 70 C70 60 60 40 30 40 Z' fill='white' fill-opacity='0.7'/%3E%3C/svg%3E"),
        url("data:image/svg+xml,%3Csvg width='300' height='150' viewBox='0 0 300 150' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M50 70 C20 70 0 100 30 120 C0 140 20 180 50 160 C80 180 110 140 80 120 C110 100 80 70 50 70 Z' fill='white' fill-opacity='0.5'/%3E%3C/svg%3E");
    background-size:
        300px 150px,
        500px 250px;
    background-position:
        50% 20%,
        20% 70%;
    z-index: -2;
    animation: floating 40s linear infinite;
}

.grass-overlay {
    position: fixed;
    bottom: 0;
    left: 0;
    width: 100%;
    height: 25%;
    background: var(--grass-color);
    border-top: 10px solid #378e37;
    z-index: -1;
}

.grass-overlay::before {
    content: "";
    position: absolute;
    top: -20px;
    left: 0;
    width: 100%;
    height: 20px;
    background-image:
        linear-gradient(45deg, var(--grass-color) 50%, transparent 50%),
        linear-gradient(-45deg, var(--grass-color) 50%, transparent 50%);
    background-size: 20px 20px;
    background-repeat: repeat-x;
}

.game-logo {
    text-shadow:
        -2px -2px 0 #000,
        2px -2px 0 #000,
        -2px 2px 0 #000,
        2px 2px 0 #000,
        0 4px 0 rgba(0, 0, 0, 0.3);
}

@keyframes bounce {
    0% {
        transform: translateY(0);
    }
    100% {
        transform: translateY(-5px);
    }
}

@keyframes spin {
    from {
        transform: rotate(0deg);
    }
    to {
        transform: rotate(360deg);
    }
}

@keyframes twinkling {
    0%,
    100% {
        opacity: 0.3;
    }
    50% {
        opacity: 0.8;
    }
}

@keyframes floating {
    0%,
    100% {
        background-position:
            50% 20%,
            20% 70%;
    }
    50% {
        background-position:
            55% 20%,
            15% 70%;
    }
}

.mushroom-icon {
    animation: bounce 1.5s ease-in-out infinite alternate;
}

.coin-icon {
    animation: spin 3s linear infinite;
}

.active-marker {
    animation: bounce 0.5s infinite alternate;
    filter: drop-shadow(0 0 5px rgba(255, 215, 0, 0.5));
}

.game-result {
    text-shadow: 2px 2px 0 rgba(0, 0, 0, 0.5);
    animation: pulse 1.5s infinite;
}

@keyframes pulse {
    0%,
    100% {
        opacity: 1;
    }
    50% {
        opacity: 0.6;
    }
}

.connection-status {
    transition: all 0.3s ease;
}

.connection-status:hover {
    transform: translateY(-1px);
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
}

.wooden-border {
    content: "";
    position: absolute;
    top: -6px;
    left: -6px;
    right: -6px;
    bottom: -6px;
    background: repeating-linear-gradient(45deg, #e67e22, #e67e22 15px, #d35400 15px, #d35400 30px);
    border-radius: 30px;
    z-index: -1;
}

.wooden-border::before {
    content: "";
    position: absolute;
    top: 6px;
    left: 6px;
    right: 6px;
    bottom: 6px;
    border: 8px solid #d35400;
    border-radius: 24px;
    box-shadow: inset 0 0 15px rgba(0, 0, 0, 0.4);
    pointer-events: none;
    z-index: 5;
}

.game-header {
    border: 6px solid #e67e22;
    box-shadow:
        inset 0 0 15px rgba(0, 0, 0, 0.2),
        0 8px 0 rgba(0, 0, 0, 0.3);
}

.game-header::before {
    border: 4px solid #d35400;
    border-radius: 16px;
}
</style>
