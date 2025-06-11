import { ref, watchEffect } from "vue";
import { walletState } from "./wallet";

const walletUrl =
    window.location.hostname === "localhost" ? "http://localhost:4321" : "https://indexer.testnet.hyli.org";

export const oranjBalance = ref(-1);

async function fetchOranjBalance() {
    if (!walletState.wallet?.address) {
        oranjBalance.value = -1;
        return;
    }
    try {
        const response = await fetch(`${walletUrl}/v1/indexer/contract/oranj/balance/${walletState.wallet.address}`);
        const data = await response.json();
        console.log("ORANJ balance data:", data);
        oranjBalance.value = data.balance;
    } catch (error) {
        oranjBalance.value = 0; // Set to 0 on proper error
        console.error("Error fetching ORANJ balance:", error);
    }
}

watchEffect(() => {
    fetchOranjBalance();
});
