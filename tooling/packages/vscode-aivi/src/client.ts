import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  RevealOutputChannelOn,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";
import type { AiviConfig } from "./config";
import { serverInitializationOptions } from "./contract";

export function createClient(
  config: AiviConfig,
  outputChannel: vscode.LogOutputChannel,
  traceOutputChannel: vscode.LogOutputChannel,
  fileWatcher: vscode.FileSystemWatcher
): LanguageClient {
  // The aivi binary links against GTK4/libwayland even for headless
  // subcommands like `lsp`. Prevent display-server interaction by
  // clearing Wayland/X11 env vars — otherwise the child process can
  // corrupt the compositor's keyboard state, causing "stuck key"
  // auto-repeat in the editor.
  const lspEnv: Record<string, string> = { ...process.env } as Record<
    string,
    string
  >;
  delete lspEnv["WAYLAND_DISPLAY"];
  delete lspEnv["WAYLAND_SOCKET"];
  delete lspEnv["DISPLAY"];
  delete lspEnv["GDK_BACKEND"];

  const executable = {
    command: config.compilerPath,
    args: ["lsp"],
    transport: TransportKind.stdio,
    options: { env: lspEnv },
  };
  const serverOptions: ServerOptions = {
    run: executable,
    debug: executable,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { language: "aivi", scheme: "file" },
      { language: "aivi", scheme: "untitled" },
    ],
    synchronize: {
      fileEvents: fileWatcher,
    },
    initializationOptions: serverInitializationOptions(config),
    initializationFailedHandler: () => false,
    connectionOptions: { maxRestartCount: 2 },
    outputChannel,
    traceOutputChannel,
    revealOutputChannelOn: RevealOutputChannelOn.Never,
    markdown: { isTrusted: true, supportHtml: false },
  };

  return new LanguageClient(
    "aivi",
    "AIVI Language Server",
    serverOptions,
    clientOptions
  );
}
