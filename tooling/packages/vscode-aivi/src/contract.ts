export const COMMAND_IDS = {
  restartServer: "aivi.restartServer",
  showOutputChannel: "aivi.showOutputChannel",
  formatDocument: "aivi.formatDocument",
  checkFile: "aivi.checkFile",
  runTest: "aivi.runTest",
} as const;

export const SETTING_SECTIONS = {
  compilerPath: "compiler.path",
  compilerTimeout: "compiler.timeout",
  diagnosticsDebounceMs: "diagnostics.debounceMs",
  inlayHintsEnabled: "inlayHints.enabled",
  inlayHintsMaxLength: "inlayHints.maxLength",
  codeLensEnabled: "codeLens.enabled",
  formatOnSave: "format.onSave",
} as const;

export const SETTING_IDS = Object.fromEntries(
  Object.entries(SETTING_SECTIONS).map(([name, section]) => [name, `aivi.${section}`])
) as { [Key in keyof typeof SETTING_SECTIONS]: `aivi.${(typeof SETTING_SECTIONS)[Key]}` };

export const CONFIG_DEFAULTS = {
  compilerPath: "aivi",
  compilerTimeout: 15_000,
  diagnosticsDebounceMs: 200,
  inlayHintsEnabled: true,
  inlayHintsMaxLength: 30,
  codeLensEnabled: true,
  formatOnSave: false,
};

export interface AiviConfig {
  compilerPath: string;
  compilerTimeout: number;
  diagnosticsDebounceMs: number;
  inlayHintsEnabled: boolean;
  inlayHintsMaxLength: number;
  codeLensEnabled: boolean;
  formatOnSave: boolean;
}

export interface ConfigurationReader {
  get<T>(section: string, defaultValue: T): T;
}

export interface ServerInitializationOptions {
  diagnosticsDebounceMs: number;
  inlayHintsEnabled: boolean;
  inlayHintsMaxLength: number;
  codeLensEnabled: boolean;
}

export const SERVER_RESTART_SETTING_IDS = [
  SETTING_IDS.compilerPath,
  SETTING_IDS.compilerTimeout,
  SETTING_IDS.diagnosticsDebounceMs,
  SETTING_IDS.inlayHintsEnabled,
  SETTING_IDS.inlayHintsMaxLength,
  SETTING_IDS.codeLensEnabled,
] as const;

function boundedInteger(
  value: number,
  fallback: number,
  minimum: number,
  maximum: number
): number {
  if (!Number.isFinite(value)) return fallback;
  return Math.min(maximum, Math.max(minimum, Math.trunc(value)));
}

export function readConfig(reader: ConfigurationReader): AiviConfig {
  const compilerPath = reader
    .get(SETTING_SECTIONS.compilerPath, CONFIG_DEFAULTS.compilerPath)
    .trim();
  return {
    compilerPath: compilerPath || CONFIG_DEFAULTS.compilerPath,
    compilerTimeout: boundedInteger(
      reader.get(SETTING_SECTIONS.compilerTimeout, CONFIG_DEFAULTS.compilerTimeout),
      CONFIG_DEFAULTS.compilerTimeout,
      1_000,
      120_000
    ),
    diagnosticsDebounceMs: boundedInteger(
      reader.get(
        SETTING_SECTIONS.diagnosticsDebounceMs,
        CONFIG_DEFAULTS.diagnosticsDebounceMs
      ),
      CONFIG_DEFAULTS.diagnosticsDebounceMs,
      0,
      5_000
    ),
    inlayHintsEnabled: reader.get(
      SETTING_SECTIONS.inlayHintsEnabled,
      CONFIG_DEFAULTS.inlayHintsEnabled
    ),
    inlayHintsMaxLength: boundedInteger(
      reader.get(
        SETTING_SECTIONS.inlayHintsMaxLength,
        CONFIG_DEFAULTS.inlayHintsMaxLength
      ),
      CONFIG_DEFAULTS.inlayHintsMaxLength,
      4,
      200
    ),
    codeLensEnabled: reader.get(
      SETTING_SECTIONS.codeLensEnabled,
      CONFIG_DEFAULTS.codeLensEnabled
    ),
    formatOnSave: reader.get(
      SETTING_SECTIONS.formatOnSave,
      CONFIG_DEFAULTS.formatOnSave
    ),
  };
}

export function serverInitializationOptions(
  config: AiviConfig
): ServerInitializationOptions {
  return {
    diagnosticsDebounceMs: config.diagnosticsDebounceMs,
    inlayHintsEnabled: config.inlayHintsEnabled,
    inlayHintsMaxLength: config.inlayHintsMaxLength,
    codeLensEnabled: config.codeLensEnabled,
  };
}
