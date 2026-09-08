import type { AiviConfig } from "./contract";

export interface CompilerInvocation {
  command: string;
  args: string[];
  cwd: string;
}

function invocation(
  config: AiviConfig,
  cwd: string,
  subcommand: string,
  args: string[]
): CompilerInvocation {
  return {
    command: config.compilerPath,
    args: [subcommand, ...args],
    cwd,
  };
}

export function checkInvocation(
  config: AiviConfig,
  cwd: string,
  filePath: string
): CompilerInvocation {
  return invocation(config, cwd, "check", [filePath]);
}

export function testInvocation(
  config: AiviConfig,
  cwd: string,
  filePath: string,
  testName: string
): CompilerInvocation {
  return invocation(config, cwd, "test", [filePath, testName]);
}
