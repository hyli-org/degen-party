import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// https://vite.dev/config/
export default defineConfig({
    plugins: [react()],
    build: {
        lib: {
            entry: "src/lib.ts",
            name: "HyliWallet",
            fileName: (format) => `hyli-wallet.${format}.js`,
            formats: ["es", "umd"],
        },
        rollupOptions: {
            // Bundle everything, do not externalize react or react-dom
        },
        outDir: "dist",
        emptyOutDir: true,
    },
});
