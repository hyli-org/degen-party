import { computed, reactive, watchEffect } from "vue";
import { boardGameService, gameState } from "../game_data/game_data";

export const animState = reactive({
    wheelIsSpinning: false,
    wheelSpinTime: 0 as number | null,
    nextEvent: undefined as unknown | undefined,
    nextRound: undefined as number | undefined,
});

let enteringMinigameTimer: any | null = null;

let wheelSpinInterval: any | null = null;
let wheelSpinStartTime: number | null = null;

const advanceWheelSpin = (time) => {
    if (!animState.wheelIsSpinning) return;
    animState.wheelSpinTime = (time - (wheelSpinStartTime as number)) / 1000.0;
    requestAnimationFrame(advanceWheelSpin);
};

const eventsPerRound = {};

const currentRound = () => {
    return gameState.game?.round || -1;
};

export const nextRoundEvent = computed(() => {
    if (animState.nextRound === undefined || !eventsPerRound[animState.nextRound]) {
        return undefined;
    }
    if (animState.nextEvent === undefined) {
        return undefined;
    }
    const events = eventsPerRound[animState.nextRound];
    if (animState.nextEvent === 0) {
        return {
            title: "Nothing happened",
            description: "A calm day where not much happened at all.",
        };
    } else if (animState.nextEvent === 1) {
        return {
            title: "Fumble !",
            description: "In the dust storm, all players fumble their bets and swap them randomly.",
        };
    } else if (animState.nextEvent === 2) {
        return {
            title: "All or nothing !",
            description: "The situation is dire ! All players are forced to bet all their money to survive.",
        };
    } else {
        return {
            title: "A strange happening",
            description: "MINIGAME BB",
        };
    }
});

boardGameService.onStateUpdated = (payload) => {
    if (!payload?.events) return;

    for (const e of payload.events) {
        console.log(e);
        if (!eventsPerRound[currentRound()]) {
            eventsPerRound[currentRound()] = [];
        }
        eventsPerRound[currentRound()].push(e);
        if (typeof e === "object" && "WheelSpun" in e) {
            // Start the wheel spin animation (overriding any previous spin)
            animState.wheelIsSpinning = true;
            animState.wheelSpinTime = 0;
            wheelSpinStartTime = performance.now();
            wheelSpinInterval = requestAnimationFrame(advanceWheelSpin);
            animState.nextEvent = -1;
            animState.nextEvent = e.WheelSpun.outcome;
            animState.nextRound = gameState.game?.round;
            if (e.WheelSpun.outcome > 2 && !enteringMinigameTimer) {
                enteringMinigameTimer = setTimeout(() => {
                    gameState.isInMinigame = true;
                    enteringMinigameTimer = undefined;
                }, 5000);
            }
        }
    }
};
