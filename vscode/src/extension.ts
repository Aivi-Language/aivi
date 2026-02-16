import * as vscode from "vscode";
import * as fs from "node:fs";
import { spawnSync } from "node:child_process";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

function getCliCommand(): string {
  const config = vscode.workspace.getConfiguration("aivi");
  const cmd = config.get<string>("cli.command");
  return cmd && cmd.trim().length > 0 ? cmd : "aivi";
}

function docHasTests(doc: vscode.TextDocument): boolean {
  return /(^|\n)\s*@test\b/.test(doc.getText());
}

async function runAiviTest(target: string): Promise<void> {
  const cli = getCliCommand();
  const task = new vscode.Task(
    { type: "aivi", task: "test" },
    vscode.TaskScope.Workspace,
    "AIVI: test",
    "aivi",
    new vscode.ShellExecution(`${cli} test "${target}"`)
  );
  task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, clear: true };
  await vscode.tasks.executeTask(task);
}

export function activate(context: vscode.ExtensionContext) {
  const outputChannel = vscode.window.createOutputChannel("AIVI Language Server");
  const testOutput = vscode.window.createOutputChannel("AIVI Tests");
  context.subscriptions.push(testOutput);

  const isWindows = process.platform === "win32";
  const serverExe = isWindows ? "aivi-lsp.exe" : "aivi-lsp";
  const bundledServerPath = context.asAbsolutePath(`bin/${serverExe}`);

  const config = vscode.workspace.getConfiguration("aivi");
  const configuredCommand = config.get<string>("server.command");
  const configuredArgs = config.get<string[]>("server.args") ?? [];

  const hasCommand = (cmd: string): boolean => {
    const res = spawnSync(cmd, ["--version"], { stdio: "ignore" });
    return !res.error;
  };

  // Preferred order: user config -> bundled `aivi-lsp` -> `aivi-lsp` on PATH -> `aivi lsp`.
  let serverCommand: string;
  let serverArgs: string[];
  if (configuredCommand && configuredCommand.trim().length > 0) {
    serverCommand = configuredCommand;
    serverArgs = configuredArgs;
  } else if (fs.existsSync(bundledServerPath)) {
    serverCommand = bundledServerPath;
    serverArgs = [];
  } else if (hasCommand("aivi-lsp")) {
    serverCommand = "aivi-lsp";
    serverArgs = [];
  } else if (hasCommand("aivi")) {
    serverCommand = "aivi";
    serverArgs = ["lsp"];
  } else {
    serverCommand = "aivi-lsp";
    serverArgs = [];
  }

  if (!isWindows && serverCommand === bundledServerPath && fs.existsSync(bundledServerPath)) {
    try {
      fs.chmodSync(bundledServerPath, 0o755);
    } catch (err) {
      outputChannel.appendLine(`Failed to chmod aivi-lsp: ${String(err)}`);
    }
  }

  const serverOptions: ServerOptions = {
    command: serverCommand,
    args: serverArgs,
  };

  const fileWatchers = [
    vscode.workspace.createFileSystemWatcher("**/*.aivi"),
    vscode.workspace.createFileSystemWatcher("**/aivi.toml"),
    vscode.workspace.createFileSystemWatcher("**/Cargo.toml"),
    vscode.workspace.createFileSystemWatcher("**/specs/**/*"),
    vscode.workspace.createFileSystemWatcher("**/.gemini/skills/**/*"),
  ];
  for (const watcher of fileWatchers) {
    context.subscriptions.push(watcher);
  }

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ language: "aivi" }],
    synchronize: {
      fileEvents: fileWatchers,
      configurationSection: "aivi",
    },
    outputChannel,
    middleware: {
      provideDocumentFormattingEdits: (document, options, token, next) =>
        next(document, options, token),
      provideDocumentRangeFormattingEdits: (document, range, options, token, next) =>
        next(document, range, options, token),
    },
  };

  client = new LanguageClient("aivi", "Aivi Language Server", serverOptions, clientOptions);
  client.start();

  const runStatus = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 50);
  runStatus.text = "$(run) AIVI Tests";
  runStatus.tooltip = "Run AIVI integration tests";
  runStatus.color = new vscode.ThemeColor("terminal.ansiGreen");
  runStatus.command = "aivi.runTests";
  context.subscriptions.push(runStatus);

  const updateStatusVisibility = (): void => {
    const doc = vscode.window.activeTextEditor?.document;
    if (!doc || doc.languageId !== "aivi") {
      runStatus.hide();
      return;
    }
    runStatus.show();
  };
  updateStatusVisibility();
  context.subscriptions.push(vscode.window.onDidChangeActiveTextEditor(updateStatusVisibility));

  context.subscriptions.push(
    vscode.commands.registerCommand("aivi.restartServer", async () => {
      outputChannel.appendLine("Restarting AIVI Language Server...");
      const prev = client;
      client = undefined;
      await prev?.stop();
      client = new LanguageClient("aivi", "Aivi Language Server", serverOptions, clientOptions);
      client.start();
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("aivi.runTests", async () => {
      // Prefer: current file if it has tests, else run the canonical integration test suite.
      const doc = vscode.window.activeTextEditor?.document;
      if (doc && doc.languageId === "aivi" && docHasTests(doc)) {
        await runAiviTest(doc.uri.fsPath);
      } else {
        await runAiviTest("integration-tests/**");
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("aivi.runTestsFile", async () => {
      const doc = vscode.window.activeTextEditor?.document;
      if (!doc || doc.languageId !== "aivi") {
        return;
      }
      await runAiviTest(doc.uri.fsPath);
    })
  );

  context.subscriptions.push(
    vscode.tasks.onDidEndTaskProcess((e) => {
      if (e.execution.task.source !== "aivi" || e.execution.task.name !== "AIVI: test") {
        return;
      }
      const ws = vscode.workspace.workspaceFolders?.[0];
      if (!ws) {
        return;
      }
      const failedPath = vscode.Uri.joinPath(ws.uri, "target", "aivi-test-failed-files.txt").fsPath;
      const passedPath = vscode.Uri.joinPath(ws.uri, "target", "aivi-test-passed-files.txt").fsPath;

      const failedFiles = fs.existsSync(failedPath)
        ? fs
            .readFileSync(failedPath, "utf8")
            .split(/\r?\n/)
            .map((s) => s.trim())
            .filter((s) => s.length > 0)
        : [];
      const passedFiles = fs.existsSync(passedPath)
        ? fs
            .readFileSync(passedPath, "utf8")
            .split(/\r?\n/)
            .map((s) => s.trim())
            .filter((s) => s.length > 0)
        : [];

      testOutput.clear();
      testOutput.appendLine(`exitCode: ${String(e.exitCode)}`);
      testOutput.appendLine(`passedFiles: ${passedFiles.length}`);
      testOutput.appendLine(`failedFiles: ${failedFiles.length}`);
      if (failedFiles.length > 0) {
        testOutput.appendLine("");
        testOutput.appendLine("Failed files:");
        for (const f of failedFiles) {
          testOutput.appendLine(`- ${f}`);
        }
      }

      if (e.exitCode === 0) {
        void vscode.window.showInformationMessage(`AIVI tests passed (${passedFiles.length} files).`);
      } else {
        void vscode.window.showErrorMessage(
          `AIVI tests failed (${failedFiles.length} files). See "AIVI Tests" output.`
        );
        testOutput.show(true);
      }
    })
  );

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
