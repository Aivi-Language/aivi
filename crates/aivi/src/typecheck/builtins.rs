use super::TypeChecker;
use crate::typecheck::types::{Kind, TypeEnv};

mod collections;
mod core_io_concurrency_html;
mod decimal_and_networking;
mod linalg_signal_graph_system_db;
mod math_calendar_numbers;
mod text_and_regex;

impl TypeChecker {
    pub(super) fn register_builtin_types(&mut self) {
        let star = Kind::Star;
        let arrow = |a, b| Kind::Arrow(Box::new(a), Box::new(b));

        for name in [
            "Unit",
            "Bool",
            "Int",
            "Float",
            "Text",
            "Char",
            "Bytes",
            "Patch",
            "Map",
            "Set",
            "Queue",
            "Deque",
            "Heap",
            "Vec",
            "Mat",
            "Signal",
            "Spectrum",
            "Graph",
            "Edge",
            "Generator",
            "Html",
            "DateTime",
            "Regex",
            "BigInt",
            "Rational",
            "Decimal",
            "FileHandle",
            "FileStats",
            "Listener",
            "Connection",
            "Stream",
            "Send",
            "Recv",
            "Closed",
            "Server",
            "WebSocket",
            "HttpError",
            "WsError",
            "ServerReply",
            "WsMessage",
        ] {
            self.builtin_types.insert(name.to_string(), star.clone());
        }

        // Higher kinded types
        self.builtin_types
            .insert("List".to_string(), arrow(star.clone(), star.clone()));
        self.builtin_types
            .insert("Option".to_string(), arrow(star.clone(), star.clone()));
        // `Resource E A` mirrors `Effect E A`: acquisition may fail with `E`.
        self.builtin_types.insert(
            "Resource".to_string(),
            arrow(star.clone(), arrow(star.clone(), star.clone())),
        );
        self.builtin_types.insert(
            "Result".to_string(),
            arrow(star.clone(), arrow(star.clone(), star.clone())),
        );
        self.builtin_types.insert(
            "Effect".to_string(),
            arrow(star.clone(), arrow(star.clone(), star.clone())),
        );

        self.type_constructors = self.builtin_types.clone();
    }

    pub(super) fn builtin_type_constructors(&self) -> std::collections::HashMap<String, Kind> {
        self.builtin_types.clone()
    }

    pub(super) fn register_builtin_values(&mut self) {
        let mut env = TypeEnv::default();
        core_io_concurrency_html::register(self, &mut env);
        text_and_regex::register(self, &mut env);
        math_calendar_numbers::register(self, &mut env);
        decimal_and_networking::register(self, &mut env);
        collections::register(self, &mut env);
        linalg_signal_graph_system_db::register(self, &mut env);
        self.builtins = env;
    }
}
