<script setup lang="ts">
import { computed, ref, onMounted } from "vue";
import type { Space, Player } from "../game_data/game_data";
import { playerAvatar, playerColor } from "../game_data/game_data";

const props = defineProps<{
    spaces: Space[];
    players: Player[];
    size: number;
}>();

// Memoize the grid dimensions
const gridDimensions = computed(() => {
    const width = Math.ceil(Math.sqrt(props.size));
    return {
        width,
        height: Math.ceil(props.size / width),
    };
});

// Memoize the space positions - only recalculate when size changes
const spacePositions = computed(() => {
    interface Position {
        index: number;
        x: number;
        y: number;
        isReversed: boolean;
    }

    const positions: Position[] = [];
    const { width, height } = gridDimensions.value;
    let currentIndex = 0;

    for (let row = 0; row < height && currentIndex < props.size; row++) {
        const isReversedRow = row % 2 === 1;
        const rowSpaces = Math.min(width, props.size - row * width);

        for (let col = 0; col < rowSpaces; col++) {
            const actualCol = isReversedRow ? rowSpaces - 1 - col : col;
            positions.push({
                index: currentIndex++,
                x: actualCol,
                y: row,
                isReversed: isReversedRow,
            });
        }
    }

    return positions;
});

// Memoize the SVG path - only recalculate when size changes
const boardPath = computed(() => generatePath());

// Cache player positions for faster lookup
const playerPositions = computed(() => {
    const positions = new Map<number, Player[]>();
    props.players.forEach(player => {
        if (!positions.has(player.position)) {
            positions.set(player.position, []);
        }
        positions.get(player.position)?.push(player);
    });
    return positions;
});

function spaceLabel(space: Space): string {
    return (
        {
            Blue: "+3 🪙",
            Red: "-3 🪙",
            Event: "?",
            MinigameSpace: "🎮",
            Star: "⭐",
            Finish: "🏁",
        }[space] || ""
    );
}

function playersOnSpace(spaceIndex: number): Player[] {
    return playerPositions.value.get(spaceIndex) || [];
}

function generatePath(): string {
    const { width, height } = gridDimensions.value;
    const spacing = 80;
    const radius = 25;
    let path = "";

    // Get all valid positions
    const positions = spacePositions.value;
    if (positions.length === 0) return "";

    // Start from the first position
    const firstPos = positions[0];
    path = `M ${(firstPos.x + 0.5) * spacing},${(firstPos.y + 0.5) * spacing}`;

    // Connect each position to the next one
    for (let i = 0; i < positions.length - 1; i++) {
        const current = positions[i];
        const next = positions[i + 1];

        // If next position is in the same row
        if (current.y === next.y) {
            path += ` L ${(next.x + 0.5) * spacing},${(next.y + 0.5) * spacing}`;
        } else {
            // Direct curve down to the next row's position
            const startX = (current.x + 0.5) * spacing;
            const startY = (current.y + 0.5) * spacing;
            const endX = (next.x + 0.5) * spacing;
            const endY = (next.y + 0.5) * spacing;
            const controlY = (startY + endY) / 2; // Midpoint for smooth curve

            path += ` C ${startX},${controlY} ${endX},${controlY} ${endX},${endY}`;
        }
    }

    return path;
}
</script>

<template>
    <div
        class="relative p-12 bg-[#8B0000] bg-[repeating-linear-gradient(45deg,transparent,transparent_10px,#7A0000_10px,#7A0000_20px)]"
    >
        <!-- Game path -->
        <div class="absolute top-12 left-12 right-12 bottom-12">
            <svg class="w-full h-full" :viewBox="`0 0 ${gridDimensions.width * 80} ${gridDimensions.height * 80}`">
                <path
                    :d="boardPath"
                    class="fill-none stroke-[#3D8C40] stroke-[50] stroke-linecap-round stroke-linejoin-round"
                />
            </svg>
        </div>

        <div
            class="grid gap-2 relative"
            :style="{
                'grid-template-columns': `repeat(${gridDimensions.width}, 80px)`,
                'grid-template-rows': `repeat(${gridDimensions.height}, 80px)`,
            }"
        >
            <div
                v-for="pos in spacePositions"
                :key="pos.index"
                class="group w-[80px] h-[80px] relative"
                :style="{
                    gridColumn: pos.x + 1,
                    gridRow: pos.y + 1,
                }"
            >
                <!-- Use v-once for static elements that don't need to update -->
                <div
                    v-once
                    class="absolute inset-0 rounded-full bg-black/40 blur-sm transform translate-x-1 translate-y-1 -z-10"
                ></div>

                <!-- Main token body -->
                <div
                    class="w-full h-full rounded-full flex justify-center items-center text-white font-bold transition-all duration-300 hover:scale-105 hover:z-10 relative"
                    :class="[
                        {
                            'bg-black': props.spaces[pos.index] === 'Blue',
                            'bg-[#CC0000]': props.spaces[pos.index] === 'Red',
                            'bg-[#4A1A8C]': props.spaces[pos.index] === 'Event',
                            'bg-[#B37D0E]': props.spaces[pos.index] === 'MinigameSpace',
                            'bg-[#CC9900]': props.spaces[pos.index] === 'Star',
                            'bg-[#E6B800]': props.spaces[pos.index] === 'Finish',
                        },
                    ]"
                >
                    <!-- Use v-once for static elements -->
                    <div
                        v-once
                        class="absolute inset-[2px] rounded-full border-[8px] border-dashed border-white/80"
                    ></div>

                    <div
                        class="absolute inset-[12px] rounded-full"
                        :class="[
                            {
                                'bg-black': props.spaces[pos.index] === 'Blue',
                                'bg-[#CC0000]': props.spaces[pos.index] === 'Red',
                                'bg-[#4A1A8C]': props.spaces[pos.index] === 'Event',
                                'bg-[#B37D0E]': props.spaces[pos.index] === 'MinigameSpace',
                                'bg-[#CC9900]': props.spaces[pos.index] === 'Star',
                                'bg-[#E6B800]': props.spaces[pos.index] === 'Finish',
                            },
                        ]"
                    >
                        <!-- Use v-once for static elements -->
                        <div v-once class="absolute inset-[4px] rounded-full border-[3px] border-white/30"></div>

                        <div
                            class="w-full h-full flex flex-col items-center justify-center gap-1 relative z-10 transform group-hover:-translate-y-0.5 transition-transform duration-300"
                        >
                            <span class="text-lg font-extrabold drop-shadow-[0_2px_2px_rgba(0,0,0,0.5)]">{{
                                spaceLabel(props.spaces[pos.index])
                            }}</span>
                            <div v-if="playersOnSpace(pos.index).length > 0" class="absolute -top-4 flex gap-1 w-[80px] flex-wrap">
                                <div
                                    v-for="player in playersOnSpace(pos.index)"
                                    :key="player.id"
                                    class="w-[30px] h-[30px] rounded-full flex justify-center items-center text-2xl shadow-lg shadow-black/30 border-2 border-black"
                                    :style="{ backgroundColor: playerColor(player.id) }"
                                >
                                    {{ playerAvatar(player.id) }}
                                </div>
                            </div>
                        </div>
                    </div>

                    <!-- Use v-once for static elements -->
                    <div v-once class="absolute inset-[12px] rounded-full bg-gradient-to-b from-white/20 to-transparent"></div>
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped>
.grid {
    position: relative;
    z-index: 1;
}
</style>
