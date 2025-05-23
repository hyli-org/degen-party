import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
//import { analyzer } from "vite-bundle-analyzer";

// https://vite.dev/config/
export default defineConfig({
    plugins: [react()], //, analyzer({ include: /wallet\.es\.js$/ })],
    build: {
        sourcemap: true,
        minify: false,
        lib: {
            entry: "src/lib.ts",
            name: "HyliWallet",
            fileName: (format) => `hyli-wallet.${format}.js`,
            formats: ["es", "cjs"],
        },
        rollupOptions: {
            external: ["barretenberg", "barretenberg/threads"],
        },
        outDir: "dist",
        emptyOutDir: true,
    },
});
