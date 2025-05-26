import { ec } from "elliptic";
import { SHA256 } from "crypto-js";
import { any } from "three/tsl";
import { BorshSchema, borshSerialize } from "borsher";
import { identity } from "@vueuse/core";
import { walletState } from "../utils/wallet";

const SESSION_KEY_STORAGE_KEY = "blackjack_session_key";
const PUBLIC_KEY_STORAGE_KEY = "blackjack_public_key";

class AuthService {
    private sessionKey: string | null = null;
    private publicKey: string | null = null;
    private ec: ec;

    constructor() {
        this.ec = new ec("secp256k1");
        // Récupérer la sessionKey et la publicKey du localStorage au démarrage
        this.sessionKey = localStorage.getItem(SESSION_KEY_STORAGE_KEY);
        this.publicKey = localStorage.getItem(PUBLIC_KEY_STORAGE_KEY);
    }

    reload(privateKey: string, publicKey: string) {
        localStorage.setItem(SESSION_KEY_STORAGE_KEY, privateKey);
        localStorage.setItem(PUBLIC_KEY_STORAGE_KEY, publicKey);
        this.sessionKey = localStorage.getItem(SESSION_KEY_STORAGE_KEY);
        this.publicKey = localStorage.getItem(PUBLIC_KEY_STORAGE_KEY);
    }

    generateSessionKey(): string {
        // Génère une paire de clés ECDSA
        const keyPair = this.ec.genKeyPair();
        // Stocke la clé privée
        const privateKey = keyPair.getPrivate("hex");
        if (!privateKey) {
            throw new Error("Failed to generate private key");
        }
        this.sessionKey = privateKey;

        // Stocke la clé publique
        const publicKey = keyPair.getPublic(true, "hex");
        if (!publicKey) {
            throw new Error("Failed to generate public key");
        }
        this.publicKey = publicKey;

        // Sauvegarder dans le localStorage
        localStorage.setItem(SESSION_KEY_STORAGE_KEY, this.sessionKey!);
        localStorage.setItem(PUBLIC_KEY_STORAGE_KEY, this.publicKey!);
        return this.publicKey!;
    }

    getSessionKey(): string | null {
        return this.publicKey; // On retourne la clé publique pour l'authentification
    }

    signMessage(message: string): string {
        if (!this.sessionKey) {
            throw new Error("No session key available");
        }

        const hash = SHA256(message);
        const keyPair = this.ec.keyFromPrivate(this.sessionKey);
        const signature = keyPair.sign(hash.toString());

        // Normaliser s en utilisant min(s, n-s)
        const n = this.ec.curve.n;
        const s = signature.s;
        if (s.gt(n.shrn(1))) {
            signature.s = n.sub(s);
        }

        return signature.toDER("hex");
    }

    getBlobData(message: string) {
        if (!this.sessionKey) {
            throw new Error("No session key available");
        }

        const hash = SHA256(message);
        const keyPair = this.ec.keyFromPrivate(this.sessionKey);
        const signature = keyPair.sign(hash.toString());

        // Normaliser s en utilisant min(s, n-s)
        const n = this.ec.curve.n;
        const s = signature.s;
        if (s.gt(n.shrn(1))) {
            signature.s = n.sub(s);
        }

        const toBytes = (hex: string): number[] => {
            const bytes: number[] = [];
            for (let i = 0; i < hex.length; i += 2) {
                bytes.push(parseInt(hex.substring(i, i + 2), 16));
            }
            return bytes;
        };

        return {
            data: toBytes(hash.toString()),
            public_key: toBytes(this.publicKey!),
            signature: this.toCompact(signature),
        };
    }

    clearSession() {
        this.sessionKey = null;
        this.publicKey = null;
        localStorage.removeItem(SESSION_KEY_STORAGE_KEY);
        localStorage.removeItem(PUBLIC_KEY_STORAGE_KEY);
    }

    toCompact(signature: ec.Signature): number[] {
        return signature.r.toArray("be", 32).concat(signature.s.toArray("be", 32));
    }
}

export const authService = new AuthService();

const Secp256k1BlobSchema = BorshSchema.Struct({
    identity: BorshSchema.Struct({
        0: BorshSchema.String,
    }),
    data: BorshSchema.Array(BorshSchema.u8, 32),
    public_key: BorshSchema.Array(BorshSchema.u8, 33),
    signature: BorshSchema.Array(BorshSchema.u8, 64),
});

export async function addIdentityToMessage(blob_tx: any) {
    blob_tx.blobs.push(...walletState.createIdentityBlobs());
    return blob_tx;
}
