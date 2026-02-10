"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
const fs = __importStar(require("node:fs"));
const node_1 = require("vscode-languageclient/node");
const LONG_OPS = [
    "::=",
    "<|",
    "|>",
    "=>",
    "<-",
    "->",
    ":=",
    "==",
    "!=",
    ">=",
    "<=",
    "&&",
    "||",
    "..",
];
const UNICODE_OPS = ["→", "⇒", "▷", "⇐", "←", "≠", "≥", "≤", "∧", "∨", "…"];
const PUNCT = new Set(["{", "}", "(", ")", "[", "]", ",", ";", ":", ".", "|", "@"]);
function isIdentifierStart(ch) {
    return /[A-Za-z_]/.test(ch);
}
function isIdentifierPart(ch) {
    return /[A-Za-z0-9_]/.test(ch);
}
function tokenizeAivi(text) {
    const tokens = [];
    let i = 0;
    const startsWithAny = (ops) => {
        for (const op of ops) {
            if (text.startsWith(op, i))
                return op;
        }
        return null;
    };
    while (i < text.length) {
        const ch = text[i];
        if (ch === "\r") {
            if (text[i + 1] === "\n")
                i += 2;
            else
                i++;
            tokens.push({ type: "newline", value: "\n" });
            continue;
        }
        if (ch === "\n") {
            i++;
            tokens.push({ type: "newline", value: "\n" });
            continue;
        }
        if (ch === " " || ch === "\t" || ch === "\r" || ch === "\n") {
            i++;
            continue;
        }
        if (text.startsWith("//", i)) {
            let j = i + 2;
            while (j < text.length && text[j] !== "\n")
                j++;
            tokens.push({ type: "line_comment", value: text.slice(i, j) });
            i = j;
            continue;
        }
        if (text.startsWith("/*", i)) {
            let j = i + 2;
            while (j < text.length && !text.startsWith("*/", j))
                j++;
            j = Math.min(text.length, j + 2);
            tokens.push({ type: "block_comment", value: text.slice(i, j) });
            i = j;
            continue;
        }
        if (ch === '"' || ch === "'" || ch === "`") {
            const quote = ch;
            let j = i + 1;
            while (j < text.length) {
                const c = text[j];
                if (quote !== "`" && c === "\\") {
                    j += 2;
                    continue;
                }
                if (c === quote) {
                    j++;
                    break;
                }
                j++;
            }
            tokens.push({ type: "string", value: text.slice(i, j) });
            i = j;
            continue;
        }
        if (/[0-9]/.test(ch)) {
            let j = i + 1;
            while (j < text.length && /[0-9]/.test(text[j]))
                j++;
            if (text[j] === "." && /[0-9]/.test(text[j + 1] ?? "")) {
                j++;
                while (j < text.length && /[0-9]/.test(text[j]))
                    j++;
            }
            tokens.push({ type: "number", value: text.slice(i, j) });
            i = j;
            continue;
        }
        if (isIdentifierStart(ch)) {
            let j = i + 1;
            while (j < text.length && isIdentifierPart(text[j]))
                j++;
            tokens.push({ type: "ident", value: text.slice(i, j) });
            i = j;
            continue;
        }
        const longOp = startsWithAny(LONG_OPS) ?? startsWithAny(UNICODE_OPS);
        if (longOp) {
            tokens.push({ type: "op", value: longOp });
            i += longOp.length;
            continue;
        }
        tokens.push({ type: PUNCT.has(ch) ? "punct" : "op", value: ch });
        i++;
    }
    return tokens;
}
function formatAivi(text, indentSize, maxBlankLines, baseIndent = "") {
    const tokens = tokenizeAivi(text);
    const lines = [];
    let current = "";
    let indentLevel = 0;
    let armIndentActive = false;
    let armIndentLevel = 0;
    let blankLines = 0;
    const indentUnit = " ".repeat(indentSize);
    const currentIndent = () => baseIndent + indentUnit.repeat(Math.max(0, indentLevel));
    const currentArmIndent = () => baseIndent + indentUnit.repeat(Math.max(0, armIndentLevel));
    const flushLine = () => {
        const trimmed = current.replace(/[ \t]+$/g, "");
        if (trimmed.length === 0) {
            blankLines++;
            if (blankLines <= maxBlankLines)
                lines.push("");
        }
        else {
            blankLines = 0;
            lines.push(trimmed);
        }
        current = "";
    };
    const ensureIndent = (indentOverride) => {
        if (current.length !== 0)
            return;
        current = indentOverride === "arm" ? currentArmIndent() : currentIndent();
    };
    const lastChar = () => (current.length ? current[current.length - 1] : "");
    const ensureSpace = () => {
        ensureIndent();
        if (current.length === 0)
            return;
        if (!/\s/.test(lastChar()))
            current += " ";
    };
    const write = (s) => {
        ensureIndent();
        current += s;
    };
    const writeInline = (s) => {
        current += s;
    };
    const trimSpaceBefore = () => {
        current = current.replace(/[ \t]+$/g, "");
    };
    const isPrefixContext = (prev) => {
        if (!prev)
            return true;
        if (prev.type === "op")
            return true;
        if (prev.type === "punct" && ["(", "[", "{", ",", ":", ";", "?", "|"].includes(prev.value))
            return true;
        return false;
    };
    let prev;
    let prevSignificant;
    const peekNextSignificant = (startIdx) => {
        for (let j = startIdx; j < tokens.length; j++) {
            const t = tokens[j];
            if (t.type === "newline")
                continue;
            return t;
        }
        return undefined;
    };
    for (let idx = 0; idx < tokens.length; idx++) {
        const token = tokens[idx];
        const next = tokens[idx + 1];
        if (token.type === "newline") {
            const nextSig = peekNextSignificant(idx + 1);
            if (!nextSig) {
                prev = token;
                continue;
            }
            if (prevSignificant?.type === "op" &&
                (prevSignificant.value === "=" || prevSignificant.value === "?") &&
                nextSig?.type === "punct" &&
                nextSig.value === "|") {
                armIndentActive = true;
                armIndentLevel = indentLevel + 1;
            }
            else if (armIndentActive && !(nextSig?.type === "punct" && nextSig.value === "|")) {
                armIndentActive = false;
                armIndentLevel = 0;
            }
            flushLine();
            prev = token;
            continue;
        }
        if (token.type === "line_comment") {
            if (current.trim().length)
                ensureSpace();
            else
                ensureIndent();
            writeInline(token.value);
            flushLine();
            prev = token;
            prevSignificant = token;
            continue;
        }
        if (token.type === "block_comment") {
            const commentLines = token.value.split(/\r?\n/);
            if (commentLines.length === 1) {
                if (current.trim().length)
                    ensureSpace();
                else
                    ensureIndent();
                writeInline(token.value);
            }
            else {
                if (current.trim().length)
                    flushLine();
                for (let ci = 0; ci < commentLines.length; ci++) {
                    current = currentIndent();
                    writeInline(commentLines[ci] ?? "");
                    flushLine();
                }
            }
            prev = token;
            prevSignificant = token;
            continue;
        }
        if (token.type === "punct") {
            const v = token.value;
            if (v === "@") {
                if (current.trim().length)
                    flushLine();
                ensureIndent();
                writeInline("@");
                prev = token;
                prevSignificant = token;
                continue;
            }
            if (v === "{") {
                if (current.trim().length && !/\s/.test(lastChar()) && !"([.{".includes(lastChar()))
                    ensureSpace();
                writeInline("{");
                if (next?.type === "punct" && next.value === "}") {
                    prev = token;
                    prevSignificant = token;
                    continue;
                }
                indentLevel++;
                armIndentLevel = armIndentActive ? indentLevel + 1 : 0;
                flushLine();
                prev = token;
                prevSignificant = token;
                continue;
            }
            if (v === "}") {
                indentLevel = Math.max(0, indentLevel - 1);
                armIndentLevel = armIndentActive ? indentLevel + 1 : 0;
                if (current.trim().length)
                    flushLine();
                current = currentIndent();
                writeInline("}");
                if (next && !(next.type === "punct" && next.value === ";"))
                    flushLine();
                prev = token;
                prevSignificant = token;
                continue;
            }
            if (v === "(" || v === "[") {
                if (current.trim().length && /[A-Za-z0-9_'"`]/.test(lastChar()))
                    ensureSpace();
                writeInline(v);
                prev = token;
                prevSignificant = token;
                continue;
            }
            if (v === ")" || v === "]") {
                trimSpaceBefore();
                writeInline(v);
                prev = token;
                prevSignificant = token;
                continue;
            }
            if (v === ",") {
                ensureIndent();
                trimSpaceBefore();
                writeInline(",");
                if (!(next?.type === "punct" && (next.value === "}" || next.value === "]" || next.value === ")"))) {
                    writeInline(" ");
                }
                prev = token;
                prevSignificant = token;
                continue;
            }
            if (v === ";") {
                ensureIndent();
                trimSpaceBefore();
                writeInline(";");
                flushLine();
                prev = token;
                prevSignificant = token;
                continue;
            }
            if (v === ".") {
                ensureIndent();
                trimSpaceBefore();
                writeInline(".");
                prev = token;
                prevSignificant = token;
                continue;
            }
            if (v === ":") {
                ensureIndent();
                trimSpaceBefore();
                writeInline(":");
                if (!(next?.type === "punct" && (next.value === "," || next.value === "}" || next.value === "]")))
                    writeInline(" ");
                prev = token;
                prevSignificant = token;
                continue;
            }
            if (v === "|") {
                if (current.trim().length)
                    flushLine();
                ensureIndent(armIndentActive ? "arm" : undefined);
                writeInline("| ");
                prev = token;
                prevSignificant = token;
                continue;
            }
        }
        if (token.type === "op") {
            const v = token.value;
            if (v === "?") {
                ensureSpace();
                writeInline("?");
                if (next && next.type !== "newline")
                    writeInline(" ");
                prev = token;
                prevSignificant = token;
                continue;
            }
            if ((v === "!" || v === "-") && isPrefixContext(prev)) {
                trimSpaceBefore();
                writeInline(v);
                prev = token;
                prevSignificant = token;
                continue;
            }
            ensureSpace();
            writeInline(v);
            writeInline(" ");
            prev = token;
            prevSignificant = token;
            continue;
        }
        if (token.type === "ident" || token.type === "number" || token.type === "string") {
            if (current.trim().length) {
                const lc = lastChar();
                if (!["", " ", "\t", "\n", "(", "[", "{", ".", "@", "|"].includes(lc))
                    ensureSpace();
            }
            else {
                ensureIndent();
            }
            writeInline(token.value);
            prev = token;
            prevSignificant = token;
            continue;
        }
        prev = token;
    }
    if (current.length)
        flushLine();
    return lines.join("\n").replace(/\s+$/g, "") + "\n";
}
let client;
function getFormatConfig() {
    const config = vscode.workspace.getConfiguration("aivi");
    return {
        indentSize: config.get("format.indentSize", 2),
        maxBlankLines: config.get("format.maxBlankLines", 1),
    };
}
function activate(context) {
    const isWindows = process.platform === "win32";
    const serverExe = isWindows ? "aivi-lsp.exe" : "aivi-lsp";
    const bundledServerPath = context.asAbsolutePath(`bin/${serverExe}`);
    const serverOptions = {
        command: fs.existsSync(bundledServerPath) ? bundledServerPath : "aivi-lsp",
        args: [],
    };
    const clientOptions = {
        documentSelector: [{ language: "aivi" }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher("**/*.aivi"),
        },
    };
    client = new node_1.LanguageClient("aivi", "Aivi Language Server", serverOptions, clientOptions);
    client.start();
    context.subscriptions.push(new vscode.Disposable(() => {
        void client?.stop();
    }));
    const provider = {
        provideDocumentFormattingEdits(document, options) {
            const { indentSize, maxBlankLines } = getFormatConfig();
            const size = Number.isFinite(indentSize) && indentSize > 0 ? indentSize : options.tabSize;
            const formatted = formatAivi(document.getText(), size, maxBlankLines);
            const fullRange = new vscode.Range(document.positionAt(0), document.positionAt(document.getText().length));
            return [vscode.TextEdit.replace(fullRange, formatted)];
        },
    };
    const rangeProvider = {
        provideDocumentRangeFormattingEdits(document, range, options) {
            const { indentSize, maxBlankLines } = getFormatConfig();
            const size = Number.isFinite(indentSize) && indentSize > 0 ? indentSize : options.tabSize;
            const text = document.getText(range);
            const lineStart = document.lineAt(range.start.line).text;
            const baseIndent = (lineStart.match(/^\s*/)?.[0] ?? "").replace(/\t/g, " ".repeat(size));
            const formatted = formatAivi(text, size, maxBlankLines, baseIndent).replace(/\n$/g, "");
            return [vscode.TextEdit.replace(range, formatted)];
        },
    };
    context.subscriptions.push(vscode.languages.registerDocumentFormattingEditProvider({ language: "aivi" }, provider), vscode.languages.registerDocumentRangeFormattingEditProvider({ language: "aivi" }, rangeProvider));
}
function deactivate() {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
