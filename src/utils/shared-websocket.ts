import { reactive } from "vue";

export interface WebSocketState {
    connected: boolean;
    connectionStatus: string;
}

export const wsState = reactive<WebSocketState>({
    connected: false,
    connectionStatus: "Disconnected",
});

class SharedWebSocketService {
    private static instance: SharedWebSocketService;
    private ws: WebSocket | null = null;
    private reconnectTimeout: number = 3000;
    private reconnectAttempts: number = 0;
    private maxReconnectAttempts: number = 5;
    private messageHandlers: Set<(data: any) => void> = new Set();
    private connectionPromise: Promise<void> | null = null;
    private resolveConnection: (() => void) | null = null;
    private rejectConnection: ((error: Error) => void) | null = null;

    private constructor() {}

    static getInstance(): SharedWebSocketService {
        if (!SharedWebSocketService.instance) {
            SharedWebSocketService.instance = new SharedWebSocketService();
        }
        return SharedWebSocketService.instance;
    }

    async connect(): Promise<void> {
        if (this.ws?.readyState === WebSocket.OPEN) {
            return Promise.resolve();
        }

        if (!this.connectionPromise) {
            this.connectionPromise = new Promise((resolve, reject) => {
                this.resolveConnection = resolve;
                this.rejectConnection = reject;
            });
        } else {
            return this.connectionPromise;
        }

        const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
        const port = window.location.hostname === "localhost" ? ":8080" : "";
        const wsUrl = `${protocol}//${window.location.hostname}${port}/ws`;
        //const wsUrl = `${protocol}//192.168.23.151${port}/ws`;

        console.log("Connecting to WebSocket:", wsUrl);

        this.ws = new WebSocket(wsUrl);
        this.setupEventHandlers();

        return this.connectionPromise;
    }

    private setupEventHandlers() {
        if (!this.ws) return;

        this.ws.onopen = () => {
            console.log("WebSocket connection opened");
            wsState.connected = true;
            wsState.connectionStatus = "Connected";
            this.reconnectAttempts = 0;
            this.resolveConnection?.();
        };

        this.ws.onclose = () => {
            console.log("WebSocket connection closed");
            wsState.connected = false;
            wsState.connectionStatus = "Disconnected";
            this.connectionPromise = null;
            this.handleReconnect();
        };

        this.ws.onerror = (error) => {
            console.error("WebSocket error:", error);
            wsState.connectionStatus = "Connection error";
            this.rejectConnection?.(new Error("WebSocket connection failed"));
            this.connectionPromise = null;
        };

        this.ws.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                // Forward message to all handlers
                this.messageHandlers.forEach((handler) => handler(data));
            } catch (error) {
                console.error("Error parsing WebSocket message:", error);
            }
        };
    }

    private async handleReconnect() {
        if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            wsState.connectionStatus = "Connection failed. Please refresh the page.";
            return;
        }

        wsState.connectionStatus = `Reconnecting... (Attempt ${this.reconnectAttempts + 1}/${this.maxReconnectAttempts})`;
        setTimeout(() => {
            this.reconnectAttempts++;
            this.connect().catch(console.error);
        }, this.reconnectTimeout);
    }

    addMessageHandler(handler: (data: any) => void) {
        this.messageHandlers.add(handler);
    }

    removeMessageHandler(handler: (data: any) => void) {
        this.messageHandlers.delete(handler);
    }

    async send(message: any) {
        try {
            if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
                await this.connect();
            }

            if (this.ws?.readyState === WebSocket.OPEN) {
                this.ws.send(JSON.stringify(message));
            } else {
                throw new Error("WebSocket is not connected");
            }
        } catch (error) {
            console.error("Failed to send message:", error);
            throw error;
        }
    }

    disconnect() {
        if (this.ws) {
            this.ws.close();
            this.ws = null;
            this.connectionPromise = null;
        }
    }
}

export const sharedWebSocket = SharedWebSocketService.getInstance();
