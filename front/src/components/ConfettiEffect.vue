<template>
    <div class="confetti-container" v-if="isActive">
        <div
            v-for="(confetti, index) in confettiItems"
            :key="index"
            class="confetti-item"
            :style="getConfettiStyle(confetti)"
        ></div>
    </div>
</template>

<script>
export default {
    name: "ConfettiEffect",
    props: {
        active: {
            type: Boolean,
            default: false,
        },
        duration: {
            type: Number,
            default: 3000, // Duration in milliseconds
        },
        particleCount: {
            type: Number,
            default: 100,
        },
    },
    data() {
        return {
            isActive: false,
            confettiItems: [],
            confettiColors: [
                "#FF5252", // Mario Red
                "#65B955", // Yoshi Green
                "#FFD700", // Star Yellow
                "#00A8DF", // Toad Blue
                "#FF6B15", // Bowser Orange
                "#F8BBD0", // Peach Pink
                "#9C27B0", // Wario Purple
                "#209CEE", // Mario Blue
            ],
            confettiShapes: ["circle", "square", "triangle", "star"],
        };
    },
    watch: {
        active(newVal) {
            if (newVal) {
                this.startConfetti();
            }
        },
    },
    methods: {
        startConfetti() {
            this.isActive = true;
            this.generateConfetti();

            // Automatically stop after duration
            setTimeout(() => {
                this.isActive = false;
            }, this.duration);
        },
        generateConfetti() {
            this.confettiItems = [];

            for (let i = 0; i < this.particleCount; i++) {
                this.confettiItems.push({
                    color: this.getRandomItem(this.confettiColors),
                    shape: this.getRandomItem(this.confettiShapes),
                    x: Math.random() * 100, // x position in percent
                    size: Math.random() * 0.7 + 0.3, // between 0.3 and 1
                    speed: Math.random() * 3 + 1, // fall speed
                    angle: Math.random() * 360, // rotation angle
                    spinSpeed: (Math.random() - 0.5) * 10, // rotation speed
                    swingFactor: Math.random() * 10, // horizontal swing amount
                    swingSpeed: Math.random() * 0.2 + 0.1, // horizontal swing speed
                });
            }
        },
        getRandomItem(array) {
            return array[Math.floor(Math.random() * array.length)];
        },
        getConfettiStyle(confetti) {
            const animationDuration = confetti.speed * 3 + "s";
            const animationDelay = Math.random() * 2 + "s";
            const size = confetti.size * 1 + "rem";

            let shape = "";
            switch (confetti.shape) {
                case "circle":
                    shape = "border-radius: 50%;";
                    break;
                case "square":
                    shape = "border-radius: 0;";
                    break;
                case "triangle":
                    shape = `
            clip-path: polygon(50% 0%, 0% 100%, 100% 100%);
            border-radius: 0;
          `;
                    break;
                case "star":
                    shape = `
            clip-path: polygon(50% 0%, 61% 35%, 98% 35%, 68% 57%, 79% 91%, 50% 70%, 21% 91%, 32% 57%, 2% 35%, 39% 35%);
            border-radius: 0;
          `;
                    break;
            }

            return {
                left: confetti.x + "%",
                width: size,
                height: size,
                backgroundColor: confetti.color,
                animation: `confetti-fall ${animationDuration} ease-in infinite, 
                    confetti-swing ${confetti.swingSpeed * 5 + "s"} ease-in-out infinite alternate`,
                animationDelay: animationDelay,
                transform: `rotate(${confetti.angle}deg) scale(${confetti.size})`,
                opacity: confetti.size,
                "--swing-factor": confetti.swingFactor + "vw",
                "--spin-speed": confetti.spinSpeed + "deg",
            };
        },
    },
};
</script>

<style scoped>
.confetti-container {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 9999;
    overflow: hidden;
}

.confetti-item {
    position: absolute;
    top: -10%;
    width: 1rem;
    height: 1rem;
    border-radius: 50%;
    background-color: red;
    z-index: 10000;
    will-change: transform;
}

@keyframes confetti-fall {
    0% {
        top: -10%;
        transform: translateX(0) rotate(var(--spin-speed));
    }
    100% {
        top: 110%;
        transform: translateX(0) rotate(calc(var(--spin-speed) * 50));
    }
}

@keyframes confetti-swing {
    0% {
        margin-left: calc(-1 * var(--swing-factor));
    }
    100% {
        margin-left: var(--swing-factor);
    }
}
</style>
