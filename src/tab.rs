use std::fs;
use std::io;
use std::path::PathBuf;
use iced::widget::text_editor::{self, Cursor, Position};

use crate::highlighter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FileEncoding {
    Utf8 { bom: bool },
    Utf16Le { bom: bool },
    Utf16Be { bom: bool },
    Windows1252,
}

impl FileEncoding {
    pub fn label(&self) -> &'static str {
        match self {
            FileEncoding::Utf8 { bom: false } => "UTF-8",
            FileEncoding::Utf8 { bom: true } => "UTF-8 BOM",
            FileEncoding::Utf16Le { .. } => "UTF-16 LE",
            FileEncoding::Utf16Be { .. } => "UTF-16 BE",
            FileEncoding::Windows1252 => "Windows-1252",
        }
    }
}

pub fn read_file_with_encoding(path: &std::path::Path) -> io::Result<(String, FileEncoding)> {
    let bytes = fs::read(path)?;
    if bytes.is_empty() {
        return Ok((String::new(), FileEncoding::Utf8 { bom: false }));
    }

    // 1. Check for BOMs
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (cow, _, _) = encoding_rs::UTF_8.decode(&bytes[3..]);
        let text = cow.trim_start_matches('\u{FEFF}').to_string();
        return Ok((text, FileEncoding::Utf8 { bom: true }));
    }

    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (cow, _, _) = encoding_rs::UTF_16LE.decode(&bytes[2..]);
        let text = cow.trim_start_matches('\u{FEFF}').to_string();
        return Ok((text, FileEncoding::Utf16Le { bom: true }));
    }

    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (cow, _, _) = encoding_rs::UTF_16BE.decode(&bytes[2..]);
        let text = cow.trim_start_matches('\u{FEFF}').to_string();
        return Ok((text, FileEncoding::Utf16Be { bom: true }));
    }

    // 2. Try UTF-8 without BOM
    if let Ok(text) = std::str::from_utf8(&bytes) {
        return Ok((text.to_string(), FileEncoding::Utf8 { bom: false }));
    }

    // 3. Heuristic for UTF-16 without BOM (detect null-byte pattern)
    if bytes.len() >= 4 {
        let nulls_even = bytes.iter().step_by(2).filter(|&&b| b == 0).count();
        let nulls_odd = bytes.iter().skip(1).step_by(2).filter(|&&b| b == 0).count();
        let half = bytes.len() / 2;

        if nulls_odd > half / 3 && nulls_even < half / 10 {
            let (cow, had_errors) = encoding_rs::UTF_16LE.decode_without_bom_handling(&bytes);
            if !had_errors {
                return Ok((cow.trim_start_matches('\u{FEFF}').to_string(), FileEncoding::Utf16Le { bom: false }));
            }
        } else if nulls_even > half / 3 && nulls_odd < half / 10 {
            let (cow, had_errors) = encoding_rs::UTF_16BE.decode_without_bom_handling(&bytes);
            if !had_errors {
                return Ok((cow.trim_start_matches('\u{FEFF}').to_string(), FileEncoding::Utf16Be { bom: false }));
            }
        }
    }

    // 4. Try Windows-1252 fallback
    let (cow, _, had_errors) = encoding_rs::WINDOWS_1252.decode(&bytes);
    if !had_errors {
        return Ok((cow.to_string(), FileEncoding::Windows1252));
    }

    // 5. Ultimate fallback: lossy UTF-8
    let text = String::from_utf8_lossy(&bytes).to_string();
    Ok((text, FileEncoding::Utf8 { bom: false }))
}

pub fn write_file_with_encoding(path: &std::path::Path, text: &str, encoding: FileEncoding) -> io::Result<()> {
    match encoding {
        FileEncoding::Utf8 { bom: false } => {
            fs::write(path, text.as_bytes())?;
        }
        FileEncoding::Utf8 { bom: true } => {
            let mut bytes = vec![0xEF, 0xBB, 0xBF];
            bytes.extend_from_slice(text.as_bytes());
            fs::write(path, &bytes)?;
        }
        FileEncoding::Utf16Le { bom } => {
            let mut bytes = Vec::with_capacity(if bom { 2 } else { 0 } + text.len() * 2);
            if bom {
                bytes.extend_from_slice(&[0xFF, 0xFE]);
            }
            for u in text.encode_utf16() {
                bytes.extend_from_slice(&u.to_le_bytes());
            }
            fs::write(path, &bytes)?;
        }
        FileEncoding::Utf16Be { bom } => {
            let mut bytes = Vec::with_capacity(if bom { 2 } else { 0 } + text.len() * 2);
            if bom {
                bytes.extend_from_slice(&[0xFE, 0xFF]);
            }
            for u in text.encode_utf16() {
                bytes.extend_from_slice(&u.to_be_bytes());
            }
            fs::write(path, &bytes)?;
        }
        FileEncoding::Windows1252 => {
            let (cow, _, had_errors) = encoding_rs::WINDOWS_1252.encode(text);
            if had_errors {
                fs::write(path, text.as_bytes())?;
            } else {
                fs::write(path, &cow)?;
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
pub struct Tab {
    pub id: String,
    pub path: Option<PathBuf>,
    pub title: String,
    pub content: text_editor::Content,
    pub is_dirty: bool,
    pub saved_text: String,
    pub manual_syntax: Option<String>,
    pub encoding: FileEncoding,
}

impl Tab {
    pub fn new_empty(counter: usize) -> Self {
        let id = format!(
            "tab_{}_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            counter
        );
        let title = if counter == 0 {
            "Untitled".to_string()
        } else {
            format!("Untitled {}", counter + 1)
        };

        Self {
            id,
            path: None,
            title,
            content: text_editor::Content::new(),
            is_dirty: false,
            saved_text: String::new(),
            manual_syntax: None,
            encoding: FileEncoding::Utf8 { bom: false },
        }
    }

    pub fn from_file(path: PathBuf) -> Result<Self, io::Error> {
        let (text, encoding) = read_file_with_encoding(&path)?;
        let title = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string();
        let id = format!("tab_{}", path.display().to_string().replace('/', "_"));

        Ok(Self {
            id,
            path: Some(path),
            title,
            content: text_editor::Content::with_text(&text),
            is_dirty: false,
            saved_text: text,
            manual_syntax: None,
            encoding,
        })
    }

    pub fn from_draft(
        id: String,
        path: Option<PathBuf>,
        title: String,
        text: String,
        is_dirty: bool,
        cursor_line: usize,
        cursor_col: usize,
        encoding: FileEncoding,
    ) -> Self {
        let mut content = text_editor::Content::with_text(&text);
        let position = Position {
            line: cursor_line,
            column: cursor_col,
        };
        content.move_to(Cursor {
            position,
            selection: None,
        });

        let saved_text = if !is_dirty && path.is_some() {
            text.clone()
        } else {
            String::new()
        };

        Self {
            id,
            path,
            title,
            content,
            is_dirty,
            saved_text,
            manual_syntax: None,
            encoding,
        }
    }

    pub fn save(&mut self) -> Result<(), io::Error> {
        if let Some(path) = &self.path {
            let current_text = self.content.text();
            write_file_with_encoding(path, &current_text, self.encoding)?;
            self.saved_text = current_text;
            self.is_dirty = false;
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "No file path set"))
        }
    }

    pub fn save_as(&mut self, new_path: PathBuf) -> Result<(), io::Error> {
        let current_text = self.content.text();
        write_file_with_encoding(&new_path, &current_text, self.encoding)?;
        self.title = new_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string();
        self.path = Some(new_path);
        self.saved_text = current_text;
        self.is_dirty = false;
        Ok(())
    }

    pub fn on_edit(&mut self, action: text_editor::Action) {
        match action {
            text_editor::Action::Edit(text_editor::Edit::Enter) => {
                self.handle_enter();
            }
            other => {
                let is_edit = other.is_edit();
                self.content.perform(other);
                if is_edit {
                    self.is_dirty = true;
                }
            }
        }
    }

    pub fn handle_enter(&mut self) {
        let cursor = self.content.cursor();
        let current_line_idx = cursor.position.line;
        let current_col = cursor.position.column;

        let line_text = self
            .content
            .line(current_line_idx)
            .map(|l| l.text.to_string())
            .unwrap_or_default();

        let col = current_col.min(line_text.len());
        let safe_col = if line_text.is_char_boundary(col) {
            col
        } else {
            (0..col).rev().find(|&i| line_text.is_char_boundary(i)).unwrap_or(0)
        };

        let text_before = &line_text[..safe_col];
        let text_after = &line_text[safe_col..];

        // 1. Calculate base indentation
        let base_indent: String = line_text
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();

        // 2. Extra indent step (4 spaces, or tab if file uses tabs)
        let indent_step = if base_indent.contains('\t') {
            "\t"
        } else {
            "    "
        };

        let trimmed_before = text_before.trim_end();
        let trimmed_after = text_after.trim_start();

        let opens_block = trimmed_before.ends_with('{')
            || trimmed_before.ends_with('(')
            || trimmed_before.ends_with('[')
            || trimmed_before.ends_with(':');

        let is_between_delimiters = (trimmed_before.ends_with('{') && trimmed_after.starts_with('}'))
            || (trimmed_before.ends_with('(') && trimmed_after.starts_with(')'))
            || (trimmed_before.ends_with('[') && trimmed_after.starts_with(']'));

        if is_between_delimiters {
            // Split between delimiters into 3 lines:
            // Line 1: text_before (e.g. "fn foo() {")
            // Line 2: inner indent (e.g. "    ") <- cursor here
            // Line 3: base_indent + text_after (e.g. "}")
            let inner_indent = format!("{}{}", base_indent, indent_step);
            let insertion = format!("\n{}\n{}", inner_indent, base_indent);

            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                std::sync::Arc::new(insertion),
            )));
            self.content.perform(text_editor::Action::Move(text_editor::Motion::Up));
            self.content.perform(text_editor::Action::Move(text_editor::Motion::End));
        } else if opens_block {
            let new_indent = format!("{}{}", base_indent, indent_step);
            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Enter));
            if !new_indent.is_empty() {
                self.content.perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                    std::sync::Arc::new(new_indent),
                )));
            }
        } else {
            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Enter));
            if !base_indent.is_empty() {
                self.content.perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                    std::sync::Arc::new(base_indent),
                )));
            }
        }

        self.is_dirty = true;
    }

    /// Handles Tab key: indents selection or inserts spaces up to the next 4-space tab stop.
    pub fn indent(&mut self) {
        let cursor = self.content.cursor();
        if cursor.selection.is_some() {
            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Indent));
        } else {
            let col = cursor.position.column;
            let spaces_needed = 4 - (col % 4);
            let spaces_str = " ".repeat(spaces_needed);
            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                std::sync::Arc::new(spaces_str),
            )));
        }
        self.is_dirty = true;
    }

    /// Handles Shift+Tab key: unindents current line or selection.
    pub fn unindent(&mut self) {
        self.content.perform(text_editor::Action::Edit(text_editor::Edit::Unindent));
        self.is_dirty = true;
    }

    /// Smart backspace: if preceding 4 characters are spaces in indentation, deletes all 4 at once.
    pub fn smart_backspace(&mut self) {
        let cursor = self.content.cursor();
        if cursor.selection.is_some() {
            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Backspace));
            self.is_dirty = true;
            return;
        }

        let line_text = self
            .content
            .line(cursor.position.line)
            .map(|l| l.text.to_string())
            .unwrap_or_default();

        let col = cursor.position.column.min(line_text.len());
        let before_cursor = &line_text[..col];

        // If cursor is in leading whitespace and at a tab stop (multiple of 4):
        if before_cursor.chars().all(|c| c == ' ') && col >= 4 && col % 4 == 0 {
            for _ in 0..4 {
                self.content.perform(text_editor::Action::Edit(text_editor::Edit::Backspace));
            }
        } else {
            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Backspace));
        }
        self.is_dirty = true;
    }

    pub fn cursor_position(&self) -> (usize, usize) {
        let cursor = self.content.cursor();
        (cursor.position.line, cursor.position.column)
    }

    pub fn line_count(&self) -> usize {
        self.content.line_count()
    }

    pub fn syntax_token(&self) -> String {
        if let Some(manual) = &self.manual_syntax {
            return manual.clone();
        }

        let first_line = self
            .content
            .line(0)
            .map(|l| l.text.to_string())
            .unwrap_or_default();
        let (token, _) = highlighter::detect_syntax_and_name(self.path.as_deref(), &first_line);
        token
    }

    pub fn language_label(&self) -> String {
        if let Some(manual) = &self.manual_syntax {
            let syntax = highlighter::find_syntax(manual);
            return syntax.name.clone();
        }

        let first_line = self
            .content
            .line(0)
            .map(|l| l.text.to_string())
            .unwrap_or_default();
        let (_, name) = highlighter::detect_syntax_and_name(self.path.as_deref(), &first_line);
        name
    }

    pub fn set_syntax(&mut self, token: Option<String>) {
        self.manual_syntax = token;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_indent_normal_line() {
        let mut tab = Tab::from_draft(
            "t1".into(),
            None,
            "test".into(),
            "    let x = 10;".into(),
            false,
            0,
            15,
            FileEncoding::Utf8 { bom: false },
        );
        tab.on_edit(text_editor::Action::Edit(text_editor::Edit::Enter));
        assert_eq!(tab.content.line_count(), 2);
        assert_eq!(tab.content.line(0).unwrap().text, "    let x = 10;");
        assert_eq!(tab.content.line(1).unwrap().text, "    ");
        assert_eq!(tab.cursor_position(), (1, 4));
    }

    #[test]
    fn test_auto_indent_open_brace() {
        let mut tab = Tab::from_draft(
            "t2".into(),
            None,
            "test".into(),
            "    if true {".into(),
            false,
            0,
            13,
            FileEncoding::Utf8 { bom: false },
        );
        tab.on_edit(text_editor::Action::Edit(text_editor::Edit::Enter));
        assert_eq!(tab.content.line_count(), 2);
        assert_eq!(tab.content.line(0).unwrap().text, "    if true {");
        assert_eq!(tab.content.line(1).unwrap().text, "        ");
        assert_eq!(tab.cursor_position(), (1, 8));
    }

    #[test]
    fn test_auto_indent_between_braces() {
        let mut tab = Tab::from_draft(
            "t3".into(),
            None,
            "test".into(),
            "    fn test() {}".into(),
            false,
            0,
            15, // between { and }
            FileEncoding::Utf8 { bom: false },
        );
        tab.on_edit(text_editor::Action::Edit(text_editor::Edit::Enter));
        assert_eq!(tab.content.line_count(), 3);
        assert_eq!(tab.content.line(0).unwrap().text, "    fn test() {");
        assert_eq!(tab.content.line(1).unwrap().text, "        ");
        assert_eq!(tab.content.line(2).unwrap().text, "    }");
        assert_eq!(tab.cursor_position(), (1, 8));
    }

    #[test]
    fn test_tab_indent_and_unindent() {
        let mut tab = Tab::new_empty(0);
        tab.indent();
        assert_eq!(tab.content.text(), "    ");
        assert_eq!(tab.cursor_position(), (0, 4));

        tab.unindent();
        assert_eq!(tab.content.text(), "");
        assert_eq!(tab.cursor_position(), (0, 0));
    }

    #[test]
    fn test_smart_backspace() {
        let mut tab = Tab::from_draft(
            "t4".into(),
            None,
            "test".into(),
            "    ".into(),
            false,
            0,
            4,
            FileEncoding::Utf8 { bom: false },
        );
        tab.smart_backspace();
        assert_eq!(tab.content.text(), "");
        assert_eq!(tab.cursor_position(), (0, 0));
    }

    #[test]
    fn test_utf16le_read_write() {
        let dir = std::env::temp_dir();
        let test_file = dir.join("zero_test_utf16.txt");
        let sample = "Ciao mondo in UTF-16\nRiga 2!";
        write_file_with_encoding(&test_file, sample, FileEncoding::Utf16Le { bom: true }).unwrap();

        let (read_text, enc) = read_file_with_encoding(&test_file).unwrap();
        assert_eq!(enc, FileEncoding::Utf16Le { bom: true });
        assert_eq!(read_text, sample);

        let _ = fs::remove_file(test_file);
    }
}
