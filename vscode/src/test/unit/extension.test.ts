import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock the 'vscode' module using our __mocks__/vscode.ts
vi.mock('vscode');

import * as vscode from 'vscode';
import { extractHoverToken, fallbackHoverMarkdownForToken } from '../../hoverFallback';
// Import the activate function from your extension. 
// Note: You might need to export `activate` from `src/extension.ts` if not already exported.
// Assuming `src/extension.ts` is the entry point.
// Since we can't easily import the actual extension.ts if it has side effects on import, 
// we will just test the mock setup for this proof of concept.
// In a real scenario, you'd refactor extension.ts to be testable or import specific functions.

describe('Extension Test Suite', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('should verify mocked vscode API functionality', () => {
        vscode.window.showInformationMessage('Hello World');
        expect(vscode.window.showInformationMessage).toHaveBeenCalledWith('Hello World');
    });

    it('should verify commands registration mock', () => {
        const disposable = { dispose: vi.fn() };
        (vscode.commands.registerCommand as any).mockReturnValue(disposable);

        const cmdId = 'aivi.testCommand';
        const callback = vi.fn();

        vscode.commands.registerCommand(cmdId, callback);
        expect(vscode.commands.registerCommand).toHaveBeenCalledWith(cmdId, callback);
    });

    it('extracts operator hover tokens', () => {
        const text = 'x = a |> b';
        const offset = text.indexOf('|>') + 1;
        expect(extractHoverToken(text, offset)).toBe('|>');
    });

    it('extracts decorator hover tokens', () => {
        const text = '@test "works"\nfoo = 1';
        const offset = text.indexOf('@test') + 2;
        expect(extractHoverToken(text, offset)).toBe('@test');
    });

    it('has fallback docs for core syntax tokens', () => {
        expect(fallbackHoverMarkdownForToken('module')).toBeTruthy();
        expect(fallbackHoverMarkdownForToken('=>')).toBeTruthy();
        expect(fallbackHoverMarkdownForToken('<|')).toBeTruthy();
    });
});
