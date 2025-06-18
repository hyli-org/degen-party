<script setup lang="ts">
import { ref } from "vue";
import { watchEffect } from "vue";
import { useRoute } from "vue-router";

import { declareCustomElement } from "testnet-maintenance-widget";
import { HyliWalletElement } from "wallet-wrapper";

declareCustomElement();
customElements.define("hyli-wallet", HyliWalletElement);

const route = useRoute();
const routeFullPath = ref(route.fullPath);
watchEffect(() => {
    routeFullPath.value = route.fullPath;
});

// If not on localhost, use the production node URL
const nodeurl = window.location.hostname === "localhost" ? undefined : "https://node.testnet.hyli.org";
</script>

<template>
    <maintenance-widget :nodeurl="nodeurl" />
    <RouterView :key="routeFullPath" />
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
    line-height: 1.6;
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

@keyframes pulse {
    0%,
    100% {
        opacity: 1;
    }
    50% {
        opacity: 0.6;
    }
}
</style>
