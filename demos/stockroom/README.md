# Stockroom

A small restock dashboard written in AIVI. It reads a JSON inventory, rejects invalid
quantities and duplicate SKUs, and lists shortages by descending quantity with SKU as
the tie-breaker. Search matches SKU or name, ignoring case and surrounding whitespace.
Reload reads the file again and preserves the search query.

From the repository root:

```sh
cargo build --bin aivi
target/debug/aivi check --runnable demos/stockroom/main.aivi
target/debug/aivi test demos/stockroom/tests.aivi
cd demos/stockroom
../../target/debug/aivi run main.aivi
```

Edit `inventory.json`, then press **Reload**. The application reads relative to its working
directory. It does not write the inventory. Its small-file validation checks SKU uniqueness
with a scan per item; this is a small stockroom example, not a large-catalog benchmark.

The input is an object with an `items` array. Each item has exactly `sku` and `name` text
fields, plus `onHand` and `target` integer fields. Quantities must be nonnegative; SKU and
name must contain non-whitespace characters. SKUs are case-sensitive identifiers.
Replenishment quantity is `target - onHand` only when `onHand < target`.

- `inventory.aivi`: pure model, validation, search, sorting, and text formatting.
- `main.aivi`: filesystem source, derived signals, and GTK markup.
- `tests.aivi`: executable domain examples.

To exercise actual reads, errors, recovery, and GTK events in a desktop session:

```sh
python3 demos/stockroom/smoke.py
```

Run that command from the repository root. It uses temporary input files and writes screenshots
to `out/stockroom.png` and `out/stockroom-360.png`.

To package the application from the repository root:

```sh
target/debug/aivi build demos/stockroom/main.aivi -o out/stockroom
AIVI_STOCKROOM_FILE="$PWD/demos/stockroom/inventory.json" out/stockroom
```

The executable contains the application bundle and a copy of the sample inventory. Set
`AIVI_STOCKROOM_FILE` to an absolute path to use a live external file. Without that setting,
the packaged app reads the bundled sample in its extracted launch directory. Source-backed
runs default to `inventory.json` in their working directory. `aivi compile` emits an object file; `aivi build` is the runnable deployment
path. Read the [language audit](../../manual/explanation/stockroom-language-audit.md) for findings,
comparisons, and proposed improvements.
