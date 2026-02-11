import * as vscode from "vscode";
import * as fs from "node:fs";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext) {
  const isWindows = process.platform === "win32";
  const serverExe = isWindows ? "aivi-lsp.exe" : "aivi-lsp";
  const bundledServerPath = context.asAbsolutePath(`bin/${serverExe}`);
  if (!isWindows && fs.existsSync(bundledServerPath)) {
    try {
      fs.chmodSync(bundledServerPath, 0o755);
    } catch (err) {
      console.warn(`Failed to chmod aivi-lsp: ${String(err)}`);
    }
  }
  const serverOptions: ServerOptions = {
    command: fs.existsSync(bundledServerPath) ? bundledServerPath : "aivi-lsp",
    args: [],
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ language: "aivi" }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.aivi"),
    },
    outputChannel: vscode.window.createOutputChannel("AIVI Language Server"),
  };

  client = new LanguageClient("aivi", "Aivi Language Server", serverOptions, clientOptions);
  client.start();
  context.subscriptions.push(
    new vscode.Disposable(() => {
      void client?.stop();
    })
  );

}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
