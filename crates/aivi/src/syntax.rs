pub const KEYWORDS_CONTROL: &[&str] = &[
    "do", "effect", "generate", "resource", "if", "then", "else", "when", "unless", "yield",
    "loop", "recurse", "or", "match", "given", "mock", "in",
];

pub const KEYWORDS_OTHER: &[&str] = &[
    "module", "export", "use", "as", "hiding", "domain", "class", "instance", "over", "patch",
    "with", "machine", "on", "snapshot",
];

pub const KEYWORDS_ALL: &[&str] = &[
    "do", "effect", "generate", "resource", "if", "then", "else", "when", "unless", "yield",
    "loop", "recurse", "or", "match", "given", "mock", "in", "module", "export", "use", "as",
    "hiding", "domain", "class", "instance", "over", "patch", "with", "machine", "on", "snapshot",
];

pub const BOOLEAN_LITERALS: &[&str] = &["True", "False"];

pub const CONSTRUCTORS_COMMON: &[&str] = &["None", "Some", "Ok", "Err"];

pub const SYMBOLS_3: &[([char; 3], &str)] = &[(['.', '.', '.'], "...")];

pub const SYMBOLS_2: &[([char; 2], &str)] = &[
    (['=', '>'], "=>"),
    (['-', '>'], "->"),
    (['<', '-'], "<-"),
    (['<', '|'], "<|"),
    (['|', '>'], "|>"),
    (['=', '='], "=="),
    (['!', '='], "!="),
    (['<', '='], "<="),
    (['>', '='], ">="),
    (['&', '&'], "&&"),
    (['|', '|'], "||"),
    ([':', ':'], "::"),
    (['+', '+'], "++"),
    (['?', '?'], "??"),
    (['<', '<'], "<<"),
    (['>', '>'], ">>"),
    ([':', '='], ":="),
    (['.', '.'], ".."),
];

pub const SYMBOLS_1: &[char] = &[
    '{', '}', '(', ')', '[', ']', ',', '.', ':', '=', '+', '-', '*', '/', '|', '!', '<', '>', '?',
    '@', '%', '~', '^', '×',
];
