import * as fs from "node:fs";
import * as path from "node:path";

export type ServerCommandSource =
  | "configured"
  | "workspace-debug"
  | "workspace-release"
  | "bundled"
  | "path-aivi-lsp"
  | "path-aivi"
  | "fallback-aivi-lsp";

export type ServerCommandResolution = {
  command: string;
  args: string[];
  source: ServerCommandSource;
};

type ExistsSync = (filePath: string) => boolean;
type HasCommand = (command: string) => boolean;
type StatSync = (filePath: string) => { mtimeMs: number };

type ResolveServerCommandOptions = {
  extensionPath: string;
  configuredCommand?: string;
  configuredArgs?: string[];
  isWindows?: boolean;
  existsSync?: ExistsSync;
  hasCommand?: HasCommand;
  statSync?: StatSync;
};

function safeMtimeMs(filePath: string, existsSync: ExistsSync, statSync: StatSync): number {
  if (!existsSync(filePath)) {
    return Number.NEGATIVE_INFINITY;
  }
  try {
    return statSync(filePath).mtimeMs;
  } catch {
    return Number.NEGATIVE_INFINITY;
  }
}

function repoRootForSourceCheckout(
  extensionPath: string,
  existsSync: ExistsSync,
): string | undefined {
  const repoRoot = path.resolve(extensionPath, "..");
  const requiredPaths = [
    path.join(repoRoot, "Cargo.toml"),
    path.join(repoRoot, "crates", "aivi_lsp", "Cargo.toml"),
    path.join(repoRoot, "vscode", "package.json"),
  ];
  return requiredPaths.every((candidate) => existsSync(candidate)) ? repoRoot : undefined;
}

export function resolveServerCommand({
  extensionPath,
  configuredCommand,
  configuredArgs = [],
  isWindows = process.platform === "win32",
  existsSync = fs.existsSync,
  hasCommand = () => false,
  statSync = fs.statSync,
}: ResolveServerCommandOptions): ServerCommandResolution {
  if (configuredCommand && configuredCommand.trim().length > 0) {
    return {
      command: configuredCommand,
      args: configuredArgs,
      source: "configured",
    };
  }

  const exeName = isWindows ? "aivi-lsp.exe" : "aivi-lsp";
  const bundledServerPath = path.join(extensionPath, "bin", exeName);
  const bundledMtimeMs = safeMtimeMs(bundledServerPath, existsSync, statSync);
  const repoRoot = repoRootForSourceCheckout(extensionPath, existsSync);

  if (repoRoot) {
    const workspaceCandidates = [
      {
        source: "workspace-debug" as const,
        command: path.join(repoRoot, "target", "debug", exeName),
      },
      {
        source: "workspace-release" as const,
        command: path.join(repoRoot, "target", "release", exeName),
      },
    ]
      .filter((candidate) => existsSync(candidate.command))
      .sort(
        (left, right) =>
          safeMtimeMs(right.command, existsSync, statSync) -
          safeMtimeMs(left.command, existsSync, statSync),
      );

    const newestWorkspaceBuild = workspaceCandidates[0];
    if (
      newestWorkspaceBuild &&
      safeMtimeMs(newestWorkspaceBuild.command, existsSync, statSync) >= bundledMtimeMs
    ) {
      return {
        command: newestWorkspaceBuild.command,
        args: [],
        source: newestWorkspaceBuild.source,
      };
    }
  }

  if (existsSync(bundledServerPath)) {
    return {
      command: bundledServerPath,
      args: [],
      source: "bundled",
    };
  }

  if (hasCommand("aivi-lsp")) {
    return {
      command: "aivi-lsp",
      args: [],
      source: "path-aivi-lsp",
    };
  }

  if (hasCommand("aivi")) {
    return {
      command: "aivi",
      args: ["lsp"],
      source: "path-aivi",
    };
  }

  return {
    command: "aivi-lsp",
    args: [],
    source: "fallback-aivi-lsp",
  };
}
