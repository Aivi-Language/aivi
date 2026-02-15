pub const MODULE_NAME: &str = "aivi";

pub const SOURCE: &str = r#"
@no_prelude
module aivi
export Unit, Bool, Int, Float, Text, Char, Bytes, DateTime
export List, Option, Result, Tuple, Map, Set, Queue, Deque, Heap
export Source, SourceError
export None, Some, Ok, Err, True, False
export pure, fail, attempt, load

export text, regex, math, calendar, color
export bigint, rational, decimal
export url, console, crypto, env, system, logger, database, file, clock, random, channel, concurrent, httpServer, ui, http, https, sockets, streams, collections, i18n
export linalg, signal, graph"#;
