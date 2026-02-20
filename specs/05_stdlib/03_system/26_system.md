# System Module

<!-- quick-info: {"kind":"module","name":"aivi.system"} -->
The `System` module connects your program to the operating system.

It allows you to read **Environment Variables** (like secret queries or API keys), handle command-line arguments, or signal success/failure with exit codes. It is the bridge between the managed AIVI runtime and the chaotic host machine.

<!-- /quick-info -->
<div class="import-badge">use aivi.system</div>

## Overview

<<< ../../snippets/from_md/05_stdlib/03_system/25_system/block_01.aivi{aivi}

## Core API (v0.1)

### Environment

| Function | Explanation |
| --- | --- |
| **env.get** name<br><pre><code>`Text -> Effect Text (Option Text)`</code></pre> | Reads the environment variable `name`. Returns `None` when the variable is not set. |
| **env.set** name value<br><pre><code>`Text -> Text -> Effect Text Unit`</code></pre> | Sets the environment variable `name` to `value`. |
| **env.remove** name<br><pre><code>`Text -> Effect Text Unit`</code></pre> | Removes the environment variable `name`. |

### Process

| Function | Explanation |
| --- | --- |
| **args**<br><pre><code>`Effect Text (List Text)`</code></pre> | Returns the command-line arguments passed to the program. |
| **localeTag**<br><pre><code>`Effect Text (Option Text)`</code></pre> | Returns the locale tag of the host system (e.g. `"en-GB"`), or `None` when unavailable. |
| **exit** code<br><pre><code>`Int -> Effect Text Unit`</code></pre> | Terminates the process with `code`. A code of `0` signals success; any other value signals failure. |

## Type Signatures

<<< ../../snippets/from_md/05_stdlib/03_system/25_system/block_02.aivi{aivi}
