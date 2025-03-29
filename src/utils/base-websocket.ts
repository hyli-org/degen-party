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

    protected abstract onOpen(): void;
    protected abstract onMessage(data: any): void;

    public async send(message: any) {
        try {
            await sharedWebSocket.send(message);
        } catch (error) {
            console.error("Failed to send message:", error);
            throw error;
        }
    }

    disconnect() {
        // Individual services don't disconnect the shared WebSocket
    }
}
