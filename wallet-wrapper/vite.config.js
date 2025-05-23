import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
//import { analyzer } from "vite-bundle-analyzer";

// https://vite.dev/config/
export default defineConfig((mode) => ({
    define: {
        // Necessary for react-dom to behave
        "process.env.NODE_ENV": JSON.stringify(mode.mode),
    },
    build: {
        sourcemap: true,
        minify: true,
        lib: {
            entry: "src/lib.ts",
            name: "HyliWallet",
            fileName: (format) => `hyli-wallet.${format}.js`,
            formats: ["es", "cjs"],
        },
        rollupOptions: {
            external: ["hyli-wallet"],
        },
        outDir: "dist",
        emptyOutDir: true,
    },
    plugins: [react()], //, analyzer()],
}));
