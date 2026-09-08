use std::path::PathBuf;

use ropey::{Rope, RopeSlice};
use tower_lsp::lsp_types::{Position, TextDocumentContentChangeEvent, Url};

use crate::state::{DocumentSnapshot, DocumentState, ServerState};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentChangeError {
    NotOpen,
    StaleVersion { current: i32, received: i32 },
    LineOutOfBounds { line: u32 },
    CharacterOutOfBounds { line: u32, character: u32 },
    CharacterSplitsSurrogate { line: u32, character: u32 },
    ReversedRange,
    RangeLengthMismatch { expected: u32, received: u32 },
    RangeTooLarge,
}

/// Open or replace a document with the client's authoritative version.
pub fn open_document(state: &ServerState, uri: &Url, version: i32, text: String) {
    let path = uri_to_path(uri);
    let rope = Rope::from_str(&text);
    let file = state.db.open_file(path, text);
    state.documents.insert(
        uri.clone(),
        DocumentState {
            file,
            version,
            text: rope,
        },
    );
}

/// Apply one LSP change batch transactionally.
///
/// Every range is interpreted against the result of the preceding change in
/// the same notification. The query input and stored version are committed
/// only after every UTF-16 position and optional range length has validated.
pub fn change_document(
    state: &ServerState,
    uri: &Url,
    version: i32,
    changes: &[TextDocumentContentChangeEvent],
) -> Result<DocumentSnapshot, DocumentChangeError> {
    let Some(mut document) = state.documents.get_mut(uri) else {
        return Err(DocumentChangeError::NotOpen);
    };
    if version <= document.version {
        return Err(DocumentChangeError::StaleVersion {
            current: document.version,
            received: version,
        });
    }

    let mut text = document.text.clone();
    for change in changes {
        apply_change(&mut text, change)?;
    }

    document.file.set_text(&state.db, text.to_string());
    document.version = version;
    document.text = text.clone();
    Ok(DocumentSnapshot {
        file: document.file,
        version,
        text,
    })
}

/// Remove a document from tracking and from the database.
pub fn close_document(state: &ServerState, uri: &Url) -> Option<DocumentSnapshot> {
    let (_, document) = state.documents.remove(uri)?;
    state.semantic_tokens.remove(uri);
    state.db.remove_file(document.file);
    Some(DocumentSnapshot {
        file: document.file,
        version: document.version,
        text: document.text,
    })
}

fn apply_change(
    text: &mut Rope,
    change: &TextDocumentContentChangeEvent,
) -> Result<(), DocumentChangeError> {
    let Some(range) = change.range else {
        *text = Rope::from_str(&change.text);
        return Ok(());
    };

    let start = position_to_char(text, range.start)?;
    let end = position_to_char(text, range.end)?;
    if start > end {
        return Err(DocumentChangeError::ReversedRange);
    }
    if let Some(received) = change.range_length {
        let expected = utf16_len(text.slice(start..end))?;
        if expected != received {
            return Err(DocumentChangeError::RangeLengthMismatch { expected, received });
        }
    }

    text.remove(start..end);
    text.insert(start, &change.text);
    Ok(())
}

fn position_to_char(text: &Rope, position: Position) -> Result<usize, DocumentChangeError> {
    let line_index =
        usize::try_from(position.line).map_err(|_| DocumentChangeError::LineOutOfBounds {
            line: position.line,
        })?;
    if line_index >= text.len_lines() {
        return Err(DocumentChangeError::LineOutOfBounds {
            line: position.line,
        });
    }

    let line = text.line(line_index);
    let content_chars = line_content_len_chars(line);
    let target = usize::try_from(position.character).map_err(|_| {
        DocumentChangeError::CharacterOutOfBounds {
            line: position.line,
            character: position.character,
        }
    })?;
    let mut utf16_offset = 0usize;
    for (char_offset, character) in line.chars().take(content_chars).enumerate() {
        if utf16_offset == target {
            return Ok(text.line_to_char(line_index) + char_offset);
        }
        let next_offset = utf16_offset + character.len_utf16();
        if target < next_offset {
            return Err(DocumentChangeError::CharacterSplitsSurrogate {
                line: position.line,
                character: position.character,
            });
        }
        utf16_offset = next_offset;
    }
    if utf16_offset == target {
        Ok(text.line_to_char(line_index) + content_chars)
    } else {
        Err(DocumentChangeError::CharacterOutOfBounds {
            line: position.line,
            character: position.character,
        })
    }
}

fn line_content_len_chars(line: RopeSlice<'_>) -> usize {
    let length = line.len_chars();
    if length == 0 {
        return 0;
    }
    match line.char(length - 1) {
        '\n' if length >= 2 && line.char(length - 2) == '\r' => length - 2,
        '\n' | '\r' => length - 1,
        _ => length,
    }
}

fn utf16_len(text: RopeSlice<'_>) -> Result<u32, DocumentChangeError> {
    text.chars().try_fold(0u32, |length, character| {
        length
            .checked_add(character.len_utf16() as u32)
            .ok_or(DocumentChangeError::RangeTooLarge)
    })
}

fn uri_to_path(uri: &Url) -> PathBuf {
    uri.to_file_path()
        .unwrap_or_else(|_| PathBuf::from(uri.as_str()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tower_lsp::lsp_types::{Position, Range, TextDocumentContentChangeEvent, Url};

    use super::{DocumentChangeError, change_document, close_document, open_document};
    use crate::state::ServerState;

    fn uri(name: &str) -> Url {
        Url::from_file_path(PathBuf::from("/document-tests").join(name))
            .expect("test URI should be valid")
    }

    fn ranged(
        start: (u32, u32),
        end: (u32, u32),
        range_length: Option<u32>,
        text: &str,
    ) -> TextDocumentContentChangeEvent {
        TextDocumentContentChangeEvent {
            range: Some(Range {
                start: Position {
                    line: start.0,
                    character: start.1,
                },
                end: Position {
                    line: end.0,
                    character: end.1,
                },
            }),
            range_length,
            text: text.to_owned(),
        }
    }

    #[test]
    fn applies_utf16_changes_sequentially_and_preserves_crlf_lines() {
        let state = ServerState::new();
        let uri = uri("unicode.aivi");
        open_document(&state, &uri, 7, "a😀b\r\nsecond\rthird".to_owned());

        let snapshot = change_document(
            &state,
            &uri,
            8,
            &[
                ranged((0, 1), (0, 3), Some(2), "é"),
                ranged((0, 2), (0, 3), Some(1), "!"),
                ranged((2, 0), (2, 5), Some(5), "third-line"),
            ],
        )
        .expect("valid UTF-16 changes should apply");

        assert_eq!(snapshot.version, 8);
        assert_eq!(snapshot.text.to_string(), "aé!\r\nsecond\rthird-line");
        assert_eq!(snapshot.file.text(&state.db), snapshot.text.to_string());
    }

    #[test]
    fn rejects_positions_inside_surrogate_pairs() {
        let state = ServerState::new();
        let uri = uri("surrogate.aivi");
        open_document(&state, &uri, 1, "a😀b".to_owned());

        let error = change_document(&state, &uri, 2, &[ranged((0, 2), (0, 3), None, "")])
            .expect_err("a UTF-16 position inside an emoji must be rejected");

        assert_eq!(
            error,
            DocumentChangeError::CharacterSplitsSurrogate {
                line: 0,
                character: 2
            }
        );
        assert_eq!(state.document_snapshot(&uri).unwrap().version, 1);
        assert_eq!(
            state.document_snapshot(&uri).unwrap().text.to_string(),
            "a😀b"
        );
    }

    #[test]
    fn invalid_later_edit_rolls_back_the_entire_batch() {
        let state = ServerState::new();
        let uri = uri("transaction.aivi");
        open_document(&state, &uri, 10, "alpha\nbeta\n".to_owned());

        let error = change_document(
            &state,
            &uri,
            11,
            &[
                ranged((0, 0), (0, 5), Some(5), "changed"),
                ranged((1, 0), (1, 4), Some(99), "invalid"),
            ],
        )
        .expect_err("an invalid later edit must reject the whole notification");

        assert_eq!(
            error,
            DocumentChangeError::RangeLengthMismatch {
                expected: 4,
                received: 99
            }
        );
        let snapshot = state.document_snapshot(&uri).unwrap();
        assert_eq!(snapshot.version, 10);
        assert_eq!(snapshot.text.to_string(), "alpha\nbeta\n");
        assert_eq!(snapshot.file.text(&state.db), "alpha\nbeta\n");
    }

    #[test]
    fn rejects_stale_versions_and_changes_to_closed_documents() {
        let state = ServerState::new();
        let uri = uri("versions.aivi");
        open_document(&state, &uri, 4, "old".to_owned());

        let stale = change_document(
            &state,
            &uri,
            4,
            &[TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "stale".to_owned(),
            }],
        )
        .expect_err("equal versions must not overwrite the document");
        assert_eq!(
            stale,
            DocumentChangeError::StaleVersion {
                current: 4,
                received: 4
            }
        );

        close_document(&state, &uri).expect("document should close");
        let closed = change_document(
            &state,
            &uri,
            5,
            &[TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "resurrected".to_owned(),
            }],
        )
        .expect_err("didChange must not implicitly reopen a document");
        assert_eq!(closed, DocumentChangeError::NotOpen);
    }

    #[test]
    fn full_replacement_can_precede_incremental_change_in_one_batch() {
        let state = ServerState::new();
        let uri = uri("mixed.aivi");
        open_document(&state, &uri, 1, "discarded".to_owned());

        let snapshot = change_document(
            &state,
            &uri,
            2,
            &[
                TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "new 😀 text".to_owned(),
                },
                ranged((0, 4), (0, 6), Some(2), "compact"),
            ],
        )
        .expect("later ranges should address the replacement text");

        assert_eq!(snapshot.text.to_string(), "new compact text");
    }
}
