use std::fmt;

/// A tiny pretty-printing algebra (Wadler/Leijen style).
///
/// The formatter builds a `Doc` tree and the renderer chooses between flat vs broken layouts
/// using `Group` + `Line(Soft)` (softline) nodes.
#[derive(Clone)]
pub enum Doc {
    Nil,
    Text(String),
    Line(LineKind),
    Concat(Vec<Doc>),
    Indent(usize, Box<Doc>),
    Group(Box<Doc>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Hard,
    Soft,
}

impl Doc {
    pub fn nil() -> Self {
        Doc::Nil
    }

    pub fn text<T: Into<String>>(text: T) -> Self {
        Doc::Text(text.into())
    }

    pub fn hardline() -> Self {
        Doc::Line(LineKind::Hard)
    }

    pub fn softline() -> Self {
        Doc::Line(LineKind::Soft)
    }

    pub fn concat(items: Vec<Doc>) -> Self {
        let mut out = Vec::new();
        for item in items {
            match item {
                Doc::Nil => {}
                Doc::Concat(inner) => out.extend(inner),
                other => out.push(other),
            }
        }
        if out.is_empty() {
            Doc::Nil
        } else if out.len() == 1 {
            out.pop().unwrap()
        } else {
            Doc::Concat(out)
        }
    }

    pub fn indent(self, spaces: usize) -> Self {
        if spaces == 0 {
            self
        } else {
            Doc::Indent(spaces, Box::new(self))
        }
    }

    pub fn group(self) -> Self {
        Doc::Group(Box::new(self))
    }
}

impl fmt::Debug for Doc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Doc::Nil => write!(f, "Nil"),
            Doc::Text(t) => write!(f, "Text({t:?})"),
            Doc::Line(k) => write!(f, "Line({k:?})"),
            Doc::Concat(items) => f.debug_tuple("Concat").field(items).finish(),
            Doc::Indent(n, doc) => f.debug_tuple("Indent").field(n).field(doc).finish(),
            Doc::Group(doc) => f.debug_tuple("Group").field(doc).finish(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Mode {
    Flat,
    Break,
}

/// Render a `Doc` into a string with a max line width.
///
/// This is deterministic and linear-ish: the "fits" check is bounded by `max_width` and stops
/// early when the flat layout would exceed the remaining space.
pub fn render(doc: Doc, max_width: usize) -> String {
    let max_width = max_width.clamp(20, 400);
    let mut out = String::new();

    // Work stack: (indent, mode, doc)
    let mut stack: Vec<(usize, Mode, Doc)> = vec![(0, Mode::Break, doc)];
    let mut col = 0usize;

    while let Some((indent, mode, doc)) = stack.pop() {
        match doc {
            Doc::Nil => {}
            Doc::Text(s) => {
                col += s.chars().count();
                out.push_str(&s);
            }
            Doc::Line(kind) => match kind {
                LineKind::Hard => {
                    out.push('\n');
                    for _ in 0..indent {
                        out.push(' ');
                    }
                    col = indent;
                }
                LineKind::Soft => match mode {
                    Mode::Flat => {
                        out.push(' ');
                        col += 1;
                    }
                    Mode::Break => {
                        out.push('\n');
                        for _ in 0..indent {
                            out.push(' ');
                        }
                        col = indent;
                    }
                },
            },
            Doc::Concat(items) => {
                for item in items.into_iter().rev() {
                    stack.push((indent, mode, item));
                }
            }
            Doc::Indent(extra, doc) => {
                stack.push((indent + extra, mode, *doc));
            }
            Doc::Group(doc) => {
                let doc = *doc;
                let fits_flat = fits(max_width.saturating_sub(col), indent, &doc, &stack);
                stack.push((indent, if fits_flat { Mode::Flat } else { Mode::Break }, doc));
            }
        }
    }

    out
}

fn fits(remaining: usize, indent: usize, doc: &Doc, rest: &[(usize, Mode, Doc)]) -> bool {
    let mut remaining = remaining as isize;
    let mut stack: Vec<(usize, Mode, &Doc)> = Vec::new();
    stack.push((indent, Mode::Flat, doc));
    for (i, m, d) in rest.iter().rev() {
        stack.push((*i, *m, d));
    }

    while remaining >= 0 {
        let Some((indent, mode, doc)) = stack.pop() else {
            return true;
        };
        match doc {
            Doc::Nil => {}
            Doc::Text(s) => {
                remaining -= s.chars().count() as isize;
            }
            Doc::Line(kind) => match kind {
                LineKind::Hard => return true,
                LineKind::Soft => match mode {
                    Mode::Flat => remaining -= 1,
                    Mode::Break => return true,
                },
            },
            Doc::Concat(items) => {
                for item in items.iter().rev() {
                    stack.push((indent, mode, item));
                }
            }
            Doc::Indent(extra, doc) => stack.push((indent + extra, mode, doc)),
            Doc::Group(doc) => stack.push((indent, Mode::Flat, doc)),
        }
    }

    false
}

