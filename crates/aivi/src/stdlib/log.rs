pub const MODULE_NAME: &str = "aivi.log";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.log
export Level, Context
export Trace, Debug, Info, Warn, Error
export logger, log, trace, debug, info, warn, error

use aivi

Level = Trace | Debug | Info | Warn | Error
Context = List (Text, Text)

log : Level -> Text -> Context -> Effect Text Unit
log = level message context => logger.log level message context

trace : Text -> Context -> Effect Text Unit
trace = message context => logger.trace message context

debug : Text -> Context -> Effect Text Unit
debug = message context => logger.debug message context

info : Text -> Context -> Effect Text Unit
info = message context => logger.info message context

warn : Text -> Context -> Effect Text Unit
warn = message context => logger.warn message context

error : Text -> Context -> Effect Text Unit
error = message context => logger.error message context
"#;
