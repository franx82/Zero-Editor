use iced::widget::text_editor::{self, Cursor, Position};
use std::fs;
use std::io;
use std::path::PathBuf;

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

fn reject_decode_errors(errors: bool) -> io::Result<()> {
    if errors {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "The file contains invalid Unicode sequences",
        ))
    } else {
        Ok(())
    }
}

pub fn read_file_with_encoding(path: &std::path::Path) -> io::Result<(String, FileEncoding)> {
    let bytes = fs::read(path)?;
    if bytes.is_empty() {
        return Ok((String::new(), FileEncoding::Utf8 { bom: false }));
    }

    // 1. Check for BOMs
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (cow, _, errors) = encoding_rs::UTF_8.decode(&bytes[3..]);
        reject_decode_errors(errors)?;
        let text = cow.into_owned();
        return Ok((text, FileEncoding::Utf8 { bom: true }));
    }

    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (cow, _, errors) = encoding_rs::UTF_16LE.decode(&bytes[2..]);
        reject_decode_errors(errors)?;
        let text = cow.into_owned();
        return Ok((text, FileEncoding::Utf16Le { bom: true }));
    }

    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (cow, _, errors) = encoding_rs::UTF_16BE.decode(&bytes[2..]);
        reject_decode_errors(errors)?;
        let text = cow.into_owned();
        return Ok((text, FileEncoding::Utf16Be { bom: true }));
    }

    // 3. Heuristic for UTF-16 without BOM (detect null-byte pattern)
    if bytes.len() >= 4 && bytes.len() % 2 == 0 {
        let nulls_even = bytes.iter().step_by(2).filter(|&&b| b == 0).count();
        let nulls_odd = bytes.iter().skip(1).step_by(2).filter(|&&b| b == 0).count();
        let half = bytes.len() / 2;

        if nulls_odd > half / 3 && nulls_even * 10 < half {
            let (cow, had_errors) = encoding_rs::UTF_16LE.decode_without_bom_handling(&bytes);
            if !had_errors {
                return Ok((cow.into_owned(), FileEncoding::Utf16Le { bom: false }));
            }
        } else if nulls_even > half / 3 && nulls_odd * 10 < half {
            let (cow, had_errors) = encoding_rs::UTF_16BE.decode_without_bom_handling(&bytes);
            if !had_errors {
                return Ok((cow.into_owned(), FileEncoding::Utf16Be { bom: false }));
            }
        }
    }

    // 2. Try UTF-8 without BOM
    if let Ok(text) = std::str::from_utf8(&bytes) {
        return Ok((text.to_string(), FileEncoding::Utf8 { bom: false }));
    }

    // 4. Try Windows-1252 fallback
    let (cow, _, had_errors) = encoding_rs::WINDOWS_1252.decode(&bytes);
    if !had_errors {
        return Ok((cow.to_string(), FileEncoding::Windows1252));
    }

    // 5. Ultimate fallback: lossy UTF-8
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Unrecognized encoding: opening canceled to preserve the data",
    ))
}

pub fn write_file_with_encoding(
    path: &std::path::Path,
    text: &str,
    encoding: FileEncoding,
) -> io::Result<()> {
    match encoding {
        FileEncoding::Utf8 { bom: false } => {
            crate::storage::atomic_write(path, text.as_bytes())?;
        }
        FileEncoding::Utf8 { bom: true } => {
            let mut bytes = vec![0xEF, 0xBB, 0xBF];
            bytes.extend_from_slice(text.as_bytes());
            crate::storage::atomic_write(path, &bytes)?;
        }
        FileEncoding::Utf16Le { bom } => {
            let mut bytes = Vec::with_capacity((if bom { 2 } else { 0 }) + text.len() * 2);
            if bom {
                bytes.extend_from_slice(&[0xFF, 0xFE]);
            }
            for u in text.encode_utf16() {
                bytes.extend_from_slice(&u.to_le_bytes());
            }
            crate::storage::atomic_write(path, &bytes)?;
        }
        FileEncoding::Utf16Be { bom } => {
            let mut bytes = Vec::with_capacity((if bom { 2 } else { 0 }) + text.len() * 2);
            if bom {
                bytes.extend_from_slice(&[0xFE, 0xFF]);
            }
            for u in text.encode_utf16() {
                bytes.extend_from_slice(&u.to_be_bytes());
            }
            crate::storage::atomic_write(path, &bytes)?;
        }
        FileEncoding::Windows1252 => {
            let (cow, _, had_errors) = encoding_rs::WINDOWS_1252.encode(text);
            if had_errors {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Some characters cannot be represented in Windows-1252. Convert the document to UTF-8 from the command palette.",
                ));
            } else {
                crate::storage::atomic_write(path, &cow)?;
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct Snapshot {
    text: String,
    cursor: Cursor,
}

fn clamp_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

#[derive(Debug)]
pub struct Tab {
    pub id: String,
    pub path: Option<PathBuf>,
    pub title: String,
    pub content: text_editor::Content,
    pub is_dirty: bool,
    pub saved_text: String,
    saved_encoding: FileEncoding,
    pub manual_syntax: Option<String>,
    pub encoding: FileEncoding,
    pub scroll_line: usize,
    pub visible_lines: usize,
    pub scroll_offset: f32,
    pub horizontal_offset: f32,
    viewport_height: f32,
    viewport_initialized: bool,
    line_height: f32,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl Tab {
    pub fn new_empty(counter: usize) -> Self {
        let id = crate::storage::unique_id();
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
            saved_encoding: FileEncoding::Utf8 { bom: false },
            manual_syntax: None,
            encoding: FileEncoding::Utf8 { bom: false },
            scroll_line: 0,
            visible_lines: 32,
            scroll_offset: 0.0,
            horizontal_offset: 0.0,
            viewport_height: 672.0,
            viewport_initialized: false,
            line_height: 21.0,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn from_file(path: PathBuf) -> Result<Self, io::Error> {
        let (text, encoding) = read_file_with_encoding(&path)?;
        let title = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string();
        let id = crate::storage::unique_id();

        Ok(Self {
            id,
            path: Some(path),
            title,
            content: text_editor::Content::with_text(&text),
            is_dirty: false,
            saved_text: text,
            saved_encoding: encoding,
            manual_syntax: None,
            encoding,
            scroll_line: 0,
            visible_lines: 32,
            scroll_offset: 0.0,
            horizontal_offset: 0.0,
            viewport_height: 672.0,
            viewport_initialized: false,
            line_height: 21.0,
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    pub fn from_draft(
        id: String,
        path: Option<PathBuf>,
        title: String,
        text: String,
        is_dirty: bool,
        (cursor_line, cursor_col): (usize, usize),
        encoding: FileEncoding,
    ) -> Self {
        let mut content = text_editor::Content::with_text(&text);
        let line = cursor_line.min(content.line_count().saturating_sub(1));
        let line_text = content
            .line(line)
            .map(|l| l.text.into_owned())
            .unwrap_or_default();
        let position = Position {
            line,
            column: clamp_boundary(&line_text, cursor_col),
        };
        content.move_to(Cursor {
            position,
            selection: None,
        });

        let saved_text = if !is_dirty && path.is_some() {
            text.clone()
        } else {
            path.as_ref()
                .and_then(|p| read_file_with_encoding(p).ok())
                .map(|(t, _)| t)
                .unwrap_or_default()
        };

        Self {
            id,
            path: path.clone(),
            title,
            content,
            is_dirty,
            saved_text,
            saved_encoding: path
                .as_ref()
                .and_then(|p| read_file_with_encoding(p).ok())
                .map(|(_, e)| e)
                .unwrap_or(encoding),
            manual_syntax: None,
            encoding,
            scroll_line: 0,
            visible_lines: 32,
            scroll_offset: 0.0,
            horizontal_offset: 0.0,
            viewport_height: 672.0,
            viewport_initialized: false,
            line_height: 21.0,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn save(&mut self) -> Result<(), io::Error> {
        if let Some(path) = &self.path {
            let current_text = self.content.text();
            write_file_with_encoding(path, &current_text, self.encoding)?;
            self.saved_text = current_text;
            self.saved_encoding = self.encoding;
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
        self.saved_encoding = self.encoding;
        self.is_dirty = false;
        Ok(())
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.content.text(),
            cursor: self.content.cursor(),
        }
    }

    fn checkpoint(&mut self) {
        let snapshot = self.snapshot();
        if self
            .undo
            .last()
            .is_none_or(|last| last.text != snapshot.text)
        {
            self.undo.push(snapshot);
        }
        // Bound snapshot history by count and bytes, preserving at least the latest edit.
        while self.undo.len() > 1
            && (self.undo.len() > 200
                || self.undo.iter().map(|s| s.text.len()).sum::<usize>() > 32 * 1024 * 1024)
        {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.content = text_editor::Content::with_text(&snapshot.text);
        self.content.move_to(snapshot.cursor);
        self.scroll_line = 0;
        self.scroll_offset = 0.0;
        self.sync_scroll_with_cursor();
        self.refresh_dirty();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.restore(previous);
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.restore(next);
        }
    }

    fn refresh_dirty(&mut self) {
        self.is_dirty =
            self.content.text() != self.saved_text || self.encoding != self.saved_encoding;
    }

    pub fn select_range(&mut self, start: usize, end: usize) {
        let text = self.content.text();
        let position = crate::search::position_at(&text, end);
        let selection = if start != end {
            Some(crate::search::position_at(&text, start))
        } else {
            None
        };
        self.content
            .perform(text_editor::Action::Move(text_editor::Motion::Left));
        self.content.move_to(Cursor {
            position,
            selection,
        });
        self.sync_scroll_with_cursor();
    }

    pub fn replace_range(&mut self, start: usize, end: usize, replacement: String) {
        self.checkpoint();
        self.select_range(start, end);
        self.content
            .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                std::sync::Arc::new(replacement),
            )));
        self.refresh_dirty();
    }

    pub fn goto(&mut self, line: usize, column: usize) {
        let text = self.content.text();
        let line = line
            .saturating_sub(1)
            .min(self.line_count().saturating_sub(1));
        let line_text = self
            .content
            .line(line)
            .map(|l| l.text.into_owned())
            .unwrap_or_default();
        let col = line_text
            .char_indices()
            .nth(column.saturating_sub(1))
            .map(|(i, _)| i)
            .unwrap_or(line_text.len());
        let offset = crate::search::offset_at(&text, Position { line, column: col });
        self.select_range(offset, offset);
    }

    fn shifted_position(&self, mut position: Position, lines: isize) -> Position {
        position.line = position
            .line
            .saturating_add_signed(lines)
            .min(self.line_count().saturating_sub(1));
        let line = self
            .content
            .line(position.line)
            .map(|l| l.text.into_owned())
            .unwrap_or_default();
        position.column = clamp_boundary(&line, position.column);
        position
    }

    fn shift_cursor(&mut self, mut cursor: Cursor, lines: isize) {
        cursor.position = self.shifted_position(cursor.position, lines);
        cursor.selection = cursor
            .selection
            .map(|position| self.shifted_position(position, lines));
        self.content
            .perform(text_editor::Action::Move(text_editor::Motion::Left));
        self.content.move_to(cursor);
        self.sync_scroll_with_cursor();
    }

    pub fn line_command(&mut self, command: crate::commands::LineCommand) {
        use crate::commands::LineCommand;
        let text = self.content.text();
        let cursor = self.content.cursor();
        let mut first = cursor.position.line;
        let mut last = first;
        if let Some(anchor) = cursor.selection {
            first = first.min(anchor.line);
            last = last.max(anchor.line);
            let end = if cursor.position.line >= anchor.line {
                cursor.position
            } else {
                anchor
            };
            if end.column == 0 && last > first {
                last -= 1;
            }
        }
        let ranges = crate::search::line_ranges(&text);
        let start = ranges[first].start;
        let end = ranges[last].end;
        let ending = self.content.line_ending().unwrap_or_default().as_str();
        let ending = if ending.is_empty() { "\n" } else { ending };
        match command {
            LineCommand::Duplicate => {
                let block = &text[start..end];
                let insertion = if last + 1 == ranges.len() {
                    format!("{ending}{block}")
                } else {
                    block.to_owned()
                };
                self.replace_range(end, end, insertion);
                self.shift_cursor(cursor, (last - first + 1) as isize);
            }
            LineCommand::Delete => {
                let start = if last + 1 == ranges.len() && first > 0 {
                    start
                        - self
                            .content
                            .line(first - 1)
                            .map(|l| l.ending.as_str().len())
                            .unwrap_or(1)
                } else {
                    start
                };
                self.replace_range(start, end, String::new());
            }
            LineCommand::MoveUp if first > 0 => {
                let above = ranges[first - 1].start;
                let mut a = text[above..start].to_owned();
                let mut b = text[start..end].to_owned();
                if last + 1 == ranges.len() {
                    let separator = self
                        .content
                        .line(first - 1)
                        .map(|l| l.ending.as_str())
                        .unwrap_or(ending);
                    a.truncate(a.len() - separator.len());
                    b.push_str(separator);
                }
                self.replace_range(above, end, format!("{b}{a}"));
                self.shift_cursor(cursor, -1);
            }
            LineCommand::MoveDown if last + 1 < ranges.len() => {
                let below = ranges[last + 1].end;
                let mut a = text[start..end].to_owned();
                let mut b = text[end..below].to_owned();
                if last + 2 == ranges.len() {
                    let separator = self
                        .content
                        .line(last)
                        .map(|l| l.ending.as_str())
                        .unwrap_or(ending);
                    a.truncate(a.len() - separator.len());
                    b.push_str(separator);
                }
                self.replace_range(start, below, format!("{b}{a}"));
                self.shift_cursor(cursor, 1);
            }
            _ => {}
        }
    }

    pub fn set_viewport(&mut self, height: f32, line_height: f32) {
        self.viewport_height = height.max(line_height);
        self.line_height = line_height;
        self.visible_lines = (self.viewport_height / line_height).floor().max(1.0) as usize;
        self.set_scroll_pixels(self.scroll_pixels());
        if !self.viewport_initialized {
            self.sync_scroll_with_cursor();
            self.viewport_initialized = true;
        }
    }

    fn scroll_pixels(&self) -> f32 {
        self.scroll_line as f32 * self.line_height + self.scroll_offset
    }

    fn set_scroll_pixels(&mut self, pixels: f32) {
        let max = (self.line_count() as f32 * self.line_height - self.viewport_height).max(0.0);
        let pixels = pixels.clamp(0.0, max);
        self.scroll_line = (pixels / self.line_height).floor() as usize;
        self.scroll_offset = pixels - self.scroll_line as f32 * self.line_height;
    }

    pub fn scroll_to_line(&mut self, target_line: usize) {
        let max_scroll = self.line_count().saturating_sub(self.visible_lines);
        let target_line = target_line.min(max_scroll);
        let diff = target_line as i32 - self.scroll_line as i32;
        if diff != 0 {
            self.content
                .perform(text_editor::Action::Scroll { lines: diff });
            self.set_scroll_pixels(self.scroll_pixels() + diff as f32 * self.line_height);
        }
    }

    pub fn on_edit(&mut self, action: text_editor::Action) {
        match action {
            text_editor::Action::Edit(text_editor::Edit::Enter) => {
                self.handle_enter();
                self.sync_scroll_with_cursor();
            }
            text_editor::Action::Scroll { lines } => {
                self.set_scroll_pixels(self.scroll_pixels() + lines as f32 * self.line_height);
                self.content.perform(action);
            }
            other => {
                let is_edit = other.is_edit();
                if is_edit {
                    self.checkpoint();
                }
                self.content.perform(other);
                if is_edit {
                    self.refresh_dirty();
                }
                self.sync_scroll_with_cursor();
            }
        }
    }

    fn sync_scroll_with_cursor(&mut self) {
        let top = self.scroll_pixels();
        let cursor_top = self.cursor_position().0 as f32 * self.line_height;
        let cursor_bottom = cursor_top + self.line_height;
        let next = if cursor_top < top {
            cursor_top
        } else if cursor_bottom > top + self.viewport_height {
            cursor_bottom - self.viewport_height
        } else {
            top
        };
        self.set_scroll_pixels(next);
    }

    pub fn handle_enter(&mut self) {
        self.checkpoint();
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
            (0..col)
                .rev()
                .find(|&i| line_text.is_char_boundary(i))
                .unwrap_or(0)
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

        let is_between_delimiters = (trimmed_before.ends_with('{')
            && trimmed_after.starts_with('}'))
            || (trimmed_before.ends_with('(') && trimmed_after.starts_with(')'))
            || (trimmed_before.ends_with('[') && trimmed_after.starts_with(']'));

        if is_between_delimiters {
            // Split between delimiters into 3 lines:
            // Line 1: text_before (e.g. "fn foo() {")
            // Line 2: inner indent (e.g. "    ") <- cursor here
            // Line 3: base_indent + text_after (e.g. "}")
            let inner_indent = format!("{}{}", base_indent, indent_step);
            let insertion = format!("\n{}\n{}", inner_indent, base_indent);

            self.content
                .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                    std::sync::Arc::new(insertion),
                )));
            self.content
                .perform(text_editor::Action::Move(text_editor::Motion::Up));
            self.content
                .perform(text_editor::Action::Move(text_editor::Motion::End));
        } else if opens_block {
            let new_indent = format!("{}{}", base_indent, indent_step);
            self.content
                .perform(text_editor::Action::Edit(text_editor::Edit::Enter));
            if !new_indent.is_empty() {
                self.content
                    .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                        std::sync::Arc::new(new_indent),
                    )));
            }
        } else {
            self.content
                .perform(text_editor::Action::Edit(text_editor::Edit::Enter));
            if !base_indent.is_empty() {
                self.content
                    .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                        std::sync::Arc::new(base_indent),
                    )));
            }
        }

        self.refresh_dirty();
    }

    /// Handles Tab key: indents selection or inserts spaces up to the next 4-space tab stop.
    pub fn indent(&mut self) {
        self.checkpoint();
        let cursor = self.content.cursor();
        if cursor.selection.is_some() {
            self.content
                .perform(text_editor::Action::Edit(text_editor::Edit::Indent));
        } else {
            let col = cursor.position.column;
            let spaces_needed = 4 - (col % 4);
            let spaces_str = " ".repeat(spaces_needed);
            self.content
                .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                    std::sync::Arc::new(spaces_str),
                )));
        }
        self.refresh_dirty();
    }

    /// Handles Shift+Tab key: unindents current line or selection.
    pub fn unindent(&mut self) {
        self.checkpoint();
        self.content
            .perform(text_editor::Action::Edit(text_editor::Edit::Unindent));
        self.refresh_dirty();
    }

    /// Smart backspace: if preceding 4 characters are spaces in indentation, deletes all 4 at once.
    pub fn smart_backspace(&mut self) {
        self.checkpoint();
        let cursor = self.content.cursor();
        if cursor.selection.is_some() {
            self.content
                .perform(text_editor::Action::Edit(text_editor::Edit::Backspace));
            self.is_dirty = true;
            return;
        }

        let line_text = self
            .content
            .line(cursor.position.line)
            .map(|l| l.text.to_string())
            .unwrap_or_default();

        let col = clamp_boundary(&line_text, cursor.position.column);
        let before_cursor = &line_text[..col];

        // If cursor is in leading whitespace and at a tab stop (multiple of 4):
        if before_cursor.chars().all(|c| c == ' ') && col >= 4 && col.is_multiple_of(4) {
            for _ in 0..4 {
                self.content
                    .perform(text_editor::Action::Edit(text_editor::Edit::Backspace));
            }
        } else {
            self.content
                .perform(text_editor::Action::Edit(text_editor::Edit::Backspace));
        }
        self.refresh_dirty();
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
            (0, 15),
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
            (0, 13),
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
            (0, 15), // between { and }
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
            (0, 4),
            FileEncoding::Utf8 { bom: false },
        );
        tab.smart_backspace();
        assert_eq!(tab.content.text(), "");
        assert_eq!(tab.cursor_position(), (0, 0));
    }

    #[test]
    fn test_utf16le_read_write() {
        let dir = std::env::temp_dir();
        let test_file = dir.join(crate::storage::unique_id());
        let sample = "Ciao mondo in UTF-16\nRiga 2!";
        write_file_with_encoding(&test_file, sample, FileEncoding::Utf16Le { bom: true }).unwrap();

        let (read_text, enc) = read_file_with_encoding(&test_file).unwrap();
        assert_eq!(enc, FileEncoding::Utf16Le { bom: true });
        assert_eq!(read_text, sample);

        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_scroll_to_line_and_wheel() {
        let text = (0..100)
            .map(|i| format!("Riga {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut tab = Tab::from_draft(
            "scroll_test".into(),
            None,
            "test".into(),
            text,
            false,
            (0, 0),
            FileEncoding::Utf8 { bom: false },
        );

        assert_eq!(tab.scroll_line, 0);
        tab.scroll_to_line(40);
        assert_eq!(tab.scroll_line, 40);

        // Test wheel scroll down
        tab.on_edit(text_editor::Action::Scroll { lines: 5 });
        assert_eq!(tab.scroll_line, 45);

        // Test wheel scroll up
        tab.on_edit(text_editor::Action::Scroll { lines: -10 });
        assert_eq!(tab.scroll_line, 35);

        // Cursor should remain at (0, 0)
        assert_eq!(tab.cursor_position(), (0, 0));
    }
    #[test]
    fn utf16_without_bom_is_detected_before_utf8() {
        let path = std::env::temp_dir().join(crate::storage::unique_id());
        for encoding in [
            FileEncoding::Utf16Le { bom: false },
            FileEncoding::Utf16Be { bom: false },
        ] {
            write_file_with_encoding(&path, "AB", encoding).unwrap();
            let (text, detected) = read_file_with_encoding(&path).unwrap();
            assert_eq!(text, "AB");
            assert_eq!(detected, encoding);
        }
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn incompatible_legacy_encoding_does_not_modify_file() {
        let path = std::env::temp_dir().join(crate::storage::unique_id());
        fs::write(&path, b"original").unwrap();
        assert!(write_file_with_encoding(&path, "emoji 🦀", FileEncoding::Windows1252).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn invalid_bom_text_is_rejected() {
        let path = std::env::temp_dir().join(crate::storage::unique_id());
        fs::write(&path, [0xff, 0xfe, 0x00, 0xd8]).unwrap();
        assert!(read_file_with_encoding(&path).is_err());
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn undo_redo_and_save_point_track_dirty_state() {
        let mut tab = Tab::new_empty(0);
        tab.on_edit(text_editor::Action::Edit(text_editor::Edit::Insert('é')));
        assert!(tab.is_dirty);
        tab.undo();
        assert_eq!(tab.content.text(), "");
        assert!(!tab.is_dirty);
        tab.redo();
        assert_eq!(tab.content.text(), "é");
        tab.undo();
        tab.on_edit(text_editor::Action::Edit(text_editor::Edit::Insert('x')));
        tab.redo();
        assert_eq!(tab.content.text(), "x");
    }
    #[test]
    fn line_operations_handle_crlf_unicode_and_last_line() {
        use crate::commands::LineCommand;
        let mut tab = Tab::from_draft(
            "line".into(),
            None,
            "test".into(),
            "à\r\nb\r\n🦀".into(),
            false,
            (2, 0),
            FileEncoding::Utf8 { bom: false },
        );
        tab.line_command(LineCommand::MoveUp);
        assert_eq!(tab.content.text(), "à\r\n🦀\r\nb");
        tab.undo();
        tab.line_command(LineCommand::Delete);
        assert_eq!(tab.content.text(), "à\r\nb");
        tab.undo();
        tab.line_command(LineCommand::Duplicate);
        assert_eq!(tab.content.text(), "à\r\nb\r\n🦀\r\n🦀");
    }
    #[test]
    fn byte_position_navigation_and_selection_clear() {
        let mut tab = Tab::from_draft(
            "unicode".into(),
            None,
            "test".into(),
            "città\n🦀x".into(),
            false,
            (0, 0),
            FileEncoding::Utf8 { bom: false },
        );
        tab.select_range(0, 6);
        assert_eq!(tab.content.selection(), Some("città".into()));
        tab.goto(2, 2);
        assert_eq!(tab.cursor_position(), (1, 4));
        assert_eq!(tab.content.selection(), None);
        tab.smart_backspace();
        assert_eq!(tab.content.text(), "città\nx");
    }
    #[test]
    fn partial_visible_line_tracks_renderer_pixels() {
        let text = (1..=120)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut tab = Tab::from_draft(
            "scroll".into(),
            None,
            "test".into(),
            text,
            false,
            (0, 0),
            FileEncoding::Utf8 { bom: false },
        );
        tab.set_viewport(629.0, 21.0);
        tab.goto(80, 1);
        assert_eq!(tab.scroll_line, 50);
        assert_eq!(tab.scroll_offset, 1.0);
        tab.on_edit(text_editor::Action::Scroll { lines: 5 });
        assert_eq!(tab.scroll_line, 55);
        assert_eq!(tab.scroll_offset, 1.0);
        tab.goto(1, 1);
        assert_eq!(tab.scroll_line, 0);
        assert_eq!(tab.scroll_offset, 0.0);
    }
    #[test]
    fn moving_selected_block_keeps_selection_and_handles_mixed_endings() {
        use crate::commands::LineCommand;
        let mut tab = Tab::from_draft(
            "block".into(),
            None,
            "test".into(),
            "à\r\nb\nc".into(),
            false,
            (0, 0),
            FileEncoding::Utf8 { bom: false },
        );
        tab.select_range(0, 5); // the first two lines, excluding their final newline
        tab.line_command(LineCommand::MoveDown);
        assert_eq!(tab.content.text(), "c\nà\r\nb");
        assert!(tab.content.cursor().selection.is_some());
        tab.undo();
        assert_eq!(tab.content.text(), "à\r\nb\nc");
    }
}
