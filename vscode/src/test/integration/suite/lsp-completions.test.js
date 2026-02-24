const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vscode = require('vscode');

async function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function completionLabels(result) {
  if (!result) return [];
  const items = Array.isArray(result) ? result : result.items ?? [];
  return items.map((item) => (typeof item.label === 'string' ? item.label : item.label.label));
}

async function getCompletionsWithRetry(
  uri,
  position,
  { timeoutMs = 15_000, intervalMs = 200, acceptLabels = (labels) => labels.length > 0 } = {}
) {
  const start = Date.now();
  let lastLabels = [];

  while (Date.now() - start < timeoutMs) {
    // eslint-disable-next-line no-await-in-loop
    const res = await vscode.commands.executeCommand(
      'vscode.executeCompletionItemProvider',
      uri,
      position
    );
    lastLabels = completionLabels(res);
    if (acceptLabels(lastLabels)) return lastLabels;
    // eslint-disable-next-line no-await-in-loop
    await sleep(intervalMs);
  }

  return lastLabels;
}

suite('LSP: completions inside ~html/~gtk regions', () => {
  test('completions do not crash in tag/attr positions', async () => {
    const ext = vscode.extensions.getExtension('aivi.aivi-vscode');
    assert.ok(ext, 'Expected extension aivi.aivi-vscode to be available');

    const exeName = process.platform === 'win32' ? 'aivi-lsp.exe' : 'aivi-lsp';
    const bundledLsp = path.join(ext.extensionPath, 'bin', exeName);
    assert.ok(
      fs.existsSync(bundledLsp),
      `Expected bundled LSP binary at ${bundledLsp}. Run \`pnpm run build\` from vscode/ to generate it.`
    );

    await ext.activate();

    const wsFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(wsFolder, 'Expected a workspace folder (fixture workspace)');

    const uri = vscode.Uri.joinPath(wsFolder.uri, 'main.aivi');
    const doc = await vscode.workspace.openTextDocument(uri);
    await vscode.window.showTextDocument(doc);

    const text = doc.getText();
    const tagOffset = text.indexOf('<div') + 3; // <di|v
    const attrOffset = text.indexOf('class') + 2; // cl|ass
    assert.ok(tagOffset >= 3, 'Expected `<div` in fixture');
    assert.ok(attrOffset >= 2, 'Expected `class` in fixture');

    const tagPos = doc.positionAt(tagOffset);
    const attrPos = doc.positionAt(attrOffset);

    const tagLabels = await getCompletionsWithRetry(uri, tagPos);
    const attrLabels = await getCompletionsWithRetry(uri, attrPos);

    assert.ok(tagLabels.length > 0, 'Expected non-empty completions inside tag name');
    assert.ok(attrLabels.length > 0, 'Expected non-empty completions inside attribute name');

    // The current AIVI LSP returns AIVI keywords/sigils broadly, even in ~html.
    assert.ok(tagLabels.includes('~<html></html>'), 'Expected AIVI sigil completion inside ~html');
    assert.ok(attrLabels.includes('~<html></html>'), 'Expected AIVI sigil completion inside ~html');
  });

  test('completions include in-file symbols inside `{...}` attribute values', async () => {
    const wsFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(wsFolder, 'Expected a workspace folder (fixture workspace)');

    const uri = vscode.Uri.joinPath(wsFolder.uri, 'main.aivi');
    const doc = await vscode.workspace.openTextDocument(uri);
    await vscode.window.showTextDocument(doc);

    const text = doc.getText();
    const braceOffset = text.indexOf('{doAiviFn}') + 3; // {do|AiviFn}
    assert.ok(braceOffset >= 3, 'Expected `{doAiviFn}` in fixture');

    const pos = doc.positionAt(braceOffset);
    const labels = await getCompletionsWithRetry(uri, pos);

    assert.ok(labels.length > 0, 'Expected non-empty completions inside `{...}`');
    assert.ok(
      labels.includes('doAiviFn') || labels.includes('~<html></html>'),
      'Expected completion response inside `{...}`'
    );
  });

  test('completions run in plain HTML text content without errors', async () => {
    const wsFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(wsFolder, 'Expected a workspace folder (fixture workspace)');

    const uri = vscode.Uri.joinPath(wsFolder.uri, 'main.aivi');
    const doc = await vscode.workspace.openTextDocument(uri);
    await vscode.window.showTextDocument(doc);

    const text = doc.getText();
    const okOffset = text.indexOf('ok') + 1;
    assert.ok(okOffset >= 1, 'Expected `ok` in fixture');

    const pos = doc.positionAt(okOffset);
    const labels = await getCompletionsWithRetry(uri, pos);

    assert.ok(Array.isArray(labels), 'Expected a completion label array');
  });

  test('completions do not crash in GTK tag/attr positions', async () => {
    const wsFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(wsFolder, 'Expected a workspace folder (fixture workspace)');

    const uri = vscode.Uri.joinPath(wsFolder.uri, 'main.aivi');
    const doc = await vscode.workspace.openTextDocument(uri);
    await vscode.window.showTextDocument(doc);

    const text = doc.getText();
    const gtkTagOffset = text.indexOf('<object class="GtkBox"') + 3; // <ob|ject
    const gtkAttrOffset = text.indexOf('class="GtkBox"') + 2; // cl|ass
    assert.ok(gtkTagOffset >= 3, 'Expected `<object` in fixture');
    assert.ok(gtkAttrOffset >= 2, 'Expected `class` in GTK fixture');

    const gtkTagPos = doc.positionAt(gtkTagOffset);
    const gtkAttrPos = doc.positionAt(gtkAttrOffset);

    const gtkTagLabels = await getCompletionsWithRetry(uri, gtkTagPos);
    const gtkAttrLabels = await getCompletionsWithRetry(uri, gtkAttrPos);

    assert.ok(gtkTagLabels.length > 0, 'Expected non-empty completions inside GTK tag name');
    assert.ok(
      gtkAttrLabels.length > 0,
      'Expected non-empty completions inside GTK attribute name'
    );
    assert.ok(
      gtkTagLabels.includes('~<gtk></gtk>'),
      'Expected AIVI GTK sigil completion inside ~gtk'
    );
    assert.ok(
      gtkAttrLabels.includes('~<gtk></gtk>'),
      'Expected AIVI GTK sigil completion inside ~gtk'
    );
  });
});
