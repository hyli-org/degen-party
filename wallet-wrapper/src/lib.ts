import React from "react";
import { createRoot, Root } from "react-dom/client";
import { WalletProvider } from "hyli-wallet";
import type { ProviderOption, WalletProviderProps } from "hyli-wallet";
import { HyliWalletWrapper } from "./wrapper";
import type { WalletContextType } from "hyli-wallet";

export type { WalletContextType } from "hyli-wallet";
export type { WalletProviderProps } from "hyli-wallet";

export type WalletUpdateEvent = CustomEvent<WalletContextType>;

export class HyliWalletElement extends HTMLElement {
    mountPoint: HTMLDivElement | null = null;
    root: Root | null = null;

    providers: ProviderOption[] = ["password" as ProviderOption];
    config!: WalletProviderProps["config"];
    sessionKeyConfig: WalletProviderProps["sessionKeyConfig"] | undefined;
    onWalletEvent: WalletProviderProps["onWalletEvent"] | undefined;
    onError: WalletProviderProps["onError"] | undefined;

    constructor() {
        super();
        const reactiveProps = ["providers", "config", "sessionKeyConfig", "onWalletEvent", "onError"] as const;
        for (const prop of reactiveProps) {
            const priv = `__${prop}`;
            Object.defineProperty(this, priv, {
                enumerable: false,
                writable: true,
                value: this[prop],
            });
            Object.defineProperty(this, prop, {
                get: () => {
                    return Reflect.get(this, priv);
                },
                set: (value) => {
                    Reflect.set(this, priv, value);
                    if (this.root) {
                        this.render();
                    }
                },
                enumerable: true,
            });
        }
    }

    connectedCallback() {
        console.log(Object.keys(this));
        if (this.mountPoint) {
            return;
        }
        this.mountPoint = document.createElement("div");
        this.appendChild(this.mountPoint);
        this.root = createRoot(this.mountPoint);

        this.render();
    }

    render() {
        const getWallet = (walletCtx: WalletContextType) => {
            this.dispatchEvent(
                new CustomEvent("walletUpdate", {
                    detail: walletCtx,
                    bubbles: true,
                    composed: true,
                }) as WalletUpdateEvent
            );
        };
        console.log("Rendering with ", this.sessionKeyConfig);
        this.root!.render(
            React.createElement(
                WalletProvider,
                {
                    config: this.config,
                    sessionKeyConfig: this.sessionKeyConfig,
                    onWalletEvent: this.onWalletEvent,
                    onError: this.onError,
                },
                React.createElement(HyliWalletWrapper, { getWallet, providers: this.providers })
            )
        );
    }
}

//customElements.define("hyli-wallet", HyliWalletElement);
