# Duration Domain

The `Duration` domain provides a type-safe way to represent **Spans of Time**.

In many systems, a timeout is just an integer like `500`. But is that 500 milliseconds? 500 seconds? Ambiguous units cause outages (like setting a 30-second timeout that the system reads as 30 milliseconds).

`Duration` solves this by wrapping the number in a type that knows its unit. `500` becomes `500ms` or `0.5s`. The compiler ensures you don't compare Seconds to Apples, stopping bugs before they start.

## Overview

<<< ../../snippets/from_md/05_stdlib/02_chronos/03_duration/block_01.aivi{aivi}

## Features

<<< ../../snippets/from_md/05_stdlib/02_chronos/03_duration/block_02.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/02_chronos/03_duration/block_03.aivi{aivi}

## Usage Examples

<<< ../../snippets/from_md/05_stdlib/02_chronos/03_duration/block_04.aivi{aivi}
