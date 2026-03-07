# System Module

<!-- quick-info: {"kind":"module","name":"aivi.system"} -->
The `System` module is the bridge between an AIVI program and the operating system that launched it.

Use it to read environment variables, inspect command-line arguments, detect locale information, or terminate the process with an exit code.

<!-- /quick-info -->
<div class="import-badge">use aivi.system</div>

## Overview

<<< ../../snippets/from_md/stdlib/system/system/overview.aivi{aivi}

## Capabilities

- `env.get` and `env.decode` require permission to read process environment variables.
- `env.set` and `env.remove` require permission to write process environment variables.
- `args` and `localeTag` require access to process metadata.
- `exit` requires permission to terminate the process.

## Environment helpers

Environment variables are a common way to pass configuration into a program without editing source code.
They are especially useful for secrets, deployment-specific endpoints, and feature flags.

| Function | What it does |
| --- | --- |
| **env.get** name<br><code>Text -> Effect Text (Option Text)</code> | Reads the environment variable `name`. Returns `None` when it is not set. |
| **env.decode** prefix<br><code>Text -> Effect Text A</code> | Reads all environment variables under `prefix` and decodes them into the expected typed configuration `A`. |
| **env.set** name value<br><code>Text -> Text -> Effect Text Unit</code> | Sets the environment variable `name` to `value`. |
| **env.remove** name<br><code>Text -> Effect Text Unit</code> | Removes the environment variable `name`. |

## Process helpers

| Function | What it does |
| --- | --- |
| **args**<br><code>Effect Text (List Text)</code> | Returns the command-line arguments passed to the program. |
| **localeTag**<br><code>Effect Text (Option Text)</code> | Returns the host locale tag such as `"en-GB"`, or `None` when it is unavailable. |
| **exit** code<br><code>Int -> Effect Text Unit</code> | Terminates the process with `code`. `0` means success; non-zero codes signal failure. |

## Practical guidance

- Use `env.get` when one variable is optional.
- Use `env.decode` when several related variables form a typed config record.
- Use `args` for command-line tools and batch jobs.
- Use `exit` when another program, shell script, or CI job needs a clear success or failure signal.

## Type Signatures

<<< ../../snippets/from_md/stdlib/system/system/type_signatures.aivi{aivi}
