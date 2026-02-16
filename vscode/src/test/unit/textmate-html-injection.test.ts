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
  // Keep snapshots stable and readable:
  // - preserve order
  // - drop duplicate adjacent scopes
  // - drop extremely-generic parent scopes that add noise
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

function tokenAt(tokens: Array<{ start: number; end: number; text: string; scopes: string[] }>, index: number) {
  const tok = tokens.find((t) => t.start <= index && index < t.end);
  if (!tok) {
    throw new Error(`No token covering index ${index}`);
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
    'text.html.aivi.injection': path.join(vscodeDir, 'syntaxes', 'aivi.html.injection.tmLanguage.json'),
    'text.html.basic': path.join(vscodeDir, 'src', 'test', 'fixtures', 'textmate', 'text.html.basic.tmLanguage.json'),
  };

  const registry = new Registry({
    onigLib,
    loadGrammar: async (scopeName: string): Promise<IRawGrammar> => {
      const p = grammarPaths[scopeName];
      if (!p) {
        // The HTML reference grammar may optionally include other scopes (CSS/JS/etc).
        // For these tests we only care about HTML + embedded AIVI, so a no-op stub is enough.
        return { scopeName, patterns: [], repository: {} } as IRawGrammar;
      }
      const raw = await fs.readFile(p, 'utf8');
      return JSON.parse(raw) as IRawGrammar;
    },
    // Mirror `package.json#contributes.grammars[].injectTo` for the test harness.
    getInjections: (scopeName: string): string[] => {
      if (scopeName === 'source.aivi') return ['text.html.aivi.injection'];
      return [];
    },
  });

  const loaded = await registry.loadGrammar('source.aivi');
  if (!loaded) throw new Error('Failed to load source.aivi grammar');
  grammar = loaded;
});

describe('TextMate: AIVI + injected HTML (~html sigil)', () => {
  it('treats type signature lines as a single scope (no colored braces)', () => {
    const line = 'deepName : { data: { user: { profile: { name: String } } } } -> String';
    const tok = tokenizeText(grammar, line);
    expect(tok).toMatchSnapshot();

    const tokens = tokensForLine(tok, 0, line);
    const braceIndex = line.indexOf('{');
    const brace = tokenAt(tokens, braceIndex);
    expect(brace.text).toContain('{');
    expect(brace.scopes.join(' ')).toContain('meta.type.signature.aivi');
    expect(brace.scopes.join(' ')).not.toContain('punctuation.section.bracket.aivi');
  });

  it('tokenizes a simple HTML tag with attribute and string value', () => {
    const line = 'x = ~<html> <div class="a">hello</div> </html>';
    const tok = tokenizeText(grammar, line);
    expect(tok).toMatchSnapshot();

    const tokens = tokensForLine(tok, 0, line);
    const div = findToken(tokens, (t) => t.text === 'div' && t.scopes.join(' ').includes('entity.name.tag.html'));
    expect(div.scopes.join(' ')).toContain('entity.name.tag.html');

    const classAttr = findFirstTokenIncluding(tokens, 'class');
    expect(classAttr.scopes.join(' ')).toContain('entity.other.attribute-name.html');

    const strA = findToken(tokens, (t) => t.text === 'a' && t.scopes.join(' ').includes('string.quoted.double.html'));
    expect(strA.scopes.join(' ')).toContain('string.quoted.double.html');
  });

  it('tokenizes nested tags and returns to AIVI scopes outside the sigil', () => {
    const line = 'use aivi.ui (TextNode)  y = ~<html> <section><span id="x">t</span></section> </html>  z = 1';
    const tok = tokenizeText(grammar, line);
    expect(tok).toMatchSnapshot();

    const tokens = tokensForLine(tok, 0, line);
    const useKw = findFirstTokenIncluding(tokens, 'use');
    expect(useKw.scopes.join(' ')).toContain('keyword.other.aivi');

    const section = findToken(tokens, (t) => t.text === 'section' && t.scopes.join(' ').includes('entity.name.tag.html'));
    expect(section.scopes.join(' ')).toContain('entity.name.tag.html');

    const after = findFirstTokenIncluding(tokens, 'z');
    expect(after.scopes.join(' ')).not.toContain('.html');
    expect(after.scopes.join(' ')).not.toContain('string.quoted.other.sigil.html.aivi');
  });

  it('treats event handler attributes as HTML (no JS parsing required)', () => {
    const line = 'doAiviFn = pure Unit  btn = ~<html> <button onClick={doAiviFn}>ok</button> </html>';
    const tok = tokenizeText(grammar, line);
    expect(tok).toMatchSnapshot();

    const tokens = tokensForLine(tok, 0, line);
    const onClick = findFirstTokenIncluding(tokens, 'onClick');
    expect(onClick.scopes.join(' ')).toContain('.html');
    expect(onClick.scopes.join(' ')).toContain('attribute');

    const attrValue = findFirstTokenIncluding(tokens, '{doAiviFn}');
    expect(attrValue.scopes.join(' ')).toContain('string.unquoted.html');
    expect(attrValue.scopes.join(' ')).not.toContain('source.js');
  });

  it('handles self-closing tags and `<` that is not a tag', () => {
    const line = 'img = ~<html> <img alt="x" /> <div>2 < 3</div> </html>';
    const tok = tokenizeText(grammar, line);
    expect(tok).toMatchSnapshot();

    const tokens = tokensForLine(tok, 0, line);
    const img = findToken(tokens, (t) => t.text === 'img' && t.scopes.join(' ').includes('entity.name.tag.html'));
    expect(img.scopes.join(' ')).toContain('entity.name.tag.html');

    // The `<` in `2 < 3` should *not* be parsed as a tag start.
    const textLtIndex = line.indexOf('2 < 3') + 2; // points at `<`
    const lt = tokenAt(tokens, textLtIndex);
    expect(lt.text).toContain('<');
    expect(lt.scopes.join(' ')).not.toContain('entity.name.tag.html');
  });
});
