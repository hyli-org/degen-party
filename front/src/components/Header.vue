<script setup lang="ts">
import { gameState } from "../game_data/game_data";
import { computed } from "vue";
import { wsState } from "../utils/shared-websocket";

import { animState } from "../components/animState";
import { onWalletReady, walletConfig } from "../utils/wallet";

const sessionKeyConfig = computed(() => {
    let ret = {
        duration: 60 * 60 * 24 * 7 * 1000,
        whitelist: ["testnet_chat", gameState.board_game_contract, gameState.crash_game_contract],
    };
    return ret;
});

const connectionStatusColor = computed(() => {
    if (wsState.connected) return "bg-green-500";
    if (wsState.connectionStatus.includes("Reconnecting")) return "bg-yellow-500";
    return "bg-red-500";
});
</script>

<template>
    <header
        class="relative z-10 flex items-center justify-between border-b-[5px] border-white px-6 py-3 shadow-lg"
        style="background: linear-gradient(to bottom, #ff7a7a, var(--mario-red))"
    >
        <div class="flex items-center gap-2 -rotate-2">
            <h1 class="game-logo text-4xl font-extrabold uppercase tracking-wider text-white">Orange Trail</h1>
            <img src="/src/assets/trail_truck.png" alt="Trail Truck" class="h-12 w-12 image-bounce" />
        </div>

        <div class="flex items-center gap-4">
            <hyli-wallet
                :config="walletConfig"
                :sessionKeyConfig="sessionKeyConfig"
                :providers="['password', 'google']"
                :forceSessionKey="true"
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
                <div class="turn-counter rounded-full border-3 border-white bg-black/20 px-4 py-2 font-bold text-white">
                    Turn
                    <span class="font-baloo text-2xl font-extrabold text-[var(--star-yellow)]">
                        {{ animState.currentRoundIndex + 1 }}
                    </span>
                </div>
            </template>
        </div>
    </header>
</template>

<style>
.game-logo {
    text-shadow:
        -2px -2px 0 #000,
        2px -2px 0 #000,
        -2px 2px 0 #000,
        2px 2px 0 #000,
        0 4px 0 rgba(0, 0, 0, 0.3);
}

.image-bounce {
    animation: bounce 1.5s ease-in-out infinite alternate;
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
