#[path = "main/repl/mod.rs"]
mod repl;
#[path = "main/watch.rs"]
mod watch;
include!("main/cli.rs");
include!("main/commands.rs");
