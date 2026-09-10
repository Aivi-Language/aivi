import assert from "node:assert/strict";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { once } from "node:events";
import { resolve } from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";
import { pathToFileURL } from "node:url";

interface JsonRpcResponse {
  id: number;
  result?: unknown;
  error?: { code: number; message: string };
}

class LspConnection {
  private buffer = Buffer.alloc(0);
  private nextId = 1;
  private readonly pending = new Map<
    number,
    { resolve: (value: unknown) => void; reject: (error: Error) => void }
  >();
  private readonly received: unknown[] = [];

  constructor(private readonly child: ChildProcessWithoutNullStreams) {
    child.stdout.on("data", (chunk: Buffer) => {
      this.buffer = Buffer.concat([this.buffer, chunk]);
      this.consume();
    });
    child.once("exit", (code, signal) => {
      const error = new Error(
        `language server exited before responding (code=${code}, signal=${signal})`
      );
      for (const request of this.pending.values()) request.reject(error);
      this.pending.clear();
    });
  }

  request(method: string, params?: unknown): Promise<unknown> {
    const id = this.nextId++;
    const response = new Promise<unknown>((resolveResponse, rejectResponse) => {
      this.pending.set(id, { resolve: resolveResponse, reject: rejectResponse });
    });
    this.send({
      jsonrpc: "2.0",
      id,
      method,
      ...(params === undefined ? {} : { params }),
    });
    return response;
  }

  notify(method: string, params?: unknown): void {
    this.send({
      jsonrpc: "2.0",
      method,
      ...(params === undefined ? {} : { params }),
    });
  }

  transcript(): string {
    return JSON.stringify(this.received);
  }

  private send(message: unknown): void {
    const payload = Buffer.from(JSON.stringify(message), "utf8");
    this.child.stdin.write(`Content-Length: ${payload.length}\r\n\r\n`);
    this.child.stdin.write(payload);
  }

  private consume(): void {
    while (true) {
      const headerEnd = this.buffer.indexOf("\r\n\r\n");
      if (headerEnd < 0) return;
      const header = this.buffer.subarray(0, headerEnd).toString("ascii");
      const length = /^Content-Length:\s*(\d+)$/im.exec(header)?.[1];
      assert.ok(length, `missing Content-Length in ${JSON.stringify(header)}`);
      const contentLength = Number(length);
      const bodyStart = headerEnd + 4;
      const bodyEnd = bodyStart + contentLength;
      if (this.buffer.length < bodyEnd) return;

      const message = JSON.parse(
        this.buffer.subarray(bodyStart, bodyEnd).toString("utf8")
      ) as JsonRpcResponse;
      this.received.push(message);
      this.buffer = this.buffer.subarray(bodyEnd);
      if (typeof message.id !== "number") continue;
      const request = this.pending.get(message.id);
      if (!request) continue;
      this.pending.delete(message.id);
      if (message.error) {
        request.reject(
          new Error(`LSP ${message.error.code}: ${message.error.message}`)
        );
      } else {
        request.resolve(message.result);
      }
    }
  }
}

async function timeout<T>(operation: Promise<T>, milliseconds: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      operation,
      new Promise<never>((_, reject) => {
        timer = setTimeout(
          () => reject(new Error(`operation timed out after ${milliseconds} ms`)),
          milliseconds
        );
      }),
    ]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}

test("raw stdio transport completes a real AIVI LSP lifecycle", async () => {
  const packageRoot = resolve(__dirname, "../..");
  const repositoryRoot = resolve(packageRoot, "../../..");
  const executable = resolve(repositoryRoot, "target/debug/aivi");
  assert.ok(
    existsSync(executable),
    `missing ${executable}; run the test:server script first`
  );

  const env = { ...process.env };
  delete env.WAYLAND_DISPLAY;
  delete env.WAYLAND_SOCKET;
  delete env.DISPLAY;
  delete env.GDK_BACKEND;
  const child = spawn(executable, ["lsp"], {
    cwd: repositoryRoot,
    env,
    stdio: ["pipe", "pipe", "pipe"],
  });
  const stderr: Buffer[] = [];
  const fixtureDirectory = mkdtempSync(resolve(tmpdir(), "vscode-aivi-lsp-"));
  const fixturePath = resolve(fixtureDirectory, "code-lens.aivi");
  writeFileSync(
    fixturePath,
    [
      "@test",
      "value selected : Task Text Bool = pure True",
      "@test",
      "func helper = value => value",
      "",
    ].join("\n")
  );
  const fixtureUri = pathToFileURL(fixturePath).href;
  child.stderr.on("data", (chunk: Buffer) => stderr.push(chunk));
  const connection = new LspConnection(child);

  try {
    const initialized = await timeout(connection.request("initialize", {
      processId: process.pid,
      clientInfo: { name: "vscode-aivi-integration", version: "0.1.0" },
      rootUri: pathToFileURL(repositoryRoot).href,
      workspaceFolders: [{
        uri: pathToFileURL(repositoryRoot).href,
        name: "aivi",
      }],
      capabilities: {
        workspace: { workspaceFolders: true },
        textDocument: {
          synchronization: { didSave: true },
          semanticTokens: {
            requests: { range: true, full: { delta: true } },
            tokenTypes: [],
            tokenModifiers: [],
            formats: ["relative"],
          },
        },
      },
      initializationOptions: {
        diagnosticsDebounceMs: 25,
        inlayHintsEnabled: true,
        inlayHintsMaxLength: 40,
        codeLensEnabled: true,
      },
    }), 10_000) as { capabilities: Record<string, unknown> };

    assert.equal(
      (initialized.capabilities.textDocumentSync as { change: number }).change,
      2
    );
    assert.ok(initialized.capabilities.signatureHelpProvider);
    assert.equal(initialized.capabilities.documentHighlightProvider, true);
    assert.equal(initialized.capabilities.foldingRangeProvider, true);
    assert.ok(initialized.capabilities.semanticTokensProvider);

    connection.notify("initialized", {});
    connection.notify("textDocument/didOpen", {
      textDocument: {
        uri: fixtureUri,
        languageId: "aivi",
        version: 1,
        text: readFileSync(fixturePath, "utf8"),
      },
    });
    const codeLenses = await timeout(connection.request("textDocument/codeLens", {
      textDocument: { uri: fixtureUri },
    }), 5_000) as Array<{
      command?: { command: string; arguments?: unknown[] };
    }>;
    assert.equal(codeLenses.length, 1, "only executable @test values get code lenses");
    assert.equal(codeLenses[0]?.command?.command, "aivi.runTest");
    assert.deepEqual(codeLenses[0]?.command?.arguments, [fixtureUri, "selected"]);

    try {
      await timeout(connection.request("shutdown"), 5_000);
    } catch (error) {
      throw new Error(
        `${String(error)}\ntranscript: ${connection.transcript()}\nstderr:\n${
          Buffer.concat(stderr).toString("utf8")
        }`
      );
    }
    const exited = once(child, "exit");
    connection.notify("exit");
    child.stdin.end();
    const [code, signal] = await timeout(exited, 5_000);
    assert.equal(signal, null);
    assert.equal(
      code,
      0,
      `server stderr:\n${Buffer.concat(stderr).toString("utf8")}`
    );
  } finally {
    if (child.exitCode === null && child.signalCode === null) child.kill();
    rmSync(fixtureDirectory, { recursive: true, force: true });
  }
});
