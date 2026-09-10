# Standard Library

> **This is the reference sidecar.** Reach for it when you need to know what a specific module
> exports today. If you are still learning AIVI, start with [Tutorials](/tutorials/) or
> [How-to Guides](/how-to/) first, then come back here when you know the job or module you need.

The AIVI standard library is the set of modules that ships with the language. Some names, such
as `Option`, `Result`, `Signal`, and `Task`, are built in. Most other tools live in named modules
that you import with `use`.

Use this page as a map:

- start with the modules in **Start here** if you are new to AIVI
- use the grouped lists below when you already know the job you need to do
- treat each linked page as the source of truth for what the current stdlib actually exposes

Several modules are intentionally small today, and some are shared type vocabularies rather than
full runtime clients. Their pages call that out up front so you do not have to guess.

## How to read module pages

| Module page shape | What the top tables show | Typical examples |
| --- | --- | --- |
| Pure helper module | The full exported function battery for the module | `aivi.option`, `aivi.result`, `aivi.list`, `aivi.math`, `aivi.bool` |
| Vocabulary / capability module | The exported types, handles, and canonical members | `aivi.fs`, `aivi.http`, `aivi.db`, `aivi.env` |
| Mixed module | Separate tables for core operations and higher-level helpers | `aivi.text`, `aivi.prelude`, `aivi.matrix` |

If you open a module page and only want the quick answer to *"what does this include?"*, jump to
the **At a glance** section near the top.

For external systems, prefer provider capabilities defined through `@source`. The related stdlib
modules now mainly carry shared data vocabulary and handle-marker types such as `FsSource`,
`HttpSource`, `EnvSource`, and `LogSource`.

After [`aivi.prelude`](/stdlib/prelude), the modules most people reach for first are
[`aivi.option`](/stdlib/option), [`aivi.result`](/stdlib/result), [`aivi.list`](/stdlib/list),
[`aivi.text`](/stdlib/text), and [`aivi.math`](/stdlib/math).

## At a glance

| Module | Description | Selected exports or capability members |
| --- | --- | --- |
| [aivi.prelude](prelude.md) | Convenience re-exports and built-in types | `Validation`, `isValid`, `validationToResult`, `min`, `Functor` |
| [aivi.async](async.md) | Async operation lifecycle tracker | `AsyncTracker`, `step`, `isPending`, `isDone`, `isFailed` |
| [aivi.option](option.md) | Values that may be missing | `getOrElse`, `map`, `flatMap`, `toResult` |
| [aivi.result](result.md) | Success-or-error values | `withDefault`, `map`, `mapErr`, `flatMap` |
| [aivi.validation](validation.md) | Accumulating validation for independent inputs | `getOrElse`, `mapErr`, `zipValidation`, `fold` |
| [aivi.core.either](either.md) | Disjoint union holding one of two alternatives | `Left`, `Right`, `mapLeft`, `mapRight` |
| [aivi.list](list.md) | Purely functional list operations | `map`, `filter`, `maximum`, `unique`, `sort` |
| [aivi.nonEmpty](nonEmpty.md) | Non-empty list guaranteed at the type level | `head`, `last`, `singleton`, `cons`, `fromList` |
| [aivi.pair](pair.md) | Two-element tuples | `first`, `second`, `mapFirst`, `mapSecond` |
| [aivi.matrix](matrix.md) | Rectangular 2D collections | `init`, `fromRows`, `width`, `height`, `rows` |
| [aivi.core.dict](dict.md) | Association map keyed by any `Eq` type | `Dict`, `get`, `mergeWith`, `union` |
| [aivi.core.set](set.md) | Unordered set for any `Eq` type | `singleton`, `member`, `insert`, `union` |
| [aivi.core.range](range.md) | Inclusive integer range `[start, end]` | `RangeInt`, `make`, `length`, `contains` |
| [aivi.core.fn](fn.md) | Higher-order function combinators | `compose`, `const`, `flip`, `on` |
| [aivi.arithmetic](arithmetic.md) | Compiler-backed integer arithmetic intrinsics | `add`, `sub`, `mul`, `div`, `mod`, `neg` |
| [aivi.order](order.md) | `Ord`-driven ordering plus explicit comparator helpers | `min`, `max`, `minBy`, `clampBy` |
| [aivi.bool](bool.md) | Boolean helpers | `both`, `either`, `not`, `xor` |
| [aivi.bits](bits.md) | Compiler-backed bitwise integer intrinsics | `and`, `or`, `xor`, `not`, `shiftLeft` |
| [aivi.defaults](defaults.md) | Default values for common types | `defaultText`, `defaultInt`, `defaultBool` |
| [aivi.math](math.md) | Integer arithmetic utilities | `abs`, `clamp`, `min`, `max`, `gcd` |
| [aivi.core.float](float.md) | IEEE 754 double-precision helpers | `floor`, `ceil`, `round`, `sqrt`, `pi` |
| [aivi.bigint](bigint.md) | Arbitrary-size integers | `parse`, `plus`, `times`, `dividedBy` |
| [aivi.text](text.md) | Text manipulation | `length`, `contains`, `trim`, `split`, `toUpper` |
| [aivi.regex](regex.md) | Regular-expression matching and replacement | `matches`, `hasMatch`, `replaceFirst`, `allMatches` |
| [aivi.core.bytes](bytes.md) | Byte sequence operations | `fromText`, `toText`, `slice`, `append` |
| [aivi.data.json](json.md) | JSON text helpers plus structural JSON types | `validate`, `get`, `pretty`, `Json` |
| [aivi.duration](duration.md) | Typed time spans | `ms`, `sec`, `min`, `hr`, `millis` |
| [aivi.time](time.md) | Clocks, timestamps, and duration arithmetic | `nowMs`, `monotonicMs`, `toSeconds`, `elapsed` |
| [aivi.date](date.md) | Calendar data and pure formatting | `Date`, `TimeOfDay`, `DateDelta`, `dateToIso` |
| [aivi.timer](timer.md) | Marker types for timer-backed signals | `TimerTick`, `TimerReady`, `TimerMode` |
| [aivi.random](random.md) | Randomness vocabulary and `RandomSource` | `RandomSource`, `RandomError` |
| [aivi.fs](fs.md) | Filesystem vocabulary and `FsSource` | `FsSource`, `FsError`, `FsEvent` |
| [aivi.path](path.md) | Lexical path manipulation | `join`, `filename`, `parent`, `extension` |
| [aivi.env](env.md) | Environment vocabulary and `EnvSource` | `EnvSource`, `EnvEntry` |
| [aivi.stdio](stdio.md) | Standard I/O vocabulary and `StdioSource` | `StdioSource`, `Stream`, `Stdout`, `Stderr` |
| [aivi.log](log.md) | Logging vocabulary and `LogSource` | `levelToText`, `kv`, `LogSource` |
| [aivi.process](process.md) | Process vocabulary and `ProcessSource` | `command`, `args`, `workingDir`, `env` |
| [aivi.url](url.md) | Typed URLs with explicit parsing | `Url`, `UrlError`, `parse`, `toText` |
| [aivi.http](http.md) | HTTP vocabulary and `HttpSource` | `HttpSource`, `HttpResponse`, `HttpError` |
| [aivi.api](api.md) | OpenAPI capability auth and error vocabulary | `ApiAuth`, `ApiError`, `ApiResponse` |
| [aivi.auth](auth.md) | OAuth 2.0 / PKCE sign-in records | `PkceConfig`, `PkceToken`, `PkceState` |
| [aivi.db](db.md) | Database vocabulary and `DbSource` | `query`, `commit`, `DbSource` |
| [aivi.imap](imap.md) | Mailbox and folder types for IMAP integrations | `FolderSummary`, `MailEvent`, `lastSyncedAt` |
| [aivi.smtp](smtp.md) | Outgoing mail configuration and messages | `from`, `to`, `subject`, `bodyText`, `SmtpConfig` |
| [aivi.desktop.xdg](xdg.md) | XDG error vocabulary | `dataHome`, `configHome`, `cacheHome` |
| [aivi.portal](portal.md) | Desktop portal result vocabulary plus built-in portal sources | `openFile`, `openUri`, `screenshot` |
| [aivi.dbus](dbus.md) | D-Bus vocabulary and `DbusSource` | `destination`, `path`, `interface`, `member` |
| [aivi.gnome.tray](tray.md) | GNOME tray bridge vocabulary | `TraySource`, `defaultPath`, `actionMember` |
| [aivi.gnome.settings](settings.md) | GSettings schema, key, and value types | `SettingsSchema`, `SettingsKey`, `SettingValue` |
| [aivi.gnome.onlineAccounts](onlineAccounts.md) | Desktop account and token records | `id`, `token`, `tokenType`, `expiresAt` |
| [aivi.gnome.notifications](notifications.md) | Desktop notification capability vocabulary | `NotificationSource`, `NotificationTask`, `NotificationEvent` |
| [aivi.clipboard](clipboard.md) | Clipboard content types and watcher shapes | `ClipboardContent`, `ClipboardSource` |
| [aivi.color](color.md) | Packed ARGB color domain with blend and GNOME palette | `Color`, `blend`, `gnomeBlue3`, `gnomeRed3` |
| [aivi.px](px.md) | Pixel dimension domain for type-safe sizing | `px`, `addPx`, `subPx`, `scalePx`, `zero` |
| [aivi.gtk.styles](styles.md) | Adwaita CSS class name constants | `suggestedAction`, `destructiveAction`, `classes` |
| [aivi.gtk.icons](icons.md) | Icon-name text constants | `documentOpen`, `documentSave`, `goHomeSymbolic` |
| [aivi.secret](secret.md) | Desktop keyring capability vocabulary | `SecretSource`, `SecretError`, `SecretTask` |
| [aivi.image](image.md) | Image data, metadata, and load errors | `format`, `size`, `bytes`, `hasAlpha` |
| [aivi.gresource](gresource.md) | Bundled GResource paths and load errors | `ResourcePath`, `ResourceError`, `ResourceTask` |

## Built-in types you will see often

| Type | Meaning | More |
|---|---|---|
| `Ordering` | The result of comparing two values: `Less`, `Equal`, or `Greater`. | [`aivi.order`](/stdlib/order) |
| `Option A` | A value that may be missing. | [`aivi.option`](/stdlib/option) |
| `Result E A` | A success value (`Ok`) or a failure value (`Err`). | [`aivi.result`](/stdlib/result) |
| `Validation E A` | Like `Result`, but useful when checking several independent inputs. | [`aivi.validation`](/stdlib/validation) |
| `Signal A` | A reactive value that changes over time. | [Signals guide](/guide/signals) |
| `Task E A` | Runtime work that may fail with `E` or succeed with `A`. | Used across I/O-oriented stdlib modules |

## Browse modules by area

### Start here

- [`aivi.prelude`](/stdlib/prelude) — a good first stop for everyday imports.
- [`aivi.defaults`](/stdlib/defaults) — named empty/default values for common built-in types.

### Core values and collections

- [`aivi.bool`](/stdlib/bool) — boolean helpers.
- [`aivi.option`](/stdlib/option) — values that may be missing.
- [`aivi.result`](/stdlib/result) — success-or-error values.
- [`aivi.validation`](/stdlib/validation) — validation values for user input and other checks.
- [`aivi.core.either`](/stdlib/either) — values that can hold one of two branches.
- [`aivi.list`](/stdlib/list) — list helpers.
- [`aivi.matrix`](/stdlib/matrix) — rectangular two-dimensional collections.
- [`aivi.nonEmpty`](/stdlib/nonEmpty) — lists that always contain at least one item.
- [`aivi.pair`](/stdlib/pair) — two values grouped together.
- [`aivi.order`](/stdlib/order) — comparison results and ordering helpers.
- [`aivi.core.dict`](/stdlib/dict) — key/value dictionaries.
- [`aivi.core.set`](/stdlib/set) — collections of unique values.
- [`aivi.core.range`](/stdlib/range) — numeric ranges.
- [`aivi.core.fn`](/stdlib/fn) — small function and pipeline helpers.
- [`aivi.arithmetic`](/stdlib/arithmetic) — compiler-backed integer arithmetic intrinsics.
- [`aivi.bits`](/stdlib/bits) — compiler-backed bitwise integer intrinsics.

### Numbers, text, and data

- [`aivi.math`](/stdlib/math) — everyday arithmetic helpers.
- [`aivi.core.float`](/stdlib/float) — floating-point numbers.
- [`aivi.bigint`](/stdlib/bigint) — integers that can grow past the normal `Int` range.
- [`aivi.text`](/stdlib/text) — text helpers.
- [`aivi.regex`](/stdlib/regex) — regular-expression matching and replacement.
- [`aivi.core.bytes`](/stdlib/bytes) — byte buffers.
- [`aivi.data.json`](/stdlib/json) — JSON text helpers plus structural JSON types.

### Time, randomness, and scheduling

- [`aivi.duration`](/stdlib/duration) — typed time spans such as `5sec`.
- [`aivi.time`](/stdlib/time) — clocks, timestamps, and duration arithmetic.
- [`aivi.date`](/stdlib/date) — calendar data types and pure formatting.
- [`aivi.timer`](/stdlib/timer) — marker types for timer-backed signals.
- [`aivi.random`](/stdlib/random) — randomness vocabulary plus `RandomSource`.

### Files, environment, and processes

- [`aivi.fs`](/stdlib/fs) — filesystem vocabulary plus `FsSource`.
- [`aivi.path`](/stdlib/path) — lexical path helpers and a distinct nominal `Path` domain.
- [`aivi.env`](/stdlib/env) — environment vocabulary plus `EnvSource`.
- [`aivi.stdio`](/stdlib/stdio) — stdio vocabulary plus `StdioSource`.
- [`aivi.log`](/stdlib/log) — logging vocabulary plus `LogSource`.
- [`aivi.process`](/stdlib/process) — process vocabulary plus `ProcessSource` capabilities.

### Network and services

Some modules in this group are full helpers, and some are shared data shapes for integrations.
The linked pages spell out which functions exist today.

- [`aivi.url`](/stdlib/url) — validated, normalized URLs and their text representation.
- [`aivi.http`](/stdlib/http) — HTTP vocabulary plus `HttpSource`.
- [`aivi.api`](/stdlib/api) — auth and error vocabulary shared by `@source api`.
- [`aivi.auth`](/stdlib/auth) — OAuth / PKCE sign-in records and state types.
- [`aivi.db`](/stdlib/db) — database vocabulary plus `DbSource`.
- [`aivi.imap`](/stdlib/imap) — mailbox records used by the built-in IMAP sources.
- [`aivi.smtp`](/stdlib/smtp) — outgoing mail settings, messages, and errors.

### Desktop, UI, and GNOME

Many pages in this group describe handle vocabularies, watcher/source shapes, partial runtime
surfaces, or shared desktop data types rather than a full feature API. They are still the right
place to look when wiring a Linux desktop app together.

- [`aivi.desktop.xdg`](/stdlib/xdg) — XDG error vocabulary; actual directories come from `PathSource`.
- [`aivi.portal`](/stdlib/portal) — desktop portal results and built-in sources for file picking, opening URIs, and screenshots.
- [`aivi.dbus`](/stdlib/dbus) — D-Bus vocabulary plus `DbusSource`.
- [`aivi.gnome.settings`](/stdlib/settings) — GSettings schema, key, and value types.
- [`aivi.secret`](/stdlib/secret) — desktop keyring capabilities.
- [`aivi.gnome.tray`](/stdlib/tray) — the application-side tray bridge.
- [`aivi.gnome.onlineAccounts`](/stdlib/onlineAccounts) — desktop account and token records.
- [`aivi.gnome.notifications`](/stdlib/notifications) — desktop notification payloads, tasks, and response events.
- [`aivi.clipboard`](/stdlib/clipboard) — clipboard content types and watcher shapes.
- [`aivi.color`](/stdlib/color) — packed UI colors and channel helpers.
- [`aivi.px`](/stdlib/px) — pixel dimensions.
- [`aivi.gtk.icons`](/stdlib/icons) — icon-name constants.
- [`aivi.gtk.styles`](/stdlib/styles) — CSS class constants.
- [`aivi.image`](/stdlib/image) — image data, metadata, and load errors.
- [`aivi.gresource`](/stdlib/gresource) — bundled resource paths and load errors.

## Common interfaces (typeclasses)

Classes describe operations shared by multiple types. Use the
[canonical executable-support table](/guide/typeclasses#canonical-builtin-executable-support)
for the exact builtin carrier/class matrix, and [class laws](/guide/class-laws) for semantic
contracts. That table distinguishes builtin support from imported authored instances and
avoids treating parser acceptance as proof of runtime support.
