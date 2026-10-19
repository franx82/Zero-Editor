<p align="center"><img src="assets/icon.png" width="120" alt="Zero Editor" /></p>
<h1 align="center">Zero Editor</h1>
<p align="center">A local macOS text editor built in Rust. A clean interface and keyboard-driven editing.</p>

Inspired by Zed's simplicity and Vim's editing workflow. No accounts, integrated AI or remote document services.

## Download

Download **Zero-Editor-v0.2.0-macos-arm64.zip** from the [latest release](https://github.com/franx82/Zero-Editor/releases/latest),
extract it and move **Zero Editor.app** to Applications. This package targets **Apple Silicon**, with macOS 12 as its declared minimum.
Only the current development Mac has been checked; older macOS versions have not been verified.

The app is ad hoc signed, without a Developer ID signature or notarization. macOS may block its first launch.
Release assets include a SHA-256 checksum file. Intel and universal builds are not included.

## Features

- English Nord interface, unified macOS titlebar, Helvetica Neue Medium UI and bundled JetBrains Mono editor fonts.
- Reorder tabs by dragging; detach them into native windows by dragging out of the bar.
- New tabs from the inline `+`, double-clicking empty tab-bar space, or `⌘N` / `⌘T`.
- Split with the next tab, or the previous tab when the current tab is last.
- Finder, drag-and-drop and command-line file opening, with immediate editor focus.
- Searchable command palette, context menus and popups that close when clicked outside.
- Literal/regex search, case sensitivity, wraparound and single/global replacement; regex replacements support `$1`, `$2`, etc.
- Unicode-aware line:column navigation, undo/redo and line/block editing commands.
- Line numbers, syntax highlighting, language selection, font zoom and optional whitespace markers.
- Word wrapping by default; toggle it with `⌘⌥W`. With wrapping off, long lines scroll horizontally and follow the cursor.
- Smart indentation, Tab/Shift+Tab and indentation-aware backspace.
- UTF-8, UTF-16 LE/BE with or without BOM, and Windows-1252 fallback.

Editing is direct. Vim Normal/Insert/Visual modes, operators and registers are planned, but **not implemented**.
See the [roadmap](docs/ROADMAP.md).

## Tabs and windows

Drag a tab along the bar to reorder it, or vertically outside the bar to move it into a new native window.
The context menu offers the same action. Middle-click closes a tab.

The split icon (`⌘\`) pairs the current tab with its immediate right neighbor. On the last tab, it pairs
with the previous tab, preserving tab order. Activate it again to return to one pane. Editing either pane
keeps the pair visible; selecting a tab outside the pair updates the split. A single tab does not split.

The status bar shows the document path, language, encoding and cursor position.
Whitespace markers are available from the command palette or `⌘.`.

## Document recovery

Session data lives in `~/Library/Application Support/dev.zero.editor/`:

- `session.json`: documents, order, cursor position, language and encoding.
- `drafts/`: copies of unsaved changes.
- `session.previous.json`: the previous committed session and its retained drafts.
- `session.lock`: prevents competing processes from overwriting the same session.

Changes are archived on a background thread after approximately 400 ms of inactivity. Closing a window
or quitting waits for pending writes. A crash can lose changes that have not yet been archived.
Files and manifests use a synchronized temporary file and rename; immutable drafts preserve the previous
session if a new write fails. Saving follows symlinks and preserves Unix permissions, but does not guarantee
preservation of ACLs or extended attributes.

Closing a modified tab offers **Save / Don't save / Cancel**. Files received at startup are added to the
recovered session. If recovery fails, the original data is preserved and automatic archiving is suspended.
A second process can edit files but cannot archive drafts into the locked session.

All windows share one session writer. Detaching preserves the buffer, cursor and undo history. Closing
a window retains its documents for the next launch; close individual tabs to remove them from the session.
Recovered documents reopen in **one window**. Window geometry and split layouts are not persisted, and
dragging tabs between existing windows is not supported.

Malformed Unicode with a BOM is rejected. If a Windows-1252 document contains unrepresentable characters,
saving stops; explicitly convert it to UTF-8 from the palette before saving again.

## Shortcuts

| Action | Shortcut |
|---|---|
| Command palette | `⌘⇧P` |
| Toggle adjacent split | `⌘\` |
| Toggle word wrap | `⌘⌥W` |
| New tab | `⌘N` / `⌘T` |
| Open file | `⌘O` |
| Save / Save as | `⌘S` / `⌘⇧S` |
| Close tab / Quit | `⌘W` / `⌘Q` |
| Find / Find and replace | `⌘F` / `⌘⌥F` |
| Next / previous match | `⌘G` / `⌘⇧G` |
| Go to line:column | `⌃G` |
| Undo / Redo | `⌘Z` / `⌘⇧Z` |
| Duplicate lines | `⌘⇧D` |
| Delete lines | `⌘⇧K` |
| Move lines up / down | `⌥↑` / `⌥↓` |
| Select language | `⌘L` |
| Toggle whitespace markers | `⌘.` |
| Increase / decrease / reset font size | `⌘+` / `⌘−` / `⌘0` |
| Select tab | `⌘1` … `⌘9` |
| Next / previous tab | `⌃Tab` / `⌃⇧Tab` |
| Close popup and return to editing | `Esc` |

## Build and development

Requires macOS, Command Line Tools and Rust stable (edition 2024). Dependencies are pinned in `Cargo.lock`.

```bash
cargo run --locked -- src/main.rs
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
./scripts/build_app.sh
```

The script creates an ad hoc signed `target/Zero Editor.app` without installing it.
Use `./scripts/build_app.sh --install` to install explicitly, or `--debug` for a development bundle.
Set `ZERO_EDITOR_STATE_DIR=/path/to/state` to isolate a development session.

`src/main.rs` owns each window's UI; `src/desktop.rs` coordinates native windows and session writing.
`src/tab.rs` implements document editing and encoding; `src/session.rs` and `src/storage.rs` handle recovery
and atomic writes. The remaining modules cover search, commands, tabs, context menus, highlighting and theme.

## Current limits

No LSP, multiple cursors, project explorer, macros or plugins. Undo history uses bounded in-memory snapshots
and does not survive a restart. Search highlights render per line, while multiline regexes remain navigable
and replaceable. External file changes are not detected before saving.

## License

[MIT](LICENSE) for code. [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono) is distributed under
the [SIL Open Font License 1.1](assets/fonts/OFL.txt).
