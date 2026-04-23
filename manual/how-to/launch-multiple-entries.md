# How to launch multiple entries together

Use a lightweight launcher entry, then let the manifest declare any heavier companions that should
start after it is ready. The launcher can be a small splash window or a headless supervisor.

## 1. Pick a fast launcher entry

Your `[run]` entry should be the smallest launcher surface that can become ready immediately.

```toml
[run]
entry = "apps/launcher/main.aivi"
```

That launcher can be a simple splash window or a headless `Task` entry. Keep expensive sources and
large module graphs in the entries that come later.

## 2. Add `[[run.launch]]` parts

Each `[[run.launch]]` block describes one extra `aivi run` child process to start after the
launcher is ready.

```toml
[run]
entry = "apps/launcher/main.aivi"

[[run.launch]]
label = "Main window"
entry = "apps/ui/main.aivi"

[[run.launch]]
label = "Daemon"
entry = "apps/daemon/main.aivi"

[[run.launch]]
label = "Tray"
entry = "apps/tray/main.aivi"
```

- `label` is shown in the terminal progress tracker.
- `entry` is resolved relative to the `aivi.toml` that supplied `[run]`.
- `view` is optional when the child needs a non-default markup value.

## 3. Run from the workspace root

```sh
aivi run
```

After the launcher is ready, AIVI starts each declared companion as its own child `aivi run`
process. If the launcher has windows, those can present first; if it is headless, the terminal
progress tracker becomes the only startup surface.

## 4. Read the launch tracker

The run tracker keeps a dedicated `launch` lane for manifest launch parts. Each part moves through:

- `queued`
- `starting`
- `started`
- `failed`

This lets one terminal session explain what the launcher is doing without falling back to shell
wrappers.

## 5. Know when it applies

`[[run.launch]]` belongs to the default `[run]` workflow. If you invoke one entry directly:

```sh
aivi run --path apps/ui/main.aivi
aivi run --app tray
```

those commands run only the requested entry. They do not inherit the default multi-entry launch
fan-out.
