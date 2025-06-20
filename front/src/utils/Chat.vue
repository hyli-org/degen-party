<script setup lang="ts">
import { getLocalPlayerId } from "../game_data/game_data";
import { addIdentityToMessage } from "../game_data/auth";
import { TestnetChatElement } from "hyli-testnet-chat";
import { walletState } from "./wallet";

if (!customElements.get("testnet-chat")) customElements.define("testnet-chat", TestnetChatElement);

const nodeUrl = window.location.hostname === "localhost" ? "http://localhost:4321" : "https://node.testnet.hyli.org";
const indexerUrl =
    window.location.hostname === "localhost" ? "http://localhost:4321" : "https://indexer.testnet.hyli.org";
</script>

<template>
    <testnet-chat
        v-if="false && !!walletState.wallet"
        :nickname="getLocalPlayerId()"
        :processBlobTx="addIdentityToMessage"
        :node_url="nodeUrl"
        :indexer_url="indexerUrl"
    ></testnet-chat>
</template>

<style scoped>
:deep(.chat-container) {
    border: none !important;
}
:deep(.messages-list) {
    border-color: #d94524 !important;
    max-height: initial !important;
}
:deep(.message) {
    background-color: #2a1c4b;
}
:deep(*) {
    color: white !important;
}
</style>
