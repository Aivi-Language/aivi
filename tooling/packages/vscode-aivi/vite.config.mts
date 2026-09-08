import { builtinModules } from "node:module";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

// Externalize every Node.js built-in (both bare and node:-prefixed)
// plus the vscode host module. Missing entries here cause Vite to
// replace the module with an empty browser stub, which silently
// breaks vscode-languageclient's JSON-RPC layer at runtime.
const nodeExternals = [
  "vscode",
  ...builtinModules,
  ...builtinModules.map((module) => `node:${module}`),
];
const configDir = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  build: {
    lib: {
      entry: resolve(configDir, "src/extension.ts"),
      formats: ["cjs"],
      fileName: () => "extension.js",
    },
    outDir: "dist",
    rollupOptions: {
      external: nodeExternals,
    },
    sourcemap: true,
    minify: false,
    target: "node18",
    chunkSizeWarningLimit: 1024,
    emptyOutDir: true,
  },
  resolve: {
    // Prevent Vite from using the "browser" field in package.json,
    // which remaps vscode-languageclient/node to the browser bundle.
    mainFields: ["module", "main"],
    conditions: ["node"],
  },
});
