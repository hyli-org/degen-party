import { sharedWebSocket } from "./shared-websocket";

export abstract class BaseWebSocketService {
    constructor() {
        this.setupMessageHandler();
    }

    private setupMessageHandler() {
        sharedWebSocket.addMessageHandler((data) => this.onMessage(data));
    }

    async connect(): Promise<void> {
        return sharedWebSocket.connect();
    }

    protected abstract onMessage(data: any): void;

    async send(message: any, signed_data: string) {
        try {
            await sharedWebSocket.send(message, signed_data);
        } catch (error) {
            console.error("Failed to send message:", error);
            throw error;
        }
    }
}
