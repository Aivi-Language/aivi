# TimeZone and ZonedDateTime

> **Status: Implemented**   available in the stdlib and runtime.

<!-- quick-info: {"kind":"module","name":"aivi.chronos.timezone"} -->
The `TimeZone` and `ZonedDateTime` domains handle geographic time offsets, daylight saving transitions, and global time coordination.

**Implementation note (v0.1):** time zone rules come from the IANA database (via `chrono-tz`); offsets include DST and ambiguous/invalid local times are runtime errors. `ZonedDateTime` literals use millisecond precision.

<!-- /quick-info -->
<div class="import-badge">use aivi.chronos.timezone<span class="domain-badge">domain</span></div>

## Overview

<<< ../../snippets/from_md/05_stdlib/02_chronos/04_timezone/block_01.aivi{aivi}

## Features

<<< ../../snippets/from_md/05_stdlib/02_chronos/04_timezone/block_02.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/02_chronos/04_timezone/block_03.aivi{aivi}

## Usage Examples

<<< ../../snippets/from_md/05_stdlib/02_chronos/04_timezone/block_04.aivi{aivi}
