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
  const entry = lines.find((l) => l.line === line);
  if (!entry) throw new Error(`Missing tokenized line ${line}`);
  return entry.tokens.map((t) => ({ ...t, text: lineText.slice(t.start, t.end) }));
}

function findFirstTokenIncluding(tokens: Array<{ text: string; scopes: string[] }>, needle: string) {
  const tok = tokens.find((t) => t.text.includes(needle));
  if (!tok) {
    const sample = tokens.map((t) => `${JSON.stringify(t.text)} :: ${t.scopes.join(' ')}`).join('\n');
    throw new Error(`Did not find token containing ${JSON.stringify(needle)}.\nTokens:\n${sample}`);
  }
  return tok;
}

function findToken(tokens: Array<{ text: string; scopes: string[] }>, predicate: (t: { text: string; scopes: string[] }) => boolean) {
  const tok = tokens.find(predicate);
  if (!tok) {
    const sample = tokens.map((t) => `${JSON.stringify(t.text)} :: ${t.scopes.join(' ')}`).join('\n');
    throw new Error(`Did not find matching token.\nTokens:\n${sample}`);
  }
  return tok;
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
    createOnigString(s: string) {
      return new OnigString(s);
    },
  });

  const vscodeDir = path.resolve(__dirname, '../../..');
  const grammarPaths: Record<string, string> = {
    'source.aivi': path.join(vscodeDir, 'syntaxes', 'aivi.tmLanguage.json'),
    'text.xml.aivi.gtk.injection': path.join(vscodeDir, 'syntaxes', 'aivi.gtk.injection.tmLanguage.json'),
    'text.xml': path.join(vscodeDir, 'src', 'test', 'fixtures', 'textmate', 'text.xml.basic.tmLanguage.json'),
  };

  const registry = new Registry({
    onigLib,
    loadGrammar: async (scopeName: string): Promise<IRawGrammar> => {
      const p = grammarPaths[scopeName];
      if (!p) return { scopeName, patterns: [], repository: {} } as IRawGrammar;
      const raw = await fs.readFile(p, 'utf8');
      return JSON.parse(raw) as IRawGrammar;
    },
    getInjections: (scopeName: string): string[] => {
      if (scopeName === 'source.aivi') return ['text.xml.aivi.gtk.injection'];
      return [];
    },
  });

  const loaded = await registry.loadGrammar('source.aivi');
  if (!loaded) throw new Error('Failed to load source.aivi grammar');
  grammar = loaded;
});

describe('TextMate: AIVI + injected XML (~gtk sigil)', () => {
  it('tokenizes a simple GTK/XML tag with attribute and string value', () => {
    const line = 'x = ~<gtk> <object class="GtkBox">hello</object> </gtk>';
    const tok = tokenizeText(grammar, line);

    const tokens = tokensForLine(tok, 0, line);
    const objectTag = findToken(tokens, (t) => t.text === 'object' && t.scopes.join(' ').includes('entity.name.tag.xml'));
    expect(objectTag.scopes.join(' ')).toContain('entity.name.tag.xml');

    const classAttr = findFirstTokenIncluding(tokens, 'class');
    expect(classAttr.scopes.join(' ')).toContain('entity.other.attribute-name.xml');

    const strGtkBox = findToken(tokens, (t) => t.text === 'GtkBox' && t.scopes.join(' ').includes('string.quoted.double.xml'));
    expect(strGtkBox.scopes.join(' ')).toContain('string.quoted.double.xml');
  });

  it('embeds AIVI inside `{...}` splices within the GTK/XML region', () => {
    const line = 'btn = ~<gtk> <object onClick={Msg.Save} /> </gtk>';
    const tok = tokenizeText(grammar, line);

    const tokens = tokensForLine(tok, 0, line);

    const open = findToken(tokens, (t) => t.text === '{' && t.scopes.join(' ').includes('punctuation.section.embedded.begin.aivi'));
    expect(open.scopes.join(' ')).toContain('punctuation.section.embedded.begin.aivi');

    const msg = findFirstTokenIncluding(tokens, 'Msg');
    expect(msg.scopes.join(' ')).toContain('entity.name.type.aivi');
  });
});
