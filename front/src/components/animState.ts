import { computed, reactive, ref, watch, watchEffect } from "vue";
import { boardGameService, gameState } from "../game_data/game_data";

export const animState = reactive({
    timeInRound: 0,
    roundStartTime: 0,
    currentRoundIndex: -1, // index in eventHistory
    // Object: animation id -> timestamp when played for this round
    playedAnimations: {} as Record<string, number>,
    eventHistory: {} as Record<number, { round: number; outcome: number }>,
});

// Helper: get current round object
export const currentRoundEvents = computed(
    () => animState.eventHistory[animState.currentRoundIndex] || { round: -1, outcome: -1 },
);

const roundToDay = (round: number) => {
    round = round + 1;
    if (round === 1) return "first";
    if (round === 2) return "second";
    if (round === 3) return "third";
    if (round === 4) return "fourth";
    if (round === 5) return "fifth";
    if (round === 6) return "sixth";
    if (round === 7) return "seventh";
    if (round === 8) return "eighth";
    if (round === 9) return "ninth";
    if (round === 10) return "tenth";
    return `${round}th`;
};

export const roundOutcome = (round: number) => {
    if (round === 9) {
        return {
            outcome: 4,
            title: `On the ${roundToDay(round)} day, the city beckons!`,
            description:
                "As the dust settles, the city of the future emerges from the storm.\nThere is only one... last... Game!",
        };
    }
    const outcome = animState.eventHistory[round]?.outcome;
    if (outcome === undefined) {
        return {
            outcome: -1,
            title: `On the ${roundToDay(round)} day, history forgets...`,
            description: "No event has been recorded for this round.",
        };
    }
    if (outcome === 0) {
        return {
            outcome: 0,
            title: `The ${roundToDay(round)} day, a quiet day`,
            description: "Really, not much happened at all.",
        };
    } else if (outcome === 1) {
        return {
            outcome: 1,
            title: `On the ${roundToDay(round)} day, Fumble !`,
            description: "In the dust storm, all players fumble their bets and swap them randomly.",
        };
    } else if (outcome === 2) {
        return {
            outcome: 2,
            title: `On the ${roundToDay(round)} day, ALL or NOTHING!`,
            description: "The situation is dire ! All players are forced to bet all their money to survive.",
        };
    } else {
        return {
            outcome: 3,
            title: `On the ${roundToDay(round)} day, something strange happened!`,
            description: "It seems... We are gamers!",
        };
    }
};

const advanceAnim = (time) => {
    animState.timeInRound = (time - animState.roundStartTime) / 1000; // convert to seconds
    animTimer = requestAnimationFrame(advanceAnim);
};
animState.roundStartTime = performance.now() / 1000; // convert to seconds
let animTimer = requestAnimationFrame(advanceAnim);

boardGameService.onStateUpdated = (payload) => {
    if (animState.currentRoundIndex === -1) {
        // First time we receive state, initialize the round index
        animState.currentRoundIndex = payload.state?.round ?? -1;
    }
    if (!payload?.events) return;
    for (const e of payload.events) {
        if (typeof e === "object" && "GameInitialized" in e) {
            // Reset the animation state when a new game is initialized
            animState.currentRoundIndex = 0;
            animState.eventHistory = reactive({});
        } else if (typeof e === "object" && "WheelSpun" in e) {
            const roundNum = e.WheelSpun.round;
            let roundEntry = animState.eventHistory[roundNum];
            if (!roundEntry) {
                roundEntry = reactive({ round: roundNum, outcome: e.WheelSpun.outcome });
                animState.eventHistory[roundNum] = roundEntry;
            } else {
                roundEntry.outcome = e.WheelSpun.outcome;
            }
        }
    }
};

// Helper to mark an animation as played for the current round
export function markAnimationPlayed(id: string) {
    animState.playedAnimations[id] = (performance.now() - animState.roundStartTime) / 1000;
}

// Helper to check if an animation has been played for the current round
export function isAnimationPlayed(id: string): boolean {
    return animState.playedAnimations?.[id] !== undefined;
}

export function getAnimationPlayedTime(id: string): number {
    return animState.playedAnimations[id] ?? 0;
}

// Object to track scheduled timers for marking animations as played
const scheduledAnimationTimers: Record<string, number> = {};

// Helper to mark an animation as played after a delay (in seconds)
// Optionally runs a callback when the animation is marked
export function markAnimationPlayedIn(id: string, delay: number, cb?: () => void) {
    // If already played or already scheduled, do nothing
    if (isAnimationPlayed(id) || scheduledAnimationTimers[id] !== undefined) return;
    const roundAtSchedule = animState.currentRoundIndex;
    const timer = window.setTimeout(() => {
        // Only mark if still on the same round and not already played
        if (animState.currentRoundIndex === roundAtSchedule && !isAnimationPlayed(id)) {
            markAnimationPlayed(id);
            if (cb) cb();
        }
        delete scheduledAnimationTimers[id];
    }, delay * 1000);
    scheduledAnimationTimers[id] = timer;
}

// Clean up scheduled timers and playedAnimations on round change

watch(
    () => animState.currentRoundIndex,
    () => {
        // Clear all scheduled timers
        for (const timer of Object.values(scheduledAnimationTimers)) {
            clearTimeout(timer);
        }
        for (const key in scheduledAnimationTimers) {
            delete scheduledAnimationTimers[key];
        }
        for (const key in animState.playedAnimations) {
            delete animState.playedAnimations[key];
        }
        animState.roundStartTime = performance.now();
        cancelAnimationFrame(animTimer);
        animTimer = requestAnimationFrame(advanceAnim);
    },
);

// Wheel spin logic

const wheelOptions = [
    { label: "Quiet day", color: "#36C6FF", outcome: 0 },
    { label: "Minigame", color: "#FF4D4D", outcome: 3 },
    { label: "Fumble", color: "#00C49A", outcome: 1 },
    { label: "Minigame", color: "#FF4D4D", outcome: 4 },
    { label: "All or Nothing", color: "#FFB347", outcome: 2 },
    //{ label: "Minigame", color: "#FF4D4D", outcome: 5 },
];

const spinning = ref(false);
export const spinAngle = ref(0); // in radians
const targetAngle = ref(0); // in radians
const spinDuration = 2; // seconds
const lastOutcome = ref<number | null>(null);

const animateSpin = () => {
    if (!spinning.value) return;
    const elapsed = animState.timeInRound - getAnimationPlayedTime("SpinWheel");
    let t = Math.min(1, elapsed / spinDuration);
    // Ease out cubic
    t = 1 - Math.pow(1 - t, 3);
    spinAngle.value = targetAngle.value * t;
    if (t < 1) {
        requestAnimationFrame(animateSpin);
    } else {
        spinning.value = false;
        spinAngle.value = targetAngle.value;
    }
};

const startSpinAnimation = (outcome: number) => {
    markAnimationPlayed("SpinWheel");
    // The wheel should land so that the outcome slice is at the top (pointer)
    const sliceAngle = (2 * Math.PI) / wheelOptions.length;
    // Add several full spins for effect
    const fullSpins = 3;
    const outcomeIndex = wheelOptions.findIndex((option) => option.outcome === outcome);
    const outcomeAngle = sliceAngle * outcomeIndex;
    targetAngle.value = fullSpins * 2 * Math.PI - outcomeAngle;
    spinning.value = true;
    animateSpin();
    lastOutcome.value = outcome;
};

watchEffect(() => {
    const outcome = currentRoundEvents.value?.outcome;
    if (outcome === undefined || outcome === -1 || isAnimationPlayed("SpinWheel")) return;
    spinAngle.value = 0;
    startSpinAnimation(outcome);
});
