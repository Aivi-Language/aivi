use std::fmt;

/// Generic error-collection container used by every compiler pass.
/// Each IR layer aliases this with its own concrete error type.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ErrorCollection<E> {
    errors: Vec<E>,
}

impl<E> ErrorCollection<E> {
    pub fn new(errors: Vec<E>) -> Self {
        Self { errors }
    }

    pub fn errors(&self) -> &[E] {
        &self.errors
    }

    pub fn into_errors(self) -> Vec<E> {
        self.errors
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

impl<E: fmt::Display> fmt::Display for ErrorCollection<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.errors.iter().enumerate() {
            if index > 0 {
                f.write_str("; ")?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

impl<E: fmt::Display + fmt::Debug> std::error::Error for ErrorCollection<E> {}
