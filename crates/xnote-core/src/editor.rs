use ropey::Rope;
use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditTransaction {
    pub range: Range<usize>,
    pub replacement: String,
}

impl EditTransaction {
    pub fn insert(offset: usize, text: impl Into<String>) -> Self {
        Self {
            range: offset..offset,
            replacement: text.into(),
        }
    }

    pub fn delete(range: Range<usize>) -> Self {
        Self {
            range,
            replacement: String::new(),
        }
    }

    pub fn replace(range: Range<usize>, replacement: impl Into<String>) -> Self {
        Self {
            range,
            replacement: replacement.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditRecord {
    pub before: EditTransaction,
    pub after: EditTransaction,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditorStats {
    pub chars: usize,
    pub lines: usize,
    pub words: usize,
}

#[derive(Clone, Debug)]
pub struct EditorBuffer {
    rope: Rope,
    version: u64,
    undo_stack: Vec<EditRecord>,
    redo_stack: Vec<EditRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorError {
    InvalidUtf8Boundary,
    OutOfBounds,
}

impl EditorBuffer {
    pub fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            version: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    pub fn is_empty(&self) -> bool {
        self.len_bytes() == 0
    }

    pub fn stats(&self) -> EditorStats {
        let text = self.rope.to_string();
        EditorStats {
            chars: text.chars().count(),
            lines: self.rope.len_lines().max(1),
            words: text.split_whitespace().filter(|w| !w.is_empty()).count(),
        }
    }

    pub fn apply(&mut self, tx: EditTransaction) -> Result<EditRecord, EditorError> {
        let normalized_range = self.validate_range(tx.range.clone())?;
        let removed = self.slice_string(normalized_range.clone())?;

        self.remove_range(normalized_range.clone())?;
        self.insert_string(normalized_range.start, &tx.replacement)?;

        let after_end = normalized_range.start + tx.replacement.len();
        let before = EditTransaction::replace(normalized_range.start..after_end, removed);
        let after = EditTransaction::replace(normalized_range, tx.replacement);
        let record = EditRecord {
            before,
            after,
        };

        self.undo_stack.push(record.clone());
        self.redo_stack.clear();
        self.version = self.version.wrapping_add(1);
        Ok(record)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> Result<Option<EditRecord>, EditorError> {
        let Some(record) = self.undo_stack.pop() else {
            return Ok(None);
        };

        self.apply_without_history(record.before.clone())?;
        self.redo_stack.push(record.clone());
        self.version = self.version.wrapping_add(1);
        Ok(Some(record))
    }

    pub fn redo(&mut self) -> Result<Option<EditRecord>, EditorError> {
        let Some(record) = self.redo_stack.pop() else {
            return Ok(None);
        };

        self.apply_without_history(record.after.clone())?;
        self.undo_stack.push(record.clone());
        self.version = self.version.wrapping_add(1);
        Ok(Some(record))
    }

    pub fn replace_all(&mut self, text: &str) {
        self.rope = Rope::from_str(text);
        self.version = self.version.wrapping_add(1);
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    fn apply_without_history(&mut self, tx: EditTransaction) -> Result<(), EditorError> {
        let normalized_range = self.validate_range(tx.range)?;
        self.remove_range(normalized_range.clone())?;
        self.insert_string(normalized_range.start, &tx.replacement)?;
        Ok(())
    }

    fn validate_range(&self, range: Range<usize>) -> Result<Range<usize>, EditorError> {
        if range.start > range.end || range.end > self.len_bytes() {
            return Err(EditorError::OutOfBounds);
        }
        if !self.is_char_boundary(range.start) || !self.is_char_boundary(range.end) {
            return Err(EditorError::InvalidUtf8Boundary);
        }
        Ok(range)
    }

    fn is_char_boundary(&self, offset: usize) -> bool {
        if offset == self.len_bytes() {
            return true;
        }
        let Ok(char_idx) = self.rope.try_byte_to_char(offset) else {
            return false;
        };
        self.rope
            .try_char_to_byte(char_idx)
            .is_ok_and(|byte_idx| byte_idx == offset)
    }

    fn slice_string(&self, range: Range<usize>) -> Result<String, EditorError> {
        let start_char = self
            .rope
            .try_byte_to_char(range.start)
            .map_err(|_| EditorError::InvalidUtf8Boundary)?;
        let end_char = self
            .rope
            .try_byte_to_char(range.end)
            .map_err(|_| EditorError::InvalidUtf8Boundary)?;
        Ok(self.rope.slice(start_char..end_char).to_string())
    }

    fn remove_range(&mut self, range: Range<usize>) -> Result<(), EditorError> {
        let start_char = self
            .rope
            .try_byte_to_char(range.start)
            .map_err(|_| EditorError::InvalidUtf8Boundary)?;
        let end_char = self
            .rope
            .try_byte_to_char(range.end)
            .map_err(|_| EditorError::InvalidUtf8Boundary)?;
        self.rope.remove(start_char..end_char);
        Ok(())
    }

    fn insert_string(&mut self, byte_offset: usize, text: &str) -> Result<(), EditorError> {
        let char_offset = self
            .rope
            .try_byte_to_char(byte_offset)
            .map_err(|_| EditorError::InvalidUtf8Boundary)?;
        self.rope.insert(char_offset, text);
        Ok(())
    }
}

impl std::fmt::Display for EditorBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.rope.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_insert_delete_replace_updates_buffer_and_version() {
        let mut buffer = EditorBuffer::new("abc");
        assert_eq!(buffer.version(), 0);

        buffer
            .apply(EditTransaction::insert(3, "d"))
            .expect("insert succeeds");
        assert_eq!(buffer.to_string(), "abcd");
        assert_eq!(buffer.version(), 1);

        buffer
            .apply(EditTransaction::delete(1..3))
            .expect("delete succeeds");
        assert_eq!(buffer.to_string(), "ad");
        assert_eq!(buffer.version(), 2);

        buffer
            .apply(EditTransaction::replace(0..2, "xyz"))
            .expect("replace succeeds");
        assert_eq!(buffer.to_string(), "xyz");
        assert_eq!(buffer.version(), 3);
    }

    #[test]
    fn undo_and_redo_restore_content() {
        let mut buffer = EditorBuffer::new("hello");
        buffer
            .apply(EditTransaction::replace(0..5, "world"))
            .expect("edit succeeds");
        assert_eq!(buffer.to_string(), "world");

        buffer.undo().expect("undo works");
        assert_eq!(buffer.to_string(), "hello");

        buffer.redo().expect("redo works");
        assert_eq!(buffer.to_string(), "world");
    }

    #[test]
    fn undo_and_redo_restore_variable_length_edits() {
        let mut buffer = EditorBuffer::new("abc");
        buffer
            .apply(EditTransaction::insert(3, "-tail"))
            .expect("insert succeeds");
        assert_eq!(buffer.to_string(), "abc-tail");

        buffer.undo().expect("undo succeeds");
        assert_eq!(buffer.to_string(), "abc");

        buffer.redo().expect("redo succeeds");
        assert_eq!(buffer.to_string(), "abc-tail");

        buffer
            .apply(EditTransaction::delete(1..8))
            .expect("delete succeeds");
        assert_eq!(buffer.to_string(), "a");

        buffer.undo().expect("undo delete succeeds");
        assert_eq!(buffer.to_string(), "abc-tail");
    }

    #[test]
    fn stats_match_expected_values() {
        let buffer = EditorBuffer::new("# Title\n\nhello world\n");
        let stats = buffer.stats();
        assert_eq!(stats.words, 4);
        assert!(stats.lines >= 3);
    }

    #[test]
    fn invalid_range_is_rejected() {
        let mut buffer = EditorBuffer::new("你好");
        let err = buffer
            .apply(EditTransaction::replace(1..2, "x"))
            .expect_err("mid-byte range should fail");
        assert_eq!(err, EditorError::InvalidUtf8Boundary);
    }
}
