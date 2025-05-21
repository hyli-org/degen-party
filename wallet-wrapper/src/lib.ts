import React from "react";
import { createRoot } from "react-dom/client";
import { WalletProvider } from "hyli-wallet";
import type { ProviderOption } from "hyli-wallet";
import { HyliWalletWrapper } from "./wrapper";

class HyliWalletElement extends HTMLElement {
    private mountPoint: HTMLDivElement | null = null;

    wallet: any = null;
    config: any = null;

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
                    config: this.config,
                },
                React.createElement(HyliWalletWrapper, { getWallet, providers })
            )
        );
    }
}

customElements.define("hyli-wallet", HyliWalletElement);
