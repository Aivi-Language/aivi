const HOVER_FALLBACK_DOCS: Record<string, string> = {
  module: "Defines the current module namespace.",
  use: "Imports names or modules into scope.",
  export: "Exports names from the current module.",
  effect: "Starts an effect block where `<-` binds effectful values.",
  resource: "Defines a scoped resource block with structured cleanup semantics.",
  domain: "Declares a domain for operator/literal rewrites.",
  class: "Declares a type class.",
  instance: "Declares a type class instance implementation.",
  "->": "Function arrow type constructor (`A -> B`).",
  "<-": "Bind operator inside `effect { ... }` blocks.",
  "=>": "Pattern-match branch arrow.",
  "|>": "Forward pipe operator.",
  "<|": "Record patch / reverse pipe operator depending on context.",
  "?": "Pattern matching operator.",
  "@test": "Marks a definition as a test case.",
};

function isIdentChar(ch: string): boolean {
  return /[A-Za-z0-9_.]/.test(ch);
}

function isSpace(ch: string): boolean {
  return ch === " " || ch === "\t" || ch === "\n" || ch === "\r";
}

function isSymbolChar(ch: string): boolean {
  return !isSpace(ch) && !isIdentChar(ch);
}

export function extractHoverToken(text: string, offset: number): string | undefined {
  if (text.length === 0) {
    return undefined;
  }
  const at = Math.max(0, Math.min(offset, text.length));
  const chAt = at < text.length ? text[at] : undefined;
  const chBefore = at > 0 ? text[at - 1] : undefined;

  if (chAt === "@" || chBefore === "@") {
    const atPos = chAt === "@" ? at : at - 1;
    let end = atPos + 1;
    while (end < text.length && /[A-Za-z0-9_]/.test(text[end])) {
      end += 1;
    }
    const token = text.slice(atPos, end).trim();
    return token.length > 0 ? token : undefined;
  }

  const onSymbol = (chAt !== undefined && isSymbolChar(chAt)) || (chBefore !== undefined && isSymbolChar(chBefore));
  if (onSymbol) {
    let start = at;
    while (start > 0 && isSymbolChar(text[start - 1])) {
      start -= 1;
    }
    let end = at;
    while (end < text.length && isSymbolChar(text[end])) {
      end += 1;
    }
    const token = text.slice(start, end).trim();
    return token.length > 0 ? token : undefined;
  }

  let start = at;
  while (start > 0 && isIdentChar(text[start - 1])) {
    start -= 1;
  }
  let end = at;
  while (end < text.length && isIdentChar(text[end])) {
    end += 1;
  }
  const token = text.slice(start, end).trim();
  if (token.length === 0) {
    return undefined;
  }
  if (start > 0 && text[start - 1] === "@") {
    return `@${token}`;
  }
  return token;
}

export function fallbackHoverMarkdownForToken(token: string): string | undefined {
  return HOVER_FALLBACK_DOCS[token] ?? HOVER_FALLBACK_DOCS[token.toLowerCase()];
}
