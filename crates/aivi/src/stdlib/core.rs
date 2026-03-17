pub const MODULE_NAME: &str = "aivi";

pub const SOURCE: &str = r#"
@no_prelude
module aivi
export Unit, Bool, Int, Float, Text, Char, Bytes, DateTime
export List, Option, Result, Validation, Tuple, Map, Set, Queue, Deque, Heap
export Source, SourceError
export None, Some, Ok, Err, Valid, Invalid, IOError, DecodeError, True, False
export pure, fail, attempt, load, constructorName, constructorOrdinal

export source, text, regex, math, calendar, color
export bigint, rational, decimal
export json, toJson
export url, console, crypto, env, system, logger, database, file, clock, instant, random, channel, concurrent, httpServer, ui, http, https, rest, email, gnomeOnlineAccounts, sockets, streams, collections, i18n, reactive
export linalg, graph, tree"#;
