import { createRouter, createWebHistory } from "vue-router";
import CrashGame from "../components/CrashGame.vue";
import Board from "../components/Board.vue";
import { watchEffect } from "vue";
import { crashGameState } from "../game_data/crash";
import { gameState } from "../game_data/game_data";

const routes = [
    {
        path: "/crash",
        name: "CrashGame",
        component: CrashGame,
    },
    {
        path: "/board",
        name: "Board",
        component: Board,
    },
    // Add more routes here as needed
];

const router = createRouter({
    history: createWebHistory(),
    routes,
});

watchEffect(() => {
    // If there's a minigame ongoing, switch to that page
    if (crashGameState.minigame && gameState.running_minigame) {
        router.push({ name: "CrashGame" });
    } else {
        router.push({ name: "Board" });
    }
});

export default router;
