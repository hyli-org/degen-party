import { reactive } from "vue";

export const walletState = reactive({
    wallet: null,
    registerSessionKey: null,
});

export const onWalletReady = async (walletEvent: any) => {
    console.log("Wallet ready event:", walletEvent.detail[0]);
    const { wallet, registerSessionKey } = walletEvent.detail[0];
    walletState.wallet = wallet;
    walletState.registerSessionKey = registerSessionKey;
    console.log("Wallet ready:", wallet);
};

export const walletConfig =
    window.location.hostname === "localhost"
        ? {
              nodeBaseUrl: "http://localhost:4321",
              walletServerBaseUrl: "http://localhost:4000",
              applicationWsUrl: "ws://localhost:8081/ws",
          }
        : {
              nodeBaseUrl: "https://node.testnet.hyli.org",
              walletServerBaseUrl: "https://wallet.testnet.hyli.org",
              applicationWsUrl: "wss://wallet.testnet.hyli.org/ws",
          };
