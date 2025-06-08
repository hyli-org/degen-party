import { createApp } from "vue";
import "./style.css";
import { sharedWebSocket } from "./utils/shared-websocket";
import App from "./App.vue";
import router from "./router";

// Initialize the shared WebSocket connection
sharedWebSocket.connect().catch(console.error);

const app = createApp(App);
app.use(router);
app.mount("#app");
