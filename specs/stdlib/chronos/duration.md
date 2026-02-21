# Duration Domain

<!-- quick-info: {"kind":"module","name":"aivi.chronos.duration"} -->
The `Duration` domain provides a type-safe way to represent **Spans of Time**.

In many systems, a timeout is just an integer like `500`. But is that 500 milliseconds? 500 seconds? Ambiguous units cause outages (like setting a 30-second timeout that the system reads as 30 milliseconds).

`Duration` solves this by wrapping the number in a type that knows its unit. `500` becomes `500ms` or `0.5s`. The compiler ensures you don't compare Seconds to Apples, stopping bugs before they start.

<!-- /quick-info -->
<div class="import-badge">use aivi.chronos.duration<span class="domain-badge">domain</span></div>

## Overview

<<< ../../snippets/from_md/stdlib/chronos/duration/overview.aivi{aivi}

## Features

<<< ../../snippets/from_md/stdlib/chronos/duration/features.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/chronos/duration/domain_definition.aivi{aivi}

## Usage Examples

<<< ../../snippets/from_md/stdlib/chronos/duration/usage_examples.aivi{aivi}
