import * as path from "node:path";

import { describe, expect, it } from "vitest";

import { resolveServerCommand } from "../../serverResolution";

function createFs(
  files: Array<{ filePath: string; mtimeMs?: number }>,
): {
  existsSync: (filePath: string) => boolean;
  statSync: (filePath: string) => { mtimeMs: number };
} {
  const mtimes = new Map(files.map(({ filePath, mtimeMs = 1 }) => [filePath, mtimeMs]));
  return {
    existsSync: (filePath: string) => mtimes.has(filePath),
    statSync: (filePath: string) => ({ mtimeMs: mtimes.get(filePath) ?? 0 }),
  };
}

describe("resolveServerCommand", () => {
  it("prefers an explicit user-configured command", () => {
    const resolution = resolveServerCommand({
      extensionPath: "/repo/vscode",
      configuredCommand: "/custom/aivi-lsp",
      configuredArgs: ["--stdio"],
    });

    expect(resolution).toEqual({
      command: "/custom/aivi-lsp",
      args: ["--stdio"],
      source: "configured",
    });
  });

  it("prefers a newer workspace build over the bundled binary in a source checkout", () => {
    const extensionPath = "/repo/vscode";
    const repoRoot = path.resolve(extensionPath, "..");
    const exeName = "aivi-lsp";
    const bundledPath = path.join(extensionPath, "bin", exeName);
    const workspaceDebugPath = path.join(repoRoot, "target", "debug", exeName);
    const { existsSync, statSync } = createFs([
      { filePath: path.join(repoRoot, "Cargo.toml") },
      { filePath: path.join(repoRoot, "crates", "aivi_lsp", "Cargo.toml") },
      { filePath: path.join(repoRoot, "vscode", "package.json") },
      { filePath: bundledPath, mtimeMs: 100 },
      { filePath: workspaceDebugPath, mtimeMs: 200 },
    ]);

    const resolution = resolveServerCommand({
      extensionPath,
      existsSync,
      statSync,
    });

    expect(resolution).toEqual({
      command: workspaceDebugPath,
      args: [],
      source: "workspace-debug",
    });
  });

  it("keeps the bundled binary when it is newer than the workspace build", () => {
    const extensionPath = "/repo/vscode";
    const repoRoot = path.resolve(extensionPath, "..");
    const exeName = "aivi-lsp";
    const bundledPath = path.join(extensionPath, "bin", exeName);
    const workspaceReleasePath = path.join(repoRoot, "target", "release", exeName);
    const { existsSync, statSync } = createFs([
      { filePath: path.join(repoRoot, "Cargo.toml") },
      { filePath: path.join(repoRoot, "crates", "aivi_lsp", "Cargo.toml") },
      { filePath: path.join(repoRoot, "vscode", "package.json") },
      { filePath: bundledPath, mtimeMs: 300 },
      { filePath: workspaceReleasePath, mtimeMs: 150 },
    ]);

    const resolution = resolveServerCommand({
      extensionPath,
      existsSync,
      statSync,
    });

    expect(resolution).toEqual({
      command: bundledPath,
      args: [],
      source: "bundled",
    });
  });

  it("falls back to PATH commands when no local binary is available", () => {
    const resolution = resolveServerCommand({
      extensionPath: "/tmp/extension",
      existsSync: () => false,
      hasCommand: (command: string) => command === "aivi",
    });

    expect(resolution).toEqual({
      command: "aivi",
      args: ["lsp"],
      source: "path-aivi",
    });
  });
});
