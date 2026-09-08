import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

import {
  COMMAND_IDS,
  CONFIG_DEFAULTS,
  SETTING_IDS,
  SETTING_SECTIONS,
  readConfig,
  serverInitializationOptions,
  type ConfigurationReader,
} from "../src/contract";
import { checkInvocation, testInvocation } from "../src/invocations";

const packageRoot = resolve(__dirname, "../..");

interface ManifestSetting {
  default: unknown;
}

interface Manifest {
  contributes: {
    commands: Array<{ command: string; title: string }>;
    configuration: { properties: Record<string, ManifestSetting> };
  };
}

function manifest(): Manifest {
  return JSON.parse(
    readFileSync(resolve(packageRoot, "package.json"), "utf8")
  ) as Manifest;
}

class Reader implements ConfigurationReader {
  constructor(private readonly values: Record<string, unknown>) {}

  get<T>(section: string, defaultValue: T): T {
    return (this.values[section] ?? defaultValue) as T;
  }
}

test("manifest commands and settings match the checked contract", () => {
  const packageManifest = manifest();
  assert.deepEqual(
    packageManifest.contributes.commands.map(({ command }) => command).sort(),
    Object.values(COMMAND_IDS).sort()
  );

  const properties = packageManifest.contributes.configuration.properties;
  assert.deepEqual(Object.keys(properties).sort(), Object.values(SETTING_IDS).sort());
  for (const [name, settingId] of Object.entries(SETTING_IDS)) {
    assert.deepEqual(
      properties[settingId]?.default,
      CONFIG_DEFAULTS[name as keyof typeof CONFIG_DEFAULTS],
      `${settingId} default drifted from CONFIG_DEFAULTS`
    );
  }
});

test("README documents every contributed command and setting", () => {
  const packageManifest = manifest();
  const readme = readFileSync(resolve(packageRoot, "README.md"), "utf8");
  for (const { title } of packageManifest.contributes.commands) {
    assert.match(readme, new RegExp(title.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }
  for (const settingId of Object.values(SETTING_IDS)) {
    assert.ok(readme.includes(`\`${settingId}\``), `${settingId} is undocumented`);
  }
});

test("configuration is bounded and maps exactly to server initialization", () => {
  const config = readConfig(new Reader({
    [SETTING_SECTIONS.compilerPath]: "  /opt/aivi bin/aivi  ",
    [SETTING_SECTIONS.compilerTimeout]: 1,
    [SETTING_SECTIONS.diagnosticsDebounceMs]: 99_000,
    [SETTING_SECTIONS.inlayHintsEnabled]: false,
    [SETTING_SECTIONS.inlayHintsMaxLength]: 1,
    [SETTING_SECTIONS.codeLensEnabled]: false,
    [SETTING_SECTIONS.formatOnSave]: true,
  }));

  assert.equal(config.compilerPath, "/opt/aivi bin/aivi");
  assert.equal(config.compilerTimeout, 1_000);
  assert.equal(config.diagnosticsDebounceMs, 5_000);
  assert.equal(config.inlayHintsMaxLength, 4);
  assert.deepEqual(serverInitializationOptions(config), {
    diagnosticsDebounceMs: 5_000,
    inlayHintsEnabled: false,
    inlayHintsMaxLength: 4,
    codeLensEnabled: false,
  });
});

test("compiler invocations preserve paths and names as literal arguments", () => {
  const config = readConfig(new Reader({
    [SETTING_SECTIONS.compilerPath]: "/opt/AIVI Compiler/aivi",
  }));
  assert.deepEqual(
    checkInvocation(config, "/workspace one", "/workspace one/$HOME;demo.aivi"),
    {
      command: "/opt/AIVI Compiler/aivi",
      args: ["check", "/workspace one/$HOME;demo.aivi"],
      cwd: "/workspace one",
    }
  );
  assert.deepEqual(
    testInvocation(
      config,
      "/workspace two",
      "/workspace two/spec.aivi",
      "name with $(shell)"
    ).args,
    ["test", "/workspace two/spec.aivi", "name with $(shell)"]
  );
});
