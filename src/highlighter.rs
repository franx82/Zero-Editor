use std::ops::Range;
use std::path::Path;
use std::sync::LazyLock;

use iced::Color;
use iced::advanced::text::highlighter::{Format, Highlighter as IcedHighlighter};
use iced::font::{self, Font};
use two_face::re_exports::syntect::highlighting::{self, Theme as SyntectTheme};
use two_face::re_exports::syntect::parsing::{
    self, ScopeStack, ScopeStackOp, SyntaxReference, SyntaxSet,
};

static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_no_newlines);
static NORD_THEME: LazyLock<SyntectTheme> = LazyLock::new(|| {
    two_face::theme::extra()
        .get(two_face::theme::EmbeddedThemeName::Nord)
        .clone()
});

const LINES_PER_SNAPSHOT: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub token: String,
    pub show_whitespace: bool,
    pub search: Option<(String, bool, bool)>,
}

#[derive(Debug, Clone)]
pub enum Highlight {
    Syntax(highlighting::StyleModifier),
    Whitespace,
    SearchMatch,
}

impl Highlight {
    pub fn color(&self) -> Option<Color> {
        match self {
            Self::Syntax(modifier) => modifier
                .foreground
                .map(|c| Color::from_rgba8(c.r, c.g, c.b, c.a as f32 / 255.0)),
            Self::Whitespace => Some(crate::theme::WHITESPACE_COLOR),
            Self::SearchMatch => Some(crate::theme::NORD13_AURORA_YELLOW),
        }
    }

    pub fn font(&self) -> Option<Font> {
        match self {
            Self::Syntax(modifier) => modifier.font_style.and_then(|style| {
                let bold = style.contains(highlighting::FontStyle::BOLD);
                let italic = style.contains(highlighting::FontStyle::ITALIC);

                if bold || italic {
                    Some(Font {
                        weight: if bold {
                            font::Weight::Bold
                        } else {
                            font::Weight::Normal
                        },
                        style: if italic {
                            font::Style::Italic
                        } else {
                            font::Style::Normal
                        },
                        ..crate::typography::MONO
                    })
                } else {
                    None
                }
            }),
            Self::Whitespace => Some(Font::with_name("ZeroWhitespace")),
            Self::SearchMatch => Some(Font {
                weight: font::Weight::Bold,
                ..crate::typography::MONO
            }),
        }
    }

    pub fn to_format(&self) -> Format<Font> {
        Format {
            color: self.color(),
            font: self.font(),
        }
    }
}

pub struct NordHighlighter {
    syntax: &'static SyntaxReference,
    highlighter: highlighting::Highlighter<'static>,
    caches: Vec<(parsing::ParseState, ScopeStack)>,
    current_line: usize,
    show_whitespace: bool,
    search_pattern: Option<regex::Regex>,
}

fn search_pattern(settings: &Settings) -> Option<regex::Regex> {
    let (query, case_sensitive, regex) = settings.search.as_ref()?;
    if query.is_empty() {
        return None;
    }
    regex::RegexBuilder::new(&if *regex {
        query.clone()
    } else {
        regex::escape(query)
    })
    .case_insensitive(!case_sensitive)
    .multi_line(true)
    .build()
    .ok()
}

impl IcedHighlighter for NordHighlighter {
    type Settings = Settings;
    type Highlight = Highlight;
    type Iterator<'a> = Box<dyn Iterator<Item = (Range<usize>, Self::Highlight)> + 'a>;

    fn new(settings: &Self::Settings) -> Self {
        let syntax = find_syntax(&settings.token);
        let highlighter = highlighting::Highlighter::new(&NORD_THEME);
        let parser = parsing::ParseState::new(syntax);
        let stack = ScopeStack::new();

        Self {
            syntax,
            highlighter,
            caches: vec![(parser, stack)],
            current_line: 0,
            show_whitespace: settings.show_whitespace,
            search_pattern: search_pattern(settings),
        }
    }

    fn update(&mut self, new_settings: &Self::Settings) {
        self.syntax = find_syntax(&new_settings.token);
        self.highlighter = highlighting::Highlighter::new(&NORD_THEME);
        self.show_whitespace = new_settings.show_whitespace;
        self.search_pattern = search_pattern(new_settings);
        self.change_line(0);
    }

    fn change_line(&mut self, line: usize) {
        let snapshot = line / LINES_PER_SNAPSHOT;

        if snapshot <= self.caches.len() {
            self.caches.truncate(snapshot);
            self.current_line = snapshot * LINES_PER_SNAPSHOT;
        } else {
            self.caches.truncate(1);
            self.current_line = 0;
        }

        let (parser, stack) = self
            .caches
            .last()
            .cloned()
            .unwrap_or_else(|| (parsing::ParseState::new(self.syntax), ScopeStack::new()));

        self.caches.push((parser, stack));
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        if self.current_line / LINES_PER_SNAPSHOT >= self.caches.len() {
            let (parser, stack) = self.caches.last().expect("caches not empty");
            self.caches.push((parser.clone(), stack.clone()));
        }

        self.current_line += 1;

        let (parser, stack) = self.caches.last_mut().expect("caches not empty");
        let ops = parser.parse_line(line, &SYNTAXES).unwrap_or_default();

        let syntax_spans = scope_iterator(ops, line, stack, &self.highlighter);
        let show_ws = self.show_whitespace;

        let mut spans: Vec<(Range<usize>, Highlight)> = Vec::new();

        for (range, modifier) in syntax_spans {
            let slice = &line[range.start..range.end];
            if !show_ws || (!slice.contains(' ') && !slice.contains('\t')) {
                spans.push((range, Highlight::Syntax(modifier)));
            } else {
                let mut cur_is_ws = None;
                let mut cur_start = range.start;

                for (offset, c) in slice.char_indices() {
                    let is_ws = c == ' ' || c == '\t';
                    let global_idx = range.start + offset;

                    match cur_is_ws {
                        None => {
                            cur_is_ws = Some(is_ws);
                            cur_start = global_idx;
                        }
                        Some(ws) if ws != is_ws => {
                            let r = cur_start..global_idx;
                            spans.push((
                                r,
                                if ws {
                                    Highlight::Whitespace
                                } else {
                                    Highlight::Syntax(modifier)
                                },
                            ));
                            cur_is_ws = Some(is_ws);
                            cur_start = global_idx;
                        }
                        _ => {}
                    }
                }

                if let Some(ws) = cur_is_ws {
                    let r = cur_start..range.end;
                    spans.push((
                        r,
                        if ws {
                            Highlight::Whitespace
                        } else {
                            Highlight::Syntax(modifier)
                        },
                    ));
                }
            }
        }

        if let Some(pattern) = &self.search_pattern {
            let matches: Vec<_> = pattern.find_iter(line).map(|m| m.range()).collect();
            let mut marked = Vec::new();
            for (range, highlight) in spans {
                let mut boundaries = vec![range.start, range.end];
                for found in &matches {
                    if found.start > range.start && found.start < range.end {
                        boundaries.push(found.start);
                    }
                    if found.end > range.start && found.end < range.end {
                        boundaries.push(found.end);
                    }
                }
                boundaries.sort_unstable();
                boundaries.dedup();
                for pair in boundaries.windows(2) {
                    let matched = matches
                        .iter()
                        .any(|m| m.start <= pair[0] && m.end >= pair[1]);
                    marked.push((
                        pair[0]..pair[1],
                        if matched {
                            Highlight::SearchMatch
                        } else {
                            highlight.clone()
                        },
                    ));
                }
            }
            return Box::new(marked.into_iter());
        }
        Box::new(spans.into_iter())
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

fn scope_iterator<'a>(
    ops: Vec<(usize, ScopeStackOp)>,
    line: &str,
    stack: &'a mut ScopeStack,
    highlighter: &'a highlighting::Highlighter<'static>,
) -> impl Iterator<Item = (Range<usize>, highlighting::StyleModifier)> + 'a {
    ScopeRangeIterator {
        ops,
        line_length: line.len(),
        index: 0,
        last_str_index: 0,
    }
    .filter_map(move |(range, scope)| {
        let _ = stack.apply(&scope);
        if range.is_empty() {
            None
        } else {
            Some((range, highlighter.style_mod_for_stack(&stack.scopes)))
        }
    })
}

struct ScopeRangeIterator {
    ops: Vec<(usize, ScopeStackOp)>,
    line_length: usize,
    index: usize,
    last_str_index: usize,
}

impl Iterator for ScopeRangeIterator {
    type Item = (Range<usize>, ScopeStackOp);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index > self.ops.len() {
            return None;
        }

        let next_str_i = if self.index == self.ops.len() {
            self.line_length
        } else {
            self.ops[self.index].0
        };

        let range = self.last_str_index..next_str_i;
        self.last_str_index = next_str_i;

        let op = if self.index == 0 {
            ScopeStackOp::Noop
        } else {
            self.ops[self.index - 1].1.clone()
        };

        self.index += 1;
        Some((range, op))
    }
}

pub fn find_syntax(token: &str) -> &'static SyntaxReference {
    if token.is_empty() || token == "txt" {
        return SYNTAXES.find_syntax_plain_text();
    }

    SYNTAXES
        .find_syntax_by_extension(token)
        .or_else(|| SYNTAXES.find_syntax_by_token(token))
        .or_else(|| SYNTAXES.find_syntax_by_name(token))
        .unwrap_or_else(|| SYNTAXES.find_syntax_plain_text())
}

/// Detects the syntax token and human-readable name for a given file and initial text.
pub fn detect_syntax_and_name(path: Option<&Path>, preview: &str) -> (String, String) {
    if let Some(path) = path {
        // 1. Check exact filename (Dockerfile, Makefile, Cargo.lock, etc.)
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            match file_name {
                "Dockerfile" | "dockerfile" => return ("dockerfile".into(), "Dockerfile".into()),
                "Makefile" | "makefile" | "GNUmakefile" => {
                    return ("makefile".into(), "Makefile".into());
                }
                "Cargo.lock" => return ("toml".into(), "TOML (Cargo.lock)".into()),
                "Cargo.toml" => return ("toml".into(), "TOML (Cargo.toml)".into()),
                ".gitignore" | ".gitattributes" | ".gitmodules" => {
                    return ("git".into(), "Git Config".into());
                }
                "CMakeLists.txt" => return ("cmake".into(), "CMake".into()),
                _ => {}
            }
        }

        // 2. Check extension in 919 bundled syntaxes
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            let ext_lower = ext.to_lowercase();
            if let Some(syntax) = SYNTAXES.find_syntax_by_extension(&ext_lower) {
                return (ext_lower, syntax.name.clone());
            }
        }
    }

    // 3. Check shebang if preview is available
    if preview.starts_with("#!") {
        let first_line = preview.lines().next().unwrap_or("");
        if first_line.contains("python") {
            return ("py".into(), "Python".into());
        }
        if first_line.contains("bash") || first_line.contains("sh") || first_line.contains("zsh") {
            return ("sh".into(), "Shell".into());
        }
        if first_line.contains("node") {
            return ("js".into(), "JavaScript".into());
        }
        if first_line.contains("ruby") {
            return ("rb".into(), "Ruby".into());
        }
        if first_line.contains("perl") {
            return ("pl".into(), "Perl".into());
        }
    }

    ("txt".into(), "Plain Text".into())
}

/// Major curated languages for fast picker or status display.
pub fn major_languages() -> &'static [(&'static str, &'static str)] {
    &[
        ("rs", "Rust"),
        ("py", "Python"),
        ("js", "JavaScript"),
        ("ts", "TypeScript"),
        ("jsx", "React JSX"),
        ("tsx", "React TSX"),
        ("html", "HTML"),
        ("css", "CSS"),
        ("scss", "SCSS"),
        ("json", "JSON"),
        ("toml", "TOML"),
        ("yaml", "YAML"),
        ("md", "Markdown"),
        ("c", "C"),
        ("cpp", "C++"),
        ("cs", "C#"),
        ("go", "Go"),
        ("java", "Java"),
        ("kt", "Kotlin"),
        ("swift", "Swift"),
        ("rb", "Ruby"),
        ("php", "PHP"),
        ("sh", "Shell / Bash"),
        ("sql", "SQL"),
        ("dockerfile", "Dockerfile"),
        ("makefile", "Makefile"),
        ("lua", "Lua"),
        ("zig", "Zig"),
        ("xml", "XML"),
        ("diff", "Diff / Patch"),
        ("txt", "Plain Text"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_major_language_syntaxes_found() {
        let (token, name) = detect_syntax_and_name(Some(Path::new("main.rs")), "");
        assert_eq!(token, "rs");
        assert_eq!(name, "Rust");

        let (token, name) = detect_syntax_and_name(Some(Path::new("script.py")), "");
        assert_eq!(token, "py");
        assert_eq!(name, "Python");

        let (token, _name) = detect_syntax_and_name(Some(Path::new("app.tsx")), "");
        assert_eq!(token, "tsx");

        let (token, _) = detect_syntax_and_name(Some(Path::new("Dockerfile")), "");
        assert_eq!(token, "dockerfile");

        let (token, _) = detect_syntax_and_name(Some(Path::new("Makefile")), "");
        assert_eq!(token, "makefile");

        let (token, name) = detect_syntax_and_name(None, "#!/usr/bin/env python3\nprint('hello')");
        assert_eq!(token, "py");
        assert_eq!(name, "Python");

        // Verify find_syntax doesn't return plain text for major tokens
        assert_ne!(find_syntax("rs").name, "Plain Text");
        assert_ne!(find_syntax("py").name, "Plain Text");
        assert_ne!(find_syntax("js").name, "Plain Text");
        assert_ne!(find_syntax("cpp").name, "Plain Text");
        assert_ne!(find_syntax("go").name, "Plain Text");
    }

    #[test]
    fn test_syntect_with_dots() {
        let mut h1 = NordHighlighter::new(&Settings {
            token: "rs".into(),
            show_whitespace: false,
            search: None,
        });
        let spans1: Vec<_> = h1.highlight_line("let x = 5;").collect();

        let mut h2 = NordHighlighter::new(&Settings {
            token: "rs".into(),
            show_whitespace: false,
            search: None,
        });
        let spans2: Vec<_> = h2.highlight_line("let·x·=·5;").collect();

        eprintln!(
            "Normal: {:?}",
            spans1
                .iter()
                .map(|(r, h)| (r.clone(), h.color()))
                .collect::<Vec<_>>()
        );
        eprintln!(
            "With dots: {:?}",
            spans2
                .iter()
                .map(|(r, h)| (r.clone(), h.color()))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_highlighter_whitespace_enabled() {
        let mut h = NordHighlighter::new(&Settings {
            token: "rs".into(),
            show_whitespace: true,
            search: None,
        });
        let spans: Vec<_> = h.highlight_line("  let x = 1;\t").collect();

        let ws_spans: Vec<_> = spans
            .iter()
            .filter(|(_, h)| matches!(h, Highlight::Whitespace))
            .collect();
        assert!(!ws_spans.is_empty(), "Whitespace spans must be generated");
        // Initial 2 spaces
        assert_eq!(ws_spans[0].0, 0..2);
        // Space between let and x
        assert_eq!(ws_spans[1].0, 5..6);
    }

    #[test]
    fn test_whitespace_font_loading() {
        let font_bytes = include_bytes!("../assets/ZeroWhitespace.ttf").to_vec();
        let mut db = cosmic_text::fontdb::Database::new();
        let ids = db.load_font_source(cosmic_text::fontdb::Source::Binary(std::sync::Arc::new(
            font_bytes,
        )));
        assert!(!ids.is_empty(), "Font was loaded successfully");
        let face = db.face(ids[0]).expect("face exists");
        assert_eq!(face.families[0].0, "ZeroWhitespace");

        let mut font_system = cosmic_text::FontSystem::new_with_locale_and_db("en-US".into(), db);
        let mut buffer =
            cosmic_text::Buffer::new(&mut font_system, cosmic_text::Metrics::new(14.0, 20.0));
        let attrs = cosmic_text::Attrs::new().family(cosmic_text::Family::Name("ZeroWhitespace"));
        buffer.set_text(
            &mut font_system,
            " \t ",
            &attrs,
            cosmic_text::Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut font_system, false);

        let mut glyph_count = 0;
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                eprintln!("Glyph: id={}, x={}, w={}", glyph.glyph_id, glyph.x, glyph.w);
                glyph_count += 1;
            }
        }
        let mut buffer2 =
            cosmic_text::Buffer::new(&mut font_system, cosmic_text::Metrics::new(14.0, 20.0));
        let attrs2 = cosmic_text::Attrs::new().family(cosmic_text::Family::Monospace);
        buffer2.set_text(
            &mut font_system,
            "   ",
            &attrs2,
            cosmic_text::Shaping::Advanced,
            None,
        );
        buffer2.shape_until_scroll(&mut font_system, false);
        for run in buffer2.layout_runs() {
            for glyph in run.glyphs {
                eprintln!("Monospace glyph width: {}", glyph.w);
            }
        }
        assert_eq!(glyph_count, 3, "Space, tab, space shaped into glyphs");
    }

    #[test]
    fn test_whitespace_split_ranges() {
        let line = "    fn foo(a: i32) {\tlet x = 1; }";
        // Verify full byte coverage
        let mut byte_idx = 0;
        let mut chunks = Vec::new();

        for (i, c) in line.char_indices() {
            let is_ws = c == ' ' || c == '\t';
            match chunks.last_mut() {
                Some((last_is_ws, start, end)) if *last_is_ws == is_ws => {
                    *end = i + c.len_utf8();
                }
                _ => {
                    chunks.push((is_ws, i, i + c.len_utf8()));
                }
            }
        }

        for (is_ws, start, end) in &chunks {
            assert_eq!(*start, byte_idx, "Ranges must be contiguous");
            byte_idx = *end;
            let slice = &line[*start..*end];
            if *is_ws {
                assert!(slice.chars().all(|c| c == ' ' || c == '\t'));
            } else {
                assert!(slice.chars().all(|c| c != ' ' && c != '\t'));
            }
        }
        assert_eq!(byte_idx, line.len(), "All bytes accounted for");
    }
    #[test]
    fn search_matches_are_visible_while_search_input_has_focus() {
        let mut h = NordHighlighter::new(&Settings {
            token: "rs".into(),
            show_whitespace: false,
            search: Some(("println".into(), false, false)),
        });
        let spans: Vec<_> = h.highlight_line("    println!(\"hi\");").collect();
        assert!(
            spans
                .iter()
                .any(|(range, h)| *range == (4..11) && matches!(h, Highlight::SearchMatch))
        );
    }
}
