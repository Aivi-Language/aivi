import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";
import { createClient } from "./client";
import { StatusBarItem } from "./status";
import { registerCommands } from "./commands";

let client: LanguageClient | undefined;
let statusBar: StatusBarItem | undefined;

const THEME_NAME = "AIVI Dark";
const THEME_PROMPTED_KEY = "aivi.themePrompted";

async function promptThemeOnFirstInstall(
  context: vscode.ExtensionContext
): Promise<void> {
  if (context.globalState.get<boolean>(THEME_PROMPTED_KEY)) return;
  await context.globalState.update(THEME_PROMPTED_KEY, true);

  const current = vscode.workspace
    .getConfiguration("workbench")
    .get<string>("colorTheme");
  if (current === THEME_NAME) return;

  const choice = await vscode.window.showInformationMessage(
    "Welcome to AIVI! Would you like to switch to the AIVI Dark color theme?",
    "Apply Theme",
    "Not Now"
  );
  if (choice === "Apply Theme") {
    await vscode.workspace
      .getConfiguration("workbench")
      .update("colorTheme", THEME_NAME, vscode.ConfigurationTarget.Global);
  }
}

export async function activate(
  context: vscode.ExtensionContext
): Promise<void> {
  const outputChannel = vscode.window.createOutputChannel("AIVI");
  const traceOutputChannel = vscode.window.createOutputChannel("AIVI Trace");

  void promptThemeOnFirstInstall(context);

  statusBar = new StatusBarItem();
  context.subscriptions.push({ dispose: () => statusBar?.dispose() });

  const restart = async (): Promise<void> => {
    if (client) {
      await client.stop();
      client = undefined;
    }
    statusBar?.setStatus("starting");
    statusBar?.show();
    client = createClient(context, outputChannel, traceOutputChannel);
    client.onDidChangeState((event) => {
      // State 2 = Running, State 1 = Starting, State 3 = Stopped
      if (event.newState === 2) {
        statusBar?.setStatus("running");
      } else if (event.newState === 3) {
        statusBar?.setStatus("crashed");
      }
    });
    try {
      await client.start();
      statusBar?.setStatus("running");
    } catch (err) {
      statusBar?.setStatus("crashed");
      outputChannel.appendLine(`Failed to start AIVI language server: ${err}`);
    }
  };

  registerCommands(context, () => client, restart, outputChannel);

  // Register format on save if configured
  context.subscriptions.push(
    vscode.workspace.onWillSaveTextDocument(async (event) => {
      if (event.document.languageId !== "aivi") return;
      const config = vscode.workspace.getConfiguration("aivi");
      if (!config.get<boolean>("format.onSave")) return;
      event.waitUntil(
        vscode.commands.executeCommand<vscode.TextEdit[]>(
          "vscode.executeFormatDocumentProvider",
          event.document.uri
        )
      );
    })
  );

  // Watch config changes to restart server on compiler path change
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (
        e.affectsConfiguration("aivi.compiler.path") ||
        e.affectsConfiguration("aivi.compiler.args")
      ) {
        void restart();
      }
    })
  );

  // Only start the LSP when an .aivi file is open or becomes open
  const startLspWhenNeeded = async (): Promise<void> => {
    if (vscode.workspace.textDocuments.some((d) => d.languageId === "aivi")) {
      await restart();
      return;
    }
    const sub = vscode.workspace.onDidOpenTextDocument(async (doc) => {
      if (doc.languageId !== "aivi") return;
      sub.dispose();
      await restart();
    });
    context.subscriptions.push(sub);
  };

  await startLspWhenNeeded();
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
}
