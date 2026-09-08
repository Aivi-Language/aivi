import * as vscode from "vscode";
import { State, type LanguageClient } from "vscode-languageclient/node";

import { createClient } from "./client";
import { registerCommands } from "./commands";
import { SERVER_RESTART_SETTING_IDS } from "./contract";
import { getConfig } from "./config";
import { AsyncSerialQueue, errorText, withTimeout } from "./lifecycle";
import { StatusBarItem } from "./status";

interface ClientHandle {
  client: LanguageClient;
  stateSubscription: vscode.Disposable;
  expectedStop: boolean;
}

let clientHandle: ClientHandle | undefined;
let statusBar: StatusBarItem | undefined;
let outputChannel: vscode.LogOutputChannel | undefined;
let traceOutputChannel: vscode.LogOutputChannel | undefined;
let fileWatcher: vscode.FileSystemWatcher | undefined;
const lifecycle = new AsyncSerialQueue();

const THEME_NAME = "AIVI Dark";
const THEME_PROMPTED_KEY = "aivi.themePrompted";

function log(message: string): void {
  outputChannel?.info(message);
}

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

function configurationScope(): vscode.Uri | undefined {
  const active = vscode.window.activeTextEditor?.document;
  if (active?.languageId === "aivi") return active.uri;
  return vscode.workspace.textDocuments.find(
    (document) => document.languageId === "aivi"
  )?.uri;
}

async function stopClient(): Promise<void> {
  const handle = clientHandle;
  clientHandle = undefined;
  if (!handle) return;

  handle.expectedStop = true;
  handle.stateSubscription.dispose();
  log("Stopping language server");
  await handle.client.stop();
}

async function reportStartupFailure(error: unknown): Promise<void> {
  const message = errorText(error);
  outputChannel?.error(`Language server failed to start:\n${message}`);
  statusBar?.setStatus("crashed");
  const action = await vscode.window.showErrorMessage(
    "AIVI language server could not start. Check aivi.compiler.path and the AIVI output log.",
    "Open Settings",
    "Show Output"
  );
  if (action === "Open Settings") {
    await vscode.commands.executeCommand(
      "workbench.action.openSettings",
      "aivi.compiler.path"
    );
  } else if (action === "Show Output") {
    outputChannel?.show();
  }
}

async function restartOnce(): Promise<void> {
  await stopClient();
  const config = getConfig(configurationScope());
  log(`Compiler: ${config.compilerPath}`);
  statusBar?.setStatus("starting");
  statusBar?.show();

  if (!outputChannel || !traceOutputChannel || !fileWatcher) {
    throw new Error("extension resources were disposed before server startup");
  }

  const client = createClient(
    config,
    outputChannel,
    traceOutputChannel,
    fileWatcher
  );
  const handle: ClientHandle = {
    client,
    expectedStop: false,
    stateSubscription: client.onDidChangeState((event) => {
      log(`Client state: ${State[event.oldState]} -> ${State[event.newState]}`);
      if (event.newState === State.Starting) {
        statusBar?.setStatus("starting");
      } else if (event.newState === State.Running) {
        statusBar?.setStatus("running");
      } else if (
        !handle.expectedStop &&
        (event.newState === State.StartFailed || event.newState === State.Stopped)
      ) {
        statusBar?.setStatus("crashed");
      }
    }),
  };
  clientHandle = handle;

  try {
    await withTimeout(
      client.start(),
      config.compilerTimeout,
      `language server did not start within ${config.compilerTimeout} ms`
    );
    log("Language server started");
    statusBar?.setStatus("running");
  } catch (error) {
    await stopClient().catch((stopError) => {
      outputChannel?.warn(
        `Cleanup after failed startup also failed: ${errorText(stopError)}`
      );
    });
    await reportStartupFailure(error);
  }
}

function restart(): Promise<void> {
  return lifecycle.run(async () => {
    try {
      await restartOnce();
    } catch (error) {
      await reportStartupFailure(error);
    }
  });
}

function updateDiagnosticCount(): void {
  const aiviDocuments = new Set(
    vscode.workspace.textDocuments
      .filter((document) => document.languageId === "aivi")
      .map((document) => document.uri.toString())
  );
  const errorCount = vscode.languages.getDiagnostics().reduce(
    (count, [uri, diagnostics]) => count + (
      aiviDocuments.has(uri.toString())
        ? diagnostics.filter(
          (diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Error
        ).length
        : 0
    ),
    0
  );
  statusBar?.setErrorCount(errorCount);
}

export async function activate(
  context: vscode.ExtensionContext
): Promise<void> {
  outputChannel = vscode.window.createOutputChannel("AIVI", { log: true });
  traceOutputChannel = vscode.window.createOutputChannel("AIVI Trace", { log: true });
  fileWatcher = vscode.workspace.createFileSystemWatcher("**/*.aivi");
  statusBar = new StatusBarItem();
  context.subscriptions.push(
    outputChannel,
    traceOutputChannel,
    fileWatcher,
    statusBar
  );

  log("Extension activating");
  void promptThemeOnFirstInstall(context).catch((error) => {
    outputChannel?.warn(`Theme prompt failed: ${errorText(error)}`);
  });

  registerCommands(context, restart, outputChannel);

  context.subscriptions.push(
    vscode.workspace.onWillSaveTextDocument((event) => {
      if (event.document.languageId !== "aivi") return;
      if (!getConfig(event.document.uri).formatOnSave) return;
      event.waitUntil(
        vscode.commands.executeCommand<vscode.TextEdit[]>(
          "vscode.executeFormatDocumentProvider",
          event.document.uri
        ).then((edits) => edits ?? [])
      );
    }),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (SERVER_RESTART_SETTING_IDS.some((id) => event.affectsConfiguration(id))) {
        void restart();
      }
    }),
    vscode.languages.onDidChangeDiagnostics(updateDiagnosticCount)
  );

  const hasAiviDocument = vscode.workspace.textDocuments.some(
    (document) => document.languageId === "aivi"
  );
  if (hasAiviDocument) {
    await restart();
  } else {
    const subscription = vscode.workspace.onDidOpenTextDocument((document) => {
      if (document.languageId !== "aivi") return;
      subscription.dispose();
      void restart();
    });
    context.subscriptions.push(subscription);
  }
  log("Extension activated");
}

export async function deactivate(): Promise<void> {
  await lifecycle.run(stopClient);
  await lifecycle.drain();
  clientHandle = undefined;
  statusBar = undefined;
  outputChannel = undefined;
  traceOutputChannel = undefined;
  fileWatcher = undefined;
}
