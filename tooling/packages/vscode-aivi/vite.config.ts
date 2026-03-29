import { defineConfig } from "vite";
import { resolve } from "path";

export default defineConfig({
  build: {
    lib: {
      entry: resolve(__dirname, "src/extension.ts"),
      formats: ["cjs"],
      fileName: () => "extension.js",
    },
    outDir: "dist",
    rollupOptions: {
      external: ["vscode", "path", "fs", "child_process", "net", "os", "crypto"],
    },
    sourcemap: true,
    minify: false,
    target: "node18",
    emptyOutDir: true,
  },
  resolve: {
    // Prevent Vite from using the "browser" field in package.json,
    // which remaps vscode-languageclient/node to the browser bundle.
    mainFields: ["module", "main"],
    conditions: ["node"],
  },
});
