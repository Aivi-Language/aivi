#[path = "main/daemon.rs"]
mod daemon;
#[path = "main/repl/mod.rs"]
mod repl;
#[path = "main/watch.rs"]
mod watch;
include!("main/cli.rs");
include!("main/commands.rs");
