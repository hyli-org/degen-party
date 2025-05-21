import { ProviderOption, useWallet, Wallet } from "hyli-wallet";
import { HyliWallet } from "hyli-wallet";

interface HyliWalletProps {
    /**
     * Optional render prop that gives full control over the connect button UI.
     * If not supplied, a simple default button will be rendered.
     */
    button?: (props: { onClick: () => void }) => React.ReactNode;
    /**
     * Optional explicit provider list (e.g., ["password", "google"]). If omitted, available providers will be detected automatically.
     */
    providers?: ProviderOption[];

    getWallet?: (wallet: any | null) => any;
}

export const HyliWalletWrapper = ({ getWallet, button, providers }: HyliWalletProps) => {
    const walletCtx = useWallet();

    if (getWallet) {
        getWallet(walletCtx);
    }

    return (
        <>
            <HyliWallet button={button} providers={providers} />
        </>
    );
};
