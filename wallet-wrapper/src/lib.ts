import React from "react";
import { createRoot } from "react-dom/client";
import { HyliWallet, WalletProvider } from "hyli-wallet";
import type { ProviderOption } from "hyli-wallet";

class HyliWalletElement extends HTMLElement {
    connectedCallback() {
        const mountPoint = document.createElement("div");
        this.appendChild(mountPoint);

        const providersAttr = this.getAttribute("providers");
        const providers = providersAttr
            ? (providersAttr.split(",") as ProviderOption[])
            : ["password" as ProviderOption];

        createRoot(mountPoint).render(
            React.createElement(WalletProvider, null, React.createElement(HyliWallet, { providers }))
        );
    }
}

customElements.define("hyli-wallet", HyliWalletElement);
