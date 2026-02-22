import * as vscode from "vscode";
import * as fs from "node:fs";
import * as path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";
import { extractHoverToken, fallbackHoverMarkdownForToken } from "./hoverFallback";

let client: LanguageClient | undefined;

function getCliCommand(): string {
  const config = vscode.workspace.getConfiguration("aivi");
  const cmd = config.get<string>("cli.command");
  return cmd && cmd.trim().length > 0 ? cmd : "aivi";
}

function docHasTests(doc: vscode.TextDocument): boolean {
  return /(^|\n)\s*@test\b/.test(doc.getText());
}

type DiscoveredTest = {
  moduleName: string;
  defName: string;
  fullName: string;
  description: string;
  decoratorLine: number; // 0-based
};

function discoverTestsFromText(text: string): { moduleName: string; tests: DiscoveredTest[] } {
  const moduleMatch = /^\s*module\s+([A-Za-z0-9_.]+)\b/m.exec(text);
  const moduleName = moduleMatch?.[1] ?? "<unknown>";

  const tests: DiscoveredTest[] = [];
  const lines = text.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    // Match @test "description" or bare @test (legacy)
    const testMatch = /^\s*@test\s*(?:"([^"]*)")?\s*$/.exec(lines[i]);
    if (!testMatch) {
      continue;
    }
    const description = testMatch[1] ?? "";

    // Find the next binding name: `foo = ...` (skip blank lines).
    let j = i + 1;
    while (j < lines.length && /^\s*$/.test(lines[j])) {
      j++;
    }
    const m = /^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=/.exec(lines[j] ?? "");
    if (!m) {
      continue;
    }
    const defName = m[1];
    const fullName = `${moduleName}.${defName}`;
    tests.push({ moduleName, defName, fullName, description: description || defName, decoratorLine: i });
  }

  return { moduleName, tests };
}

function toCliPath(uri: vscode.Uri): string {
  const fsPath = uri.fsPath;
  return process.platform === "win32" ? fsPath.replace(/\\/g, "/") : fsPath;
}

function inferWorkspaceFolderForTarget(target: string): vscode.WorkspaceFolder | undefined {
  const wsFolders = vscode.workspace.workspaceFolders;
  if (!wsFolders || wsFolders.length === 0) {
    return undefined;
  }

  // Prefer a workspace folder that contains the resolved target path.
  const base = target.replace(/(\/\.\.\.|\/\*\*)$/, "");
  if (path.isAbsolute(base)) {
    const uri = vscode.Uri.file(base);
    return vscode.workspace.getWorkspaceFolder(uri) ?? wsFolders[0];
  }

  return wsFolders[0];
}

async function runAiviTest(target: string, ws?: vscode.WorkspaceFolder): Promise<void> {
  const cli = getCliCommand();
  const workspaceFolder = ws ?? inferWorkspaceFolderForTarget(target);
  const task = new vscode.Task(
    { type: "aivi", task: "test" },
    workspaceFolder ?? vscode.TaskScope.Workspace,
    "AIVI: test",
    "aivi",
    new vscode.ShellExecution(cli, ["test", target], { cwd: workspaceFolder?.uri.fsPath })
  );
  task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, clear: true };
  await vscode.tasks.executeTask(task);
}

async function runAiviTestProcess(
  args: string[],
  cwd: string | undefined,
  token: vscode.CancellationToken | undefined,
  onStdout: (chunk: string) => void,
  onStderr: (chunk: string) => void
): Promise<number> {
  const cli = getCliCommand();
  return await new Promise<number>((resolve) => {
    const child = spawn(cli, args, { cwd, stdio: ["ignore", "pipe", "pipe"] });

    const cancel = token?.onCancellationRequested(() => {
      try {
        child.kill();
      } catch {
        // ignore
      }
    });

    child.stdout.on("data", (buf) => onStdout(buf.toString("utf8")));
    child.stderr.on("data", (buf) => onStderr(buf.toString("utf8")));
    child.on("close", (code) => {
      cancel?.dispose();
      resolve(typeof code === "number" ? code : 1);
    });
  });
}

function parseFailureNamesFromStderr(stderr: string): Set<string> {
  const out = new Set<string>();
  for (const line of stderr.split(/\r?\n/)) {
    // CLI prints failures as: `<qualified.name>: <message>`
    const m = /^([A-Za-z0-9_.]+):\s+/.exec(line);
    if (m) {
      out.add(m[1]);
    }
  }
  return out;
}

export function activate(context: vscode.ExtensionContext) {
  const outputChannel = vscode.window.createOutputChannel("AIVI Language Server");
  const testOutput = vscode.window.createOutputChannel("AIVI Tests");
  context.subscriptions.push(testOutput);

  // VS Code Testing API integration (gutter run arrows + Testing view tree).
  const testController = vscode.tests.createTestController("aiviTests", "AIVI Tests");
  context.subscriptions.push(testController);

  const itemMeta = new Map<string, { kind: "folder" | "file" | "test"; uri?: vscode.Uri; fullName?: string }>();

  const upsertWorkspaceTests = async (): Promise<void> => {
    testController.items.replace([]);
    const wsFolders = vscode.workspace.workspaceFolders ?? [];
    for (const ws of wsFolders) {
      const root = testController.createTestItem(`aiviWs:${ws.uri.toString()}`, ws.name, ws.uri);
      itemMeta.set(root.id, { kind: "folder", uri: ws.uri });
      testController.items.add(root);

      const integrationFolderUri = vscode.Uri.joinPath(ws.uri, "integration-tests");
      try {
        const stat = await vscode.workspace.fs.stat(integrationFolderUri);
        if (stat.type !== vscode.FileType.Directory) {
          continue;
        }
      } catch {
        continue;
      }

      const integration = testController.createTestItem(
        `aiviFolder:${integrationFolderUri.toString()}`,
        "integration-tests",
        integrationFolderUri
      );
      integration.canResolveChildren = true;
      itemMeta.set(integration.id, { kind: "folder", uri: integrationFolderUri });
      root.children.add(integration);

      // Eager discovery so editor gutter icons work immediately.
      await resolveFolderChildren(integration);
    }
  };

  const resolveFolderChildren = async (folderItem: vscode.TestItem): Promise<void> => {
    const meta = itemMeta.get(folderItem.id);
    if (!meta?.uri) {
      return;
    }
    folderItem.children.replace([]);
    const ws = vscode.workspace.getWorkspaceFolder(meta.uri) ?? vscode.workspace.workspaceFolders?.[0];
    if (!ws) {
      return;
    }

    const rel = path.relative(ws.uri.fsPath, meta.uri.fsPath).replace(/\\/g, "/");
    const pattern = new vscode.RelativePattern(ws, `${rel}/**/*.aivi`);
    const files = await vscode.workspace.findFiles(pattern, "**/target/**");

    for (const uri of files) {
      const bytes = await vscode.workspace.fs.readFile(uri);
      const text = Buffer.from(bytes).toString("utf8");
      if (!/(^|\n)\s*@test\b/.test(text)) {
        continue;
      }
      const discovered = discoverTestsFromText(text);
      if (discovered.tests.length === 0) {
        continue;
      }

      const label = path.posix.basename(toCliPath(uri));
      const fileItem = testController.createTestItem(`aiviFile:${uri.toString()}`, label, uri);
      fileItem.canResolveChildren = false;
      fileItem.description = discovered.moduleName;
      itemMeta.set(fileItem.id, { kind: "file", uri });
      folderItem.children.add(fileItem);

      for (const t of discovered.tests) {
        const testId = `aiviTest:${uri.toString()}::${t.fullName}`;
        const testItem = testController.createTestItem(testId, t.description, uri);
        testItem.range = new vscode.Range(
          new vscode.Position(t.decoratorLine, 0),
          new vscode.Position(t.decoratorLine, linesAt(text, t.decoratorLine).length)
        );
        testItem.description = t.moduleName;
        itemMeta.set(testItem.id, { kind: "test", uri, fullName: t.fullName });
        fileItem.children.add(testItem);
      }
    }
  };

  function linesAt(text: string, line: number): string {
    const lines = text.split(/\r?\n/);
    return lines[line] ?? "";
  }

  const collectLeafTests = (item: vscode.TestItem, out: vscode.TestItem[]): void => {
    const meta = itemMeta.get(item.id);
    if (meta?.kind === "test") {
      out.push(item);
      return;
    }
    item.children.forEach((child) => collectLeafTests(child, out));
  };

  const runHandler = async (
    request: vscode.TestRunRequest,
    token: vscode.CancellationToken
  ): Promise<void> => {
    const run = testController.createTestRun(request);
    const includedRoots = request.include ?? (() => {
      const all: vscode.TestItem[] = [];
      testController.items.forEach((i) => all.push(i));
      return all;
    })();

    const leafTests: vscode.TestItem[] = [];
    for (const root of includedRoots) {
      collectLeafTests(root, leafTests);
    }

    const toRun = leafTests.filter((t) => !(request.exclude?.includes(t) ?? false));
    const byFile = new Map<string, { uri: vscode.Uri; tests: { item: vscode.TestItem; fullName: string }[] }>();
    for (const t of toRun) {
      const meta = itemMeta.get(t.id);
      if (!meta?.uri || !meta.fullName) {
        continue;
      }
      const key = meta.uri.toString();
      const entry = byFile.get(key) ?? { uri: meta.uri, tests: [] };
      entry.tests.push({ item: t, fullName: meta.fullName });
      byFile.set(key, entry);
    }

    for (const { uri, tests } of byFile.values()) {
      if (token.isCancellationRequested) {
        break;
      }

      for (const { item } of tests) {
        run.enqueued(item);
        run.started(item);
      }

      const ws = vscode.workspace.getWorkspaceFolder(uri) ?? vscode.workspace.workspaceFolders?.[0];
      const cwd = ws?.uri.fsPath;
      const args = ["test", ...tests.flatMap((t) => ["--only", t.fullName]), toCliPath(uri)];

      let stderr = "";
      run.appendOutput(`[RUN] ${toCliPath(uri)}\n`);
      const exitCode = await runAiviTestProcess(
        args,
        cwd,
        token,
        (s) => run.appendOutput(s),
        (s) => {
          stderr += s;
          run.appendOutput(s);
        }
      );

      const failures = parseFailureNamesFromStderr(stderr);
      for (const { item, fullName } of tests) {
        if (exitCode === 0) {
          run.passed(item);
        } else if (failures.has(fullName)) {
          run.failed(item, new vscode.TestMessage("failed"));
        } else {
          // If the file failed but this test wasn't listed, mark as failed conservatively.
          run.failed(item, new vscode.TestMessage("failed"));
        }
      }

      run.appendOutput(exitCode === 0 ? `[OK ] ${toCliPath(uri)}\n` : `[FAIL] ${toCliPath(uri)}\n`);
    }

    run.end();
  };

  testController.resolveHandler = async (item) => {
    if (!item) {
      await upsertWorkspaceTests();
      return;
    }
    const meta = itemMeta.get(item.id);
    if (meta?.kind === "folder") {
      await resolveFolderChildren(item);
    }
  };

  testController.createRunProfile(
    "Run",
    vscode.TestRunProfileKind.Run,
    (request, token) => void runHandler(request, token),
    true
  );

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
      provideHover: async (document, position, token, next) => {
        const hover = await Promise.resolve(next(document, position, token));
        if (hover || document.languageId !== "aivi") {
          return hover;
        }
        const text = document.getText();
        const offset = document.offsetAt(position);
        const hoverToken = extractHoverToken(text, offset);
        if (!hoverToken) {
          return hover;
        }
        const fallback = fallbackHoverMarkdownForToken(hoverToken);
        if (!fallback) {
          return hover;
        }
        const markdown = new vscode.MarkdownString(`\`${hoverToken}\`\n\n${fallback}`);
        return new vscode.Hover(markdown);
      },
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
        await runAiviTest(toCliPath(doc.uri), vscode.workspace.getWorkspaceFolder(doc.uri));
      } else {
        const ws = vscode.workspace.workspaceFolders?.[0];
        await runAiviTest("integration-tests/**", ws);
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("aivi.runTestsFile", async () => {
      const doc = vscode.window.activeTextEditor?.document;
      if (!doc || doc.languageId !== "aivi") {
        return;
      }
      await runAiviTest(toCliPath(doc.uri), vscode.workspace.getWorkspaceFolder(doc.uri));
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("aivi.runTestsFolder", async (uri?: vscode.Uri) => {
      if (!uri) {
        return;
      }
      const ws = vscode.workspace.getWorkspaceFolder(uri) ?? vscode.workspace.workspaceFolders?.[0];
      await runAiviTest(`${toCliPath(uri)}/...`, ws);
    })
  );

  context.subscriptions.push(
    vscode.tasks.onDidEndTaskProcess((e) => {
      if (e.execution.task.source !== "aivi" || e.execution.task.name !== "AIVI: test") {
        return;
      }
      const scope = e.execution.task.scope;
      const ws =
        scope && typeof scope === "object" && "uri" in scope
          ? (scope as vscode.WorkspaceFolder)
          : vscode.workspace.workspaceFolders?.[0];
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
