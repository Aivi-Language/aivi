use std::path::PathBuf;

use crate::surface::{parse_modules, Module};

const CORE_SOURCE: &str = r#"
@no_prelude
module aivi.std.core = {
  export Unit, Bool, Int, Float, Text, Char
  export List, Option, Result, Tuple
  export None, Some, Ok, Err, True, False
  export pure, fail, attempt, load, print
}
"#;

const PRELUDE_SOURCE: &str = r#"
@no_prelude
module aivi.prelude = {
  export Unit, Bool, Int, Float, Text, Char
  export List, Option, Result, Tuple
  export None, Some, Ok, Err, True, False
  export Eq, Ord, Show, Num
  export Functor, Applicative, Monad
  export pure, fail, attempt, load, print

  export domain Calendar
  export domain Duration
  export domain Color
  export domain Vector

  use aivi.std.core
  use aivi.std.calendar
  use aivi.std.duration
  use aivi.std.color
  use aivi.std.vector

  class Eq A = {
    eq: A -> A -> Bool
  }

  class Ord A = {
    lt: A -> A -> Bool
    lte: A -> A -> Bool
  }

  class Show A = {
    show: A -> Text
  }

  class Num A = {
    add: A -> A -> A
    sub: A -> A -> A
    mul: A -> A -> A
    neg: A -> A
  }

  class Functor (F *) = {
    map: F A -> (A -> B) -> F B
  }

  class Applicative (F *) = {
    pure: A -> F A
    apply: F (A -> B) -> F A -> F B
  }

  class Monad (M *) = {
    pure: A -> M A
    flatMap: M A -> (A -> M B) -> M B
  }
}
"#;

pub fn embedded_stdlib_modules() -> Vec<Module> {
    let mut modules = Vec::new();
    modules.extend(parse_embedded("aivi.std.core", CORE_SOURCE));
    modules.extend(parse_embedded("aivi.prelude", PRELUDE_SOURCE));
    modules
}

fn parse_embedded(name: &str, source: &str) -> Vec<Module> {
    let path = PathBuf::from(format!("<embedded:{name}>"));
    let (modules, diagnostics) = parse_modules(path.as_path(), source);
    debug_assert!(
        diagnostics.is_empty(),
        "embedded stdlib module {name} failed to parse"
    );
    modules
}
