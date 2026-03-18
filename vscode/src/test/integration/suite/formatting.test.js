const assert = require('node:assert');
const vscode = require('vscode');

async function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function getFormattingEditsWithRetry(
  uri,
  options,
  { timeoutMs = 15_000, intervalMs = 200 } = {}
) {
  const start = Date.now();
  let lastEdits = [];

  while (Date.now() - start < timeoutMs) {
    // eslint-disable-next-line no-await-in-loop
    const edits =
      (await vscode.commands.executeCommand('vscode.executeFormatDocumentProvider', uri, options)) ??
      [];
    lastEdits = edits;
    if (edits.length > 0) {
      return edits;
    }
    // eslint-disable-next-line no-await-in-loop
    await sleep(intervalMs);
  }

  return lastEdits;
}

suite('LSP: document formatting', () => {
  test('format document returns edits for unformatted AIVI', async () => {
    const ext = vscode.extensions.getExtension('aivi.aivi-vscode');
    assert.ok(ext, 'Expected extension aivi.aivi-vscode to be available');
    await ext.activate();

    const wsFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(wsFolder, 'Expected a workspace folder (fixture workspace)');

    const uri = vscode.Uri.joinPath(wsFolder.uri, 'formatting.aivi');
    const doc = await vscode.workspace.openTextDocument(uri);
    const editor = await vscode.window.showTextDocument(doc);

    const edits = await getFormattingEditsWithRetry(uri, {
      insertSpaces: true,
      tabSize: 2,
    });

    assert.ok(Array.isArray(edits), 'Expected a list of text edits');
    assert.ok(edits.length > 0, 'Expected formatting edits');

    const workspaceEdit = new vscode.WorkspaceEdit();
    workspaceEdit.set(uri, edits);
    const applied = await vscode.workspace.applyEdit(workspaceEdit);
    assert.ok(applied, 'Expected VS Code to apply formatting edits');

    const formattedText = editor.document.getText();
    assert.ok(
      formattedText.includes('main = do Effect { _ <- print "hi" }'),
      `Expected formatter output to normalize spacing inside the effect block, got: ${JSON.stringify(formattedText)}`
    );

    await vscode.commands.executeCommand('workbench.action.files.revert');
  });
});
