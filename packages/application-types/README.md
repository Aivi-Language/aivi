# Application types

Optional application vocabulary: lifecycle states, commands, undo state, and in-app notifications. These are ordinary data types; this package does not implement an application framework or undo engine.

Copy the `aivi/app` directory into your project’s `aivi` directory next to `aivi.toml`. You can then import `aivi.app.lifecycle`. AIVI currently resolves local source modules; no package-manager dependency declaration is required.

See [API](API.md) for the exported data types.
