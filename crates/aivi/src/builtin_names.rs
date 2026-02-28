/// Single source of truth for all builtin names recognised by the AIVI compiler and runtime.
///
/// There are two categories:
///
/// - **Value builtins** — runtime values (constructors, functions, module records) registered
///   by `register_builtins` and resolved at compile-time by `resolve_builtin`.
///
/// - **Type builtins** — type-level names used only by the unused-binding diagnostic
///   (`is_builtin_name`) to suppress false warnings on standard type annotations.
///
/// Value-level builtin names.  Both the JIT compiler (`resolve_builtin`) and the runtime
/// (`register_builtins`) must stay in sync with this list.
pub const BUILTIN_VALUE_NAMES: &[&str] = &[
    // Constructors / constants
    "Unit",
    "True",
    "False",
    "None",
    "Some",
    "Ok",
    "Err",
    "Closed",
    // Core functions
    "foldGen",
    "constructorName",
    "constructorOrdinal",
    "__machine_on",
    "pure",
    "fail",
    "attempt",
    "load",
    "bind",
    "print",
    "println",
    "map",
    "chain",
    "assertEq",
    "__assertSnapshot",
    // Module records (stdlib namespaces)
    "file",
    "env",
    "system",
    "clock",
    "random",
    "channel",
    "concurrent",
    "httpServer",
    "ui",
    "gtk4",
    "text",
    "regex",
    "math",
    "calendar",
    "color",
    "linalg",
    "signal",
    "graph",
    "bigint",
    "rational",
    "decimal",
    "url",
    "http",
    "https",
    "rest",
    "email",
    "sockets",
    "streams",
    "instant",
    "collections",
    "console",
    "crypto",
    "logger",
    "database",
    "i18n",
    "goa",
    "secrets",
    "timezone",
    // Collection types
    "List",
    "Map",
    "Set",
    "Queue",
    "Deque",
    "Heap",
];

/// Type-level builtin names used only by the unused-binding diagnostic.
pub const BUILTIN_TYPE_NAMES: &[&str] = &[
    "Bool",
    "Int",
    "Float",
    "Text",
    "Char",
    "Bytes",
    "Effect",
    "Stream",
    "Listener",
    "Connection",
];

/// Returns `true` if `name` is any kind of builtin (value or type).
pub fn is_builtin_name(name: &str) -> bool {
    BUILTIN_VALUE_NAMES.contains(&name) || BUILTIN_TYPE_NAMES.contains(&name)
}

/// Returns `Some(name)` when `name` is a value-level builtin recognised by the JIT.
pub fn resolve_builtin(name: &str) -> Option<String> {
    if BUILTIN_VALUE_NAMES.contains(&name) {
        Some(name.to_string())
    } else {
        None
    }
}
