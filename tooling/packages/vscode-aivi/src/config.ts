import * as vscode from "vscode";
import { type AiviConfig, readConfig } from "./contract";

export type { AiviConfig } from "./contract";

export function getConfig(scope?: vscode.ConfigurationScope): AiviConfig {
  return readConfig(vscode.workspace.getConfiguration("aivi", scope));
}
