import assert from "node:assert/strict";
import * as vscode from "vscode";

async function until<T>(read: () => T | PromiseLike<T>, ready: (value: T) => boolean): Promise<T> {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const value = await read();
    if (ready(value)) return value;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error("extension host operation timed out");
}

/** Executed by the real VS Code extension host, not by node --test. */
export async function run(): Promise<void> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  assert.ok(folder);
  const uri = vscode.Uri.joinPath(folder.uri, "main.aivi");
  const document = await vscode.workspace.openTextDocument(uri);
  await vscode.window.showTextDocument(document);
  assert.equal(document.languageId, "aivi");
  const extension = vscode.extensions.getExtension("aivi-lang.vscode-aivi");
  assert.ok(extension);
  await extension.activate();

  const completion = await until(
    () => vscode.commands.executeCommand<vscode.CompletionList>("vscode.executeCompletionItemProvider", uri, new vscode.Position(2, 6)),
    (result) => !!result?.items.some((item) => item.label === "input")
  );
  assert.ok(completion);
  const lenses = await vscode.commands.executeCommand<vscode.CodeLens[]>("vscode.executeCodeLensProvider", uri);
  assert.ok(lenses?.some((lens) => lens.command?.command === "aivi.runTest"));

  // Verify command wiring actually saves buffers before starting compiler tasks.
  const edit = new vscode.WorkspaceEdit();
  edit.insert(uri, new vscode.Position(document.lineCount, 0), "\n// changed in editor\n");
  assert.ok(await vscode.workspace.applyEdit(edit));
  assert.ok(document.isDirty);
  const started: vscode.Task[] = [];
  const subscription = vscode.tasks.onDidStartTask((event) => started.push(event.execution.task));
  try {
    await vscode.commands.executeCommand("aivi.runTest", uri, "selected");
    assert.equal(document.isDirty, false);
    await until(() => started, (tasks) => tasks.some((task) => task.definition.command === "test"));
    await vscode.commands.executeCommand("aivi.checkFile");
    await until(() => started, (tasks) => tasks.some((task) => task.definition.command === "check"));
  } finally { subscription.dispose(); }

  await vscode.commands.executeCommand("aivi.restartServer");
  await until(
    () => vscode.commands.executeCommand<vscode.CompletionList>("vscode.executeCompletionItemProvider", uri, new vscode.Position(2, 6)),
    (result) => !!result?.items.some((item) => item.label === "input")
  );
  for (const execution of vscode.tasks.taskExecutions) execution.terminate();
  await vscode.workspace.fs.writeFile(vscode.Uri.joinPath(folder.uri, ".host-test-passed"), Buffer.from("passed"));
}
