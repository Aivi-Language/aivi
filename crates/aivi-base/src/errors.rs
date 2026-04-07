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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TestError(String);

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(&self.0)
        }
    }

    #[test]
    fn empty_collection_is_empty() {
        let errors = ErrorCollection::<TestError>::new(vec![]);
        assert!(errors.is_empty());
        assert_eq!(errors.errors().len(), 0);
    }

    #[test]
    fn new_preserves_errors() {
        let errors =
            ErrorCollection::new(vec![TestError("first".into()), TestError("second".into())]);
        assert!(!errors.is_empty());
        assert_eq!(errors.errors().len(), 2);
        assert_eq!(errors.errors()[0].0, "first");
    }

    #[test]
    fn into_errors_consumes_collection() {
        let errors = ErrorCollection::new(vec![TestError("only".into())]);
        let vec = errors.into_errors();
        assert_eq!(vec.len(), 1);
        assert_eq!(vec[0].0, "only");
    }

    #[test]
    fn display_joins_with_semicolons() {
        let errors = ErrorCollection::new(vec![
            TestError("a".into()),
            TestError("b".into()),
            TestError("c".into()),
        ]);
        assert_eq!(format!("{errors}"), "a; b; c");
    }

    #[test]
    fn display_single_error_has_no_separator() {
        let errors = ErrorCollection::new(vec![TestError("only".into())]);
        assert_eq!(format!("{errors}"), "only");
    }

    #[test]
    fn display_empty_is_empty_string() {
        let errors = ErrorCollection::<TestError>::new(vec![]);
        assert_eq!(format!("{errors}"), "");
    }
}
