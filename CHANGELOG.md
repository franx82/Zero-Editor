# Changelog

## 0.3.0 — 2026-10-21

- Switch the editor font to Meslo LG for rounder, less condensed glyphs.
- Trackpad swipes and Shift+wheel now scroll horizontally when word wrapping is disabled.
- Clicking no longer shifts the view; while typing the view keeps a few columns of margin.
- Selection drags keep scrolling horizontally past the editor edge, even outside the window.
- The command palette and language picker scroll with the mouse wheel and trackpad.

## 0.2.0 — 2026-10-19

- Add word wrapping, enabled by default, with `⌘⌥W` to toggle it.
- Add horizontal scrolling when word wrapping is disabled; the viewport follows the cursor.

## 0.1.0 — 2026-10-16

First downloadable release for macOS Apple Silicon.

- English interface with a unified Nord titlebar, Helvetica Neue Medium UI and bundled JetBrains Mono editor fonts.
- Redesigned tabs with dirty indicators, drag reordering and native window detachment.
- Create documents with the inline `+` button or a double-click on empty tab-bar space.
- Split adjacent documents; the last tab pairs with its predecessor.
- Context menus for the editor, tabs and empty tab bar; keyboard navigation and click-outside dismissal.
- Immediate editor focus after opening files, switching tabs and closing panels.
- Command palette, literal/regex search, replace, and Unicode-aware line:column navigation.
- Per-document undo/redo, line duplication, deletion, movement and smart indentation.
- Syntax highlighting, line numbers, adjustable font size and optional whitespace markers.
- UTF-8, UTF-16 and Windows-1252 support with explicit conversion to UTF-8.
- Save / Don't save / Cancel confirmation for modified tabs.
- Shared background session writer across windows, immutable drafts, previous session generation and process locking.
- Atomic file replacement, symlink handling and Unix permission preservation.
- Finder/LaunchServices file opening, visible error reporting and macOS CI.

`⌘⇧D` duplicates lines; whitespace markers use `⌘.`.

Known limits: Vim modal editing and LSP are not implemented. Window and split layouts
are not restored; recovered documents reopen in one window. External file changes
are not detected before saving. The app is ad hoc signed and not notarized.
