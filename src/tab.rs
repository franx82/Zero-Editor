use std::fs;
use std::io;
use std::path::PathBuf;
use iced::widget::text_editor::{self, Cursor, Position};

use crate::highlighter;

#[derive(Debug)]
pub struct Tab {
    pub id: String,
    pub path: Option<PathBuf>,
    pub title: String,
    pub content: text_editor::Content,
    pub is_dirty: bool,
    pub saved_text: String,
    pub manual_syntax: Option<String>,
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
        }
    }

    pub fn from_file(path: PathBuf) -> Result<Self, io::Error> {
        let text = fs::read_to_string(&path)?;
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
        }
    }

    pub fn save(&mut self) -> Result<(), io::Error> {
        if let Some(path) = &self.path {
            let current_text = self.content.text();
            fs::write(path, &current_text)?;
            self.saved_text = current_text;
            self.is_dirty = false;
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "No file path set"))
        }
    }

    pub fn save_as(&mut self, new_path: PathBuf) -> Result<(), io::Error> {
        let current_text = self.content.text();
        fs::write(&new_path, &current_text)?;
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
        );
        tab.smart_backspace();
        assert_eq!(tab.content.text(), "");
        assert_eq!(tab.cursor_position(), (0, 0));
    }
}
