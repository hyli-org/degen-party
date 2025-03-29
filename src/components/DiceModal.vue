<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';

const props = defineProps<{
    isOpen: boolean;
    diceResult: number;
}>();

const emit = defineEmits<{
    (e: 'close'): void;
}>();

const isAnimating = ref(false);
const showResult = ref(false);

watch(() => props.isOpen, (newValue) => {
    if (newValue) {
        isAnimating.value = true;
        setTimeout(() => {
            showResult.value = true;
            setTimeout(() => {
                isAnimating.value = false;
                setTimeout(() => {
                    emit('close');
                }, 2000);
            }, 1000);
        }, 1500);
    } else {
        showResult.value = false;
        isAnimating.value = false;
    }
});
</script>

<template>
    <Transition name="modal">
        <div v-if="isOpen" class="fixed top-24 right-4 z-[9999]">
            <!-- Modal -->
            <div class="bg-[#8B0000] rounded-2xl p-4 shadow-2xl border-4 border-[#ffa048] w-[200px] flex flex-col items-center justify-center transform scale-100">
                <!-- Dice container -->
                <div class="relative w-20 h-20">
                    <!-- Animated dice -->
                    <div v-if="isAnimating && !showResult" 
                         class="absolute inset-0 text-6xl animate-dice-roll flex items-center justify-center">
                        🎲
                    </div>
                    
                    <!-- Result -->
                    <div v-if="showResult" 
                         class="absolute inset-0 flex items-center justify-center animate-result-pop">
                        <div class="text-4xl font-['Luckiest_Guy'] text-white">
                            {{ diceResult }}
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </Transition>
</template>

<style scoped>
.modal-enter-active,
.modal-leave-active {
    transition: all 0.3s ease;
}

.modal-enter-from,
.modal-leave-to {
    opacity: 0;
    transform: scale(0.9);
}

@keyframes dice-roll {
    0% { transform: rotate(0deg) scale(1); }
    25% { transform: rotate(90deg) scale(0.8); }
    50% { transform: rotate(180deg) scale(1.1); }
    75% { transform: rotate(270deg) scale(0.9); }
    100% { transform: rotate(360deg) scale(1); }
}

.animate-dice-roll {
    animation: dice-roll 0.5s infinite;
}

@keyframes result-pop {
    0% { transform: scale(0); opacity: 0; }
    50% { transform: scale(1.2); }
    100% { transform: scale(1); opacity: 1; }
}

.animate-result-pop {
    animation: result-pop 0.5s ease-out forwards;
}
</style> 