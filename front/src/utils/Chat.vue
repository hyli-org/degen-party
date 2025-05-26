<script setup lang="ts">
import { ref } from "vue";
import { getLocalPlayerId } from "../game_data/game_data";
import { addIdentityToMessage } from "../game_data/auth";
import { TestnetChatElement } from "hyli-testnet-chat";

if (!customElements.get("testnet-chat")) customElements.define("testnet-chat", TestnetChatElement);

const nodeUrl = window.location.hostname === "localhost" ? "http://localhost:4321" : "https://node.testnet.hyli.org";
const indexerUrl =
    window.location.hostname === "localhost" ? "http://localhost:4321" : "https://indexer.testnet.hyli.org";
</script>

<template>
    <testnet-chat
        :nickname="getLocalPlayerId()"
        :processBlobTx="addIdentityToMessage"
        :node_url="nodeUrl"
        :indexer_url="indexerUrl"
    ></testnet-chat>
</template>

<style>
.chat-container,
.chat-container * {
    border: none !important;
    box-shadow: none !important;
}
</style>
