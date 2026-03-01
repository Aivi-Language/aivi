# System Module

<!-- quick-info: {"kind":"module","name":"aivi.system"} -->
The `System` module connects your program to the operating system.

It allows you to read **Environment Variables** (like secret queries or API keys), handle command-line arguments, or signal success/failure with exit codes. It is the bridge between the managed AIVI runtime and the chaotic host machine.

<!-- /quick-info -->
<div class="import-badge">use aivi.system</div>

## Overview

<<< ../../snippets/from_md/stdlib/system/system/overview.aivi{aivi}

## Core API (v0.1)

### Environment

| Function | Explanation |
| --- | --- |
| **env.get** name<br><code>Text -> Effect Text (Option Text)</code> | Reads the environment variable `name`. Returns `None` when the variable is not set. |
| **env.decode** prefix<br><code>Text -> Effect Text A</code> | Reads all environment keys under `prefix` and decodes them into the expected typed config `A`. |
| **env.set** name value<br><code>Text -> Text -> Effect Text Unit</code> | Sets the environment variable `name` to `value`. |
| **env.remove** name<br><code>Text -> Effect Text Unit</code> | Removes the environment variable `name`. |

### Process

| Function | Explanation |
| --- | --- |
| **args**<br><code>Effect Text (List Text)</code> | Returns the command-line arguments passed to the program. |
| **localeTag**<br><code>Effect Text (Option Text)</code> | Returns the locale tag of the host system (e.g. `"en-GB"`), or `None` when unavailable. |
| **exit** code<br><code>Int -> Effect Text Unit</code> | Terminates the process with `code`. A code of `0` signals success; any other value signals failure. |

## Type Signatures

<<< ../../snippets/from_md/stdlib/system/system/type_signatures.aivi{aivi}
