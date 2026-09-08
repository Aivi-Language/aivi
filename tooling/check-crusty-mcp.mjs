import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { readFile, realpath } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

const root = await realpath(fileURLToPath(new URL('..', import.meta.url)));
const config = JSON.parse(await readFile(new URL('../.mcp.json', import.meta.url), 'utf8'));
const server = config.mcpServers.Crusty;
assert.ok(server?.command, '.mcp.json must register Crusty');

const child = spawn(server.command, server.args ?? [], {
  cwd: root,
  env: { ...process.env, ...server.env },
  stdio: ['pipe', 'pipe', 'inherit'],
});
const lines = createInterface({ input: child.stdout });
const pending = new Map();
let nextId = 0;
let failure;
let toolCount;
let exit;

function fail(error) {
  failure ??= error;
  for (const request of pending.values()) request.reject(error);
  pending.clear();
}

child.on('error', fail);
child.stdin.on('error', fail);
const closed = new Promise((resolve) => child.once('close', (code, signal) => {
  if (pending.size) fail(new Error(`Crusty exited (code=${code}, signal=${signal})`));
  resolve({ code, signal });
}));
lines.on('line', (line) => {
  try {
    const response = JSON.parse(line);
    assert.equal(response.jsonrpc, '2.0');
    if (response.id === undefined) return;
    const request = pending.get(response.id);
    assert.ok(request, `Unexpected response ID ${response.id}`);
    pending.delete(response.id);
    if (response.error) request.reject(new Error(JSON.stringify(response.error)));
    else request.resolve(response.result);
  } catch (error) {
    fail(error);
  }
});

async function request(method, params = {}) {
  if (failure) throw failure;
  const id = ++nextId;
  let timer;
  try {
    return await new Promise((resolve, reject) => {
      timer = setTimeout(() => fail(new Error(`Crusty timed out: ${method}`)), 30_000);
      pending.set(id, { resolve, reject });
      child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
    });
  } finally {
    clearTimeout(timer);
    pending.delete(id);
  }
}

try {
  const initialized = await request('initialize', {
    protocolVersion: '2024-11-05',
    capabilities: {},
    clientInfo: { name: 'aivi-crusty-smoke', version: '1.0.0' },
  });
  assert.equal(initialized.protocolVersion, '2024-11-05');
  assert.ok(initialized.capabilities.tools, 'Crusty must support tools');
  child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' })}\n`);

  const { tools, nextCursor } = await request('tools/list');
  assert.equal(nextCursor, undefined, 'Update this check if Crusty starts paginating tools');
  const names = new Set(tools.map((tool) => tool.name));
  for (const name of [
    'repo.consult', 'repo.search', 'index.status', 'index.refresh',
    'change.prepare', 'change.validate', 'change.get', 'task.get', 'task.list',
    'audit.start', 'audit.get', 'validation.queue', 'validation.record', 'work.update',
  ]) assert.ok(names.has(name), `Missing Crusty tool: ${name}`);

  const consultation = await request('tools/call', {
    name: 'repo.consult',
    arguments: { topic: 'Verify the AIVI Crusty MCP connection without changing source.', budget: 800 },
  });
  assert.ok(!consultation.isError, 'Crusty consultation failed');
  const payload = consultation.structuredContent
    ?? JSON.parse(consultation.content.find((item) => item.type === 'text').text);
  assert.equal(payload.result.consulted, true);
  assert.equal(await realpath(payload.result.snapshot.repository), root);
  toolCount = names.size;
} finally {
  child.stdin.end();
  const terminate = setTimeout(() => child.kill('SIGTERM'), 2_000);
  const kill = setTimeout(() => child.kill('SIGKILL'), 5_000);
  exit = await closed;
  clearTimeout(terminate);
  clearTimeout(kill);
  lines.close();
}
if (failure) throw failure;
assert.equal(exit.code, 0, `Crusty did not stop cleanly (signal=${exit.signal})`);
console.log(`Crusty MCP verified: ${root} (${toolCount} tools)`);
