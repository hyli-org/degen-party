import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import copy from "rollup-plugin-copy";

const wasmContentTypePlugin = {
    name: "wasm-content-type-plugin",
    configureServer(server: any) {
        server.middlewares.use((req: any, res: any, next: any) => {
            if (req.url.endsWith(".wasm")) {
                res.setHeader("Content-Type", "application/wasm");
            }
            next();
        });
    },
};

// https://vite.dev/config/
export default defineConfig({
    plugins: [
        vue(),
        tailwindcss(),
        wasmContentTypePlugin,
        copy({
            targets: [{ src: "node_modules/**/*.wasm", dest: "node_modules/.vite/deps" }],
            copySync: true,
            hook: "buildStart",
        }),
    ],
});
