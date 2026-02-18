import { defineConfig } from "vite";

export default defineConfig({
  build: {
    lib: {
      entry: "src/main.ts",
      name: "AiviServerHtmlClient",
      formats: ["iife"],
      fileName: () => "aivi-server-html-client.js"
    },
    outDir: "dist",
    sourcemap: true,
    emptyOutDir: true,
    rollupOptions: { output: { inlineDynamicImports: true } }
  }
});

