import * as vscode from "vscode";
import { dirname } from "node:path";

import { COMMAND_IDS } from "./contract";
import { getConfig } from "./config";
import {
  type CompilerInvocation,
  checkInvocation,
  testInvocation,
} from "./invocations";

async function executeCompilerTask(
  name: string,
  kind: "check" | "test",
  uri: vscode.Uri,
  invocation: CompilerInvocation
): Promise<void> {
  const folder = vscode.workspace.getWorkspaceFolder(uri);
  const scope = folder ?? vscode.TaskScope.Workspace;
  const execution = new vscode.ProcessExecution(
    invocation.command,
    invocation.args,
    { cwd: invocation.cwd }
  );
  const task = new vscode.Task(
    { type: "aivi", command: kind, file: uri.fsPath },
    scope,
    name,
    "aivi",
    execution
  );
  task.presentationOptions = {
    reveal: vscode.TaskRevealKind.Always,
    panel: vscode.TaskPanelKind.Dedicated,
    clear: true,
  };
  await vscode.tasks.executeTask(task);
}

function fileUri(value: unknown): vscode.Uri | undefined {
  try {
    const uri = value instanceof vscode.Uri
      ? value
      : typeof value === "string"
        ? vscode.Uri.parse(value, true)
        : undefined;
    return uri?.scheme === "file" ? uri : undefined;
  } catch {
    return undefined;
  }
}

function commandCwd(uri: vscode.Uri): string {
  return vscode.workspace.getWorkspaceFolder(uri)?.uri.fsPath
    ?? dirname(uri.fsPath);
}

export function registerCommands(
  context: vscode.ExtensionContext,
  restart: () => Promise<void>,
  outputChannel: vscode.LogOutputChannel
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(COMMAND_IDS.restartServer, async () => {
      await restart();
      outputChannel.show();
    }),

    vscode.commands.registerCommand(COMMAND_IDS.showOutputChannel, () => {
      outputChannel.show();
    }),

    vscode.commands.registerCommand(COMMAND_IDS.formatDocument, async () => {
      const editor = vscode.window.activeTextEditor;
      if (editor?.document.languageId === "aivi") {
        await vscode.commands.executeCommand("editor.action.formatDocument");
      }
    }),

    vscode.commands.registerCommand(COMMAND_IDS.checkFile, async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor || editor.document.languageId !== "aivi") return;
      const document = editor.document;
      await document.save();
      if (document.uri.scheme !== "file") {
        await vscode.window.showWarningMessage(
          "Save the AIVI document to disk before checking it."
        );
        return;
      }
      const config = getConfig(document.uri);
      await executeCompilerTask(
        `Check ${document.uri.path.split("/").at(-1) ?? document.uri.fsPath}`,
        "check",
        document.uri,
        checkInvocation(config, commandCwd(document.uri), document.uri.fsPath)
      );
    }),

    vscode.commands.registerCommand(
      COMMAND_IDS.runTest,
      async (fileUriArgument?: string | vscode.Uri, testNameArgument?: string) => {
        const activeDocument = vscode.window.activeTextEditor?.document;
        const uri = fileUri(fileUriArgument) ?? (
          activeDocument?.languageId === "aivi" && activeDocument.uri.scheme === "file"
            ? activeDocument.uri
            : undefined
        );
        if (!uri) {
          await vscode.window.showErrorMessage(
            "AIVI tests require a saved file document."
          );
          return;
        }
        const testName = testNameArgument?.trim() || await vscode.window.showInputBox({
          prompt: "AIVI test value to run",
          validateInput: (value) => value.trim() ? undefined : "Enter a test value name.",
        });
        if (!testName?.trim()) return;

        const config = getConfig(uri);
        await executeCompilerTask(
          `Test ${testName.trim()}`,
          "test",
          uri,
          testInvocation(config, commandCwd(uri), uri.fsPath, testName.trim())
        );
      }
    )
  );
}
