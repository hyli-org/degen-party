import React from "react";
import { createRoot } from "react-dom/client";
import { WalletProvider } from "hyli-wallet";
import type { ProviderOption } from "hyli-wallet";
import { HyliWalletWrapper } from "./wrapper";

class HyliWalletElement extends HTMLElement {
    private mountPoint: HTMLDivElement | null = null;

    wallet: any = null;

    connectedCallback() {
        if (this.mountPoint) {
            return;
        }
        const mountPoint = document.createElement("div");
        this.appendChild(mountPoint);

        const providersAttr = this.getAttribute("providers");
        const providers = providersAttr
            ? (providersAttr.split(",") as ProviderOption[])
            : ["password" as ProviderOption];

        const getWallet = (...wallet: any) => {
            this.wallet = wallet;
            this.dispatchEvent(
                new CustomEvent("walletReady", {
                    detail: wallet,
                    bubbles: true,
                    composed: true,
                })
            );
        };

        createRoot(mountPoint).render(
            React.createElement(
                WalletProvider,
                {
                    config: {
                        nodeBaseUrl: "http://localhost:4321",
                        walletServerBaseUrl: "http://localhost:4000",
                        applicationWsUrl: "ws://localhost:8081/ws",
                    },
                },
                React.createElement(HyliWalletWrapper, { getWallet, providers })
            )
        );
    }
}

customElements.define("hyli-wallet", HyliWalletElement);
