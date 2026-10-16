use iced::widget::text_editor::Position;
use regex::{Regex, RegexBuilder};
use std::ops::Range;

#[derive(Debug, Default)]
pub struct Search {
    pub query: String,
    pub replacement: String,
    pub case_sensitive: bool,
    pub regex: bool,
    pub matches: Vec<Range<usize>>,
    pub current: Option<usize>,
    pub error: Option<String>,
}

impl Search {
    pub fn pattern(&self) -> Result<Regex, regex::Error> {
        RegexBuilder::new(&if self.regex {
            self.query.clone()
        } else {
            regex::escape(&self.query)
        })
        .case_insensitive(!self.case_sensitive)
        .multi_line(true)
        .build()
    }

    pub fn refresh(&mut self, text: &str) {
        self.matches.clear();
        self.current = None;
        self.error = None;
        if self.query.is_empty() {
            return;
        }
        match self.pattern() {
            Ok(pattern) => self.matches = pattern.find_iter(text).map(|m| m.range()).collect(),
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn next(&mut self, offset: usize, backwards: bool) -> Option<Range<usize>> {
        let n = self.matches.len();
        if n == 0 {
            return None;
        }
        let index = if let Some(current) = self.current {
            if backwards {
                (current + n - 1) % n
            } else {
                (current + 1) % n
            }
        } else if backwards {
            self.matches
                .iter()
                .rposition(|m| m.start < offset)
                .unwrap_or(n - 1)
        } else {
            self.matches
                .iter()
                .position(|m| m.start >= offset)
                .unwrap_or(0)
        };
        self.current = Some(index);
        Some(self.matches[index].clone())
    }

    pub fn replacement_for(&self, text: &str, range: Range<usize>) -> String {
        if !self.regex {
            return self.replacement.clone();
        }
        let Ok(pattern) = self.pattern() else {
            return text[range].to_owned();
        };
        if let Some(captures) = pattern.captures_at(text, range.start)
            && captures.get(0).is_some_and(|m| m.range() == range)
        {
            let mut replacement = String::new();
            captures.expand(&self.replacement, &mut replacement);
            return replacement;
        }
        text[range].to_owned()
    }

    pub fn replace_all(&self, text: &str) -> Option<String> {
        if self.query.is_empty() {
            return None;
        }
        let pattern = self.pattern().ok()?;
        Some(if self.regex {
            pattern
                .replace_all(text, self.replacement.as_str())
                .into_owned()
        } else {
            pattern
                .replace_all(text, regex::NoExpand(&self.replacement))
                .into_owned()
        })
    }
}

// iced/cosmic-text cursor columns are UTF-8 byte offsets. Keep CRLF intact.
pub fn line_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\r' || bytes[i] == b'\n' {
            let first = bytes[i];
            i += 1;
            if i < bytes.len()
                && ((first == b'\r' && bytes[i] == b'\n') || (first == b'\n' && bytes[i] == b'\r'))
            {
                i += 1;
            }
            ranges.push(start..i);
            start = i;
        } else {
            i += 1;
        }
    }
    ranges.push(start..text.len());
    ranges
}

pub fn position_at(text: &str, offset: usize) -> Position {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let ranges = line_ranges(text);
    let line = ranges.iter().rposition(|r| r.start <= offset).unwrap_or(0);
    let raw = &text[ranges[line].clone()];
    let length = raw.trim_end_matches(['\r', '\n']).len();
    Position {
        line,
        column: (offset - ranges[line].start).min(length),
    }
}

pub fn offset_at(text: &str, position: Position) -> usize {
    let ranges = line_ranges(text);
    let range = &ranges[position.line.min(ranges.len() - 1)];
    let mut offset = (range.start + position.column).min(range.end);
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_crlf_and_wraparound() {
        let text = "città\r\nCITTÀ\r\n🦀";
        let mut search = Search {
            query: "città".into(),
            ..Search::default()
        };
        search.refresh(text);
        assert_eq!(search.matches.len(), 2);
        assert_eq!(search.next(0, false), Some(0..6));
        assert_eq!(search.next(0, true), Some(8..14));
        assert_eq!(position_at(text, 8), Position { line: 1, column: 0 });
        assert_eq!(offset_at(text, Position { line: 2, column: 4 }), text.len());
    }
    #[test]
    fn regex_replacement_and_literal_dollars() {
        let mut s = Search {
            query: "(\\w+)=(\\d+)".into(),
            replacement: "$2:$1".into(),
            regex: true,
            ..Search::default()
        };
        assert_eq!(s.replace_all("foo=42"), Some("42:foo".into()));
        s.query = "foo".into();
        s.replacement = "$1".into();
        s.regex = false;
        assert_eq!(s.replace_all("foo foo"), Some("$1 $1".into()));
        s.regex = true;
        s.query = "[".into();
        s.refresh("foo");
        assert!(s.error.is_some());
    }
    #[test]
    fn zero_width_replacement_keeps_full_document_context() {
        let s = Search {
            query: r"\b".into(),
            replacement: "|".into(),
            regex: true,
            ..Default::default()
        };
        assert_eq!(s.replacement_for("word", 0..0), "|");
    }
}
