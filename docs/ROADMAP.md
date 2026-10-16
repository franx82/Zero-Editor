# Zero Editor roadmap

A fast local editor inspired by Zed's clarity and Vim's composable commands.
Accounts, integrated AI, telemetry and cloud services are outside the product's scope.

## Available in v0.1.0

- Automatic editor focus, context menus and dismissible popups.
- Protected tab closure, immutable recovery drafts and previous session generation.
- Background session writing shared by native windows, with pending writes completed on exit.
- Search, regex, replacement, line:column navigation and keyboard command palette.
- Undo/redo, line operations, line numbers, syntax highlighting and font zoom.
- Encoding preservation, explicit UTF-8 conversion and atomic writes.
- Reorderable tabs, native window detachment and adjacent-document split.
- English Nord interface, bundled JetBrains Mono and Helvetica Neue Medium UI.
- Automated regression tests and macOS CI.

## Future work — not implemented

1. Text engine: incremental buffers and operation-based history for large documents, explicit horizontal scrolling and performance profiling.
2. Optional Vim mode: Normal/Insert/Visual, counts, motions, operators, text objects, registers, repeat with `.` and macros. Direct editing remains available.
3. Projects: open folders, quick open, file search, recent files and external-change detection before overwriting.
4. Advanced editing: multiple cursors, bracket matching, comments, persistent window/split layouts, dragging between existing windows and configurable keymaps/indentation.
5. Local language support: optional LSP, diagnostics, symbols, rename and formatting.
6. Distribution: Developer ID signing, notarization, Intel/universal packages and compatibility checks on older macOS releases.
