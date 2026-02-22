use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;

use parking_lot::RwLock;

// ---------------------------------------------------------------------------
// Global string interner
// ---------------------------------------------------------------------------

struct InternerInner {
    map: HashMap<&'static str, Symbol>,
    strings: Vec<&'static str>,
}

static GLOBAL_INTERNER: OnceLock<RwLock<InternerInner>> = OnceLock::new();

fn interner() -> &'static RwLock<InternerInner> {
    GLOBAL_INTERNER.get_or_init(|| {
        RwLock::new(InternerInner {
            map: HashMap::new(),
            strings: Vec::new(),
        })
    })
}

/// A compact, `Copy`-able handle to an interned string.
///
/// Two `Symbol`s compare equal if and only if they refer to the same interned
/// string.  This makes name comparisons O(1) instead of O(N).
///
/// Interning happens through the global interner via [`Symbol::intern`].
/// The interned strings are leaked (`'static`) so `Symbol::as_str()` returns
/// `&'static str` without needing a borrow on the interner.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Symbol(u32);

impl Symbol {
    /// Intern the given string, returning a `Symbol` handle.
    ///
    /// If the string was already interned, the existing handle is returned.
    pub fn intern(s: &str) -> Self {
        // Fast-path: check if already interned under a read lock.
        {
            let inner = interner().read();
            if let Some(&sym) = inner.map.get(s) {
                return sym;
            }
        }
        // Slow-path: acquire write lock and insert.
        let mut inner = interner().write();
        // Double-check after acquiring write lock.
        if let Some(&sym) = inner.map.get(s) {
            return sym;
        }
        let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
        let sym = Symbol(inner.strings.len() as u32);
        inner.strings.push(leaked);
        inner.map.insert(leaked, sym);
        sym
    }

    /// Resolve this symbol back to its string slice.
    #[inline]
    pub fn as_str(self) -> &'static str {
        let inner = interner().read();
        inner.strings[self.0 as usize]
    }

    /// The raw u32 index.
    #[inline]
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Symbol({:?})", self.as_str())
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<str> for Symbol {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Symbol {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::ops::Deref for Symbol {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Symbol {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl PartialOrd for Symbol {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Symbol {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Default for Symbol {
    fn default() -> Self {
        Symbol::intern("")
    }
}

impl From<Symbol> for String {
    fn from(sym: Symbol) -> String {
        sym.as_str().to_owned()
    }
}

impl From<&str> for Symbol {
    fn from(s: &str) -> Self {
        Symbol::intern(s)
    }
}

impl From<String> for Symbol {
    fn from(s: String) -> Self {
        Symbol::intern(&s)
    }
}

impl serde::Serialize for Symbol {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// A typed index into the expression arena inside [`AstArena`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExprId(u32);

impl ExprId {
    #[inline]
    pub fn new(idx: u32) -> Self {
        Self(idx)
    }

    #[inline]
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// A typed index into the pattern arena inside [`AstArena`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PatternId(u32);

impl PatternId {
    #[inline]
    pub fn new(idx: u32) -> Self {
        Self(idx)
    }

    #[inline]
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// A typed index into the type-expression arena inside [`AstArena`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeExprId(u32);

impl TypeExprId {
    #[inline]
    pub fn new(idx: u32) -> Self {
        Self(idx)
    }

    #[inline]
    pub fn as_u32(self) -> u32 {
        self.0
    }
}
