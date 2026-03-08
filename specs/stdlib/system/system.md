# System Module

<!-- quick-info: {"kind":"module","name":"aivi.system"} -->
The `aivi.system` module is the bridge between an AIVI program and the operating system that launched it.

Use it to read environment variables, inspect command-line arguments, detect a best-effort locale tag, or terminate the current process with an exit code.

<!-- /quick-info -->
<div class="import-badge">use aivi.system</div>

## Overview

The most common `aivi.system` workflow is "read host-provided configuration, apply a fallback, then continue with normal program logic."

```aivi
use aivi.system (env)

main = do Effect {
  portOpt <- env.get "PORT"
  port =
    portOpt match
      | Some p => p
      | None   => "8080"

  print "Server will listen on {port}"
}
```

## Capabilities

- `env.get` and `env.decode` require `process.env.read`.
- `env.set` and `env.remove` require `process.env.write`.
- `args` and `localeTag` require `process.args`.
- `exit` requires `process.exit`.

## Environment helpers

Environment variables are a common way to pass configuration into a program without editing source code.
They are especially useful for secrets, deployment-specific endpoints, and feature flags.

| Function | What it does |
| --- | --- |
| **env.get** name<br><code>Text -> Effect Text (Option Text)</code> | Reads the environment variable `name`. Returns `None` when the variable is unset. |
| **env.decode** prefix<br><code>Text -> Effect Text A</code> | Reads variables that share the given `prefix` and decodes them into the expected typed configuration `A`. |
| **env.set** name value<br><code>Text -> Text -> Effect Text Unit</code> | Sets the environment variable `name` to `value`. |
| **env.remove** name<br><code>Text -> Effect Text Unit</code> | Removes the environment variable `name`. |

`env.decode` is driven by the expected result type around it. For example, if you expect `{ port: Int, debug: Bool }`, then `env.decode "AIVI_APP"` reads variables such as `AIVI_APP_PORT` and `AIVI_APP_DEBUG` and decodes them into that record.

For the source-level model behind these helpers, see [Environment sources](../../syntax/external_sources/environment.md).

## Process helpers

| Function | What it does |
| --- | --- |
| **args**<br><code>Effect Text (List Text)</code> | Returns the command-line arguments in the order the user supplied them, excluding the executable name. |
| **localeTag**<br><code>Effect Text (Option Text)</code> | Returns a best-effort host locale tag such as `"en-GB"`, or `None` when no locale information is available. |
| **exit** code<br><code>Int -> Effect Text Unit</code> | Terminates the process with `code`. `0` means success; non-zero codes signal failure. |

On Unix-like hosts, `localeTag` checks `LC_ALL`, `LC_MESSAGES`, and `LANG` in that order, then strips charset and modifier suffixes such as `.UTF-8` or `@euro`.

## Practical guidance

- Use `env.get` when one variable is optional and you want to choose the fallback yourself.
- Use `env.decode` when several related variables form one typed configuration record.
- Use `args` for command-line tools, batch jobs, and wrappers that receive user-provided flags.
- Use `localeTag` when you want a best-effort starting point for localization; see [I18n](../core/i18n.md) for locale parsing and fallback patterns.
- Use `exit` at the program boundary when another program, shell script, or CI job needs a clear success or failure signal.

## Type Signatures

```aivi
env : {
  get: Text -> Effect Text (Option Text)
  decode: Text -> Effect Text A
  set: Text -> Text -> Effect Text Unit
  remove: Text -> Effect Text Unit
}

args : Effect Text (List Text)
localeTag : Effect Text (Option Text)
exit : Int -> Effect Text Unit
```
