// @vitest-environment node
import { beforeAll, describe, expect, it } from 'vitest';
import * as fs from 'node:fs/promises';
import path from 'node:path';
import { Registry, type IGrammar, type IOnigLib, type IRawGrammar } from 'vscode-textmate';
import { loadWASM, OnigScanner, OnigString } from 'vscode-oniguruma';

type SimplifiedToken = {
  start: number;
  end: number;
  scopes: string[];
};

type SimplifiedLine = {
  line: number;
  tokens: SimplifiedToken[];
};

function bufferToArrayBuffer(buf: Buffer): ArrayBuffer {
  return buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength);
}

function simplifyScopes(scopes: string[]): string[] {
  const out: string[] = [];
  for (const scope of scopes) {
    if (out[out.length - 1] === scope) continue;
    if (scope === 'source.aivi') continue;
    out.push(scope);
  }
  return out;
}

function simplifyLineTokens(tokens: { startIndex: number; endIndex: number; scopes: string[] }[]): SimplifiedToken[] {
  return tokens.map((tok) => ({
    start: tok.startIndex,
    end: tok.endIndex,
    scopes: simplifyScopes(tok.scopes),
  }));
}

function tokenizeText(grammar: IGrammar, text: string): SimplifiedLine[] {
  const lines = text.split(/\r?\n/);
  let ruleStack: any = null;
  const out: SimplifiedLine[] = [];
  for (let i = 0; i < lines.length; i++) {
    const res = grammar.tokenizeLine(lines[i], ruleStack);
    ruleStack = res.ruleStack;
    out.push({ line: i, tokens: simplifyLineTokens(res.tokens) });
  }
  return out;
}

function tokensForLine(lines: SimplifiedLine[], line: number, lineText: string) {
  const entry = lines.find((value) => value.line === line);
  if (!entry) throw new Error(`Missing tokenized line ${line}`);
  return entry.tokens.map((token) => ({ ...token, text: lineText.slice(token.start, token.end) }));
}

function findToken(tokens: Array<{ text: string; scopes: string[] }>, predicate: (token: { text: string; scopes: string[] }) => boolean) {
  const token = tokens.find(predicate);
  if (!token) {
    const sample = tokens.map((value) => `${JSON.stringify(value.text)} :: ${value.scopes.join(' ')}`).join('\n');
    throw new Error(`Did not find matching token.\nTokens:\n${sample}`);
  }
  return token;
}

let grammar: IGrammar;

beforeAll(async () => {
  const wasmPath = require.resolve('vscode-oniguruma/release/onig.wasm');
  const wasmBin = await fs.readFile(wasmPath);
  await loadWASM(bufferToArrayBuffer(wasmBin));

  const onigLib: Promise<IOnigLib> = Promise.resolve({
    createOnigScanner(patterns: string[]) {
      return new OnigScanner(patterns);
    },
    createOnigString(value: string) {
      return new OnigString(value);
    },
  });

  const vscodeDir = path.resolve(__dirname, '../../..');
  const grammarPaths: Record<string, string> = {
    'source.aivi': path.join(vscodeDir, 'syntaxes', 'aivi.tmLanguage.json'),
    'source.css': path.join(vscodeDir, 'src', 'test', 'fixtures', 'textmate', 'source.css.basic.tmLanguage.json'),
  };

  const registry = new Registry({
    onigLib,
    loadGrammar: async (scopeName: string): Promise<IRawGrammar> => {
      const grammarPath = grammarPaths[scopeName];
      if (!grammarPath) return { scopeName, patterns: [], repository: {} } as IRawGrammar;
      const raw = await fs.readFile(grammarPath, 'utf8');
      return JSON.parse(raw) as IRawGrammar;
    },
  });

  const loaded = await registry.loadGrammar('source.aivi');
  if (!loaded) throw new Error('Failed to load source.aivi grammar');
  grammar = loaded;
});

describe('TextMate: raw text sigils', () => {
  it('colors plain backtick sigils like regular strings', () => {
    const line = 'value = ~`hello`';
    const tokenized = tokenizeText(grammar, line);
    const tokens = tokensForLine(tokenized, 0, line);
    const body = findToken(tokens, (token) => token.text === 'hello');

    expect(body.scopes.join(' ')).toContain('string.quoted.other.backtick.aivi');
    expect(body.scopes.join(' ')).not.toContain('storage.type.sigil.aivi');
  });

  it('embeds CSS inside tagged backtick sigils', () => {
    const text = ['style = ~`css', '  .myClass { color: red; }', '`'].join('\n');
    const tokenized = tokenizeText(grammar, text);
    const cssLine = '  .myClass { color: red; }';
    const tokens = tokensForLine(tokenized, 1, cssLine);

    const selector = findToken(tokens, (token) => token.text === '.myClass');
    expect(selector.scopes.join(' ')).toContain('entity.other.attribute-name.class.css');

    const property = findToken(tokens, (token) => token.text === 'color');
    expect(property.scopes.join(' ')).toContain('support.type.property-name.css');
  });
});
