<script setup lang="ts">
import { computed } from "vue";
import { gameState, playerColor, playerAvatar } from "../game_data/game_data";

const currentGame = computed(() => gameState.game);
const players = computed(() => currentGame.value?.players || []);
</script>

<template>
    <div class="w-full bg-[#6B0000]/95 border-t-4 border-[#ffa048] flex justify-center z-50 py-3 px-2 gap-4">
        <div v-for="player in players" :key="player.id" class="flex flex-col items-center mx-2 min-w-[80px]">
            <div
                class="w-14 h-14 flex items-center justify-center text-3xl rounded-full border-4 border-white mb-1 animate-avatar"
                :style="{ backgroundColor: playerColor(player.id) }"
            >
                {{ playerAvatar(player.id) }}
            </div>
            <div class="font-bold text-xs text-white text-center mb-0.5 w-16">
                {{ player.name }}<br />{{ player.coins }}💰
            </div>
            <slot :player="player" />
        </div>
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
</style>
