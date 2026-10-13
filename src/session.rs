use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::tab::Tab;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabRecord {
    pub id: String,
    pub path: Option<PathBuf>,
    pub title: String,
    pub is_dirty: bool,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub draft_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub active_tab: usize,
    pub tabs: Vec<TabRecord>,
}

pub fn session_dir() -> PathBuf {
    if let Some(mut dir) = dirs::data_local_dir() {
        let legacy_dir = dir.join("dev.editor.zero");
        dir.push("dev.zero.editor");
        if !dir.exists() && legacy_dir.exists() {
            let _ = fs::rename(&legacy_dir, &dir);
        }
        dir
    } else {
        PathBuf::from(".zero_editor_session")
    }
}

pub fn drafts_dir() -> PathBuf {
    let mut dir = session_dir();
    dir.push("drafts");
    dir
}

pub fn save_session(tabs: &[Tab], active_tab: usize) {
    save_session_to(tabs, active_tab, &session_dir(), &drafts_dir());
}

pub fn load_session() -> Option<(Vec<Tab>, usize)> {
    load_session_from(&session_dir(), &drafts_dir())
}

pub fn save_session_to(tabs: &[Tab], active_tab: usize, s_dir: &Path, d_dir: &Path) {
    if let Err(e) = fs::create_dir_all(d_dir) {
        eprintln!("Failed to create drafts dir: {e}");
        return;
    }

    let mut tab_records = Vec::new();
    let mut current_draft_files = Vec::new();

    for tab in tabs {
        let (cursor_line, cursor_col) = tab.cursor_position();
        let mut draft_filename = None;

        // Save a draft if:
        // 1. The tab is untitled, OR
        // 2. The tab has unsaved modifications (dirty)
        if tab.is_dirty || tab.path.is_none() {
            let filename = format!("{}.txt", tab.id);
            let draft_path = d_dir.join(&filename);
            let text = tab.content.text();
            if let Err(e) = fs::write(&draft_path, text) {
                eprintln!("Failed to write draft {filename}: {e}");
            } else {
                current_draft_files.push(filename.clone());
                draft_filename = Some(filename);
            }
        }

        tab_records.push(TabRecord {
            id: tab.id.clone(),
            path: tab.path.clone(),
            title: tab.title.clone(),
            is_dirty: tab.is_dirty,
            cursor_line,
            cursor_col,
            draft_file: draft_filename,
        });
    }

    let record = SessionRecord {
        active_tab: if active_tab < tabs.len() { active_tab } else { 0 },
        tabs: tab_records,
    };

    if let Ok(json) = serde_json::to_string_pretty(&record) {
        let session_file = s_dir.join("session.json");
        let _ = fs::write(session_file, json);
    }

    // Clean up orphaned drafts
    if let Ok(entries) = fs::read_dir(d_dir) {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                if !current_draft_files.contains(&name) {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
}

pub fn load_session_from(s_dir: &Path, d_dir: &Path) -> Option<(Vec<Tab>, usize)> {
    let session_file = s_dir.join("session.json");

    if !session_file.exists() {
        return None;
    }

    let content = fs::read_to_string(session_file).ok()?;
    let record: SessionRecord = serde_json::from_str(&content).ok()?;

    if record.tabs.is_empty() {
        return None;
    }

    let mut tabs = Vec::new();

    for tab_rec in record.tabs {
        // Priority 1: Check if draft exists (unsaved changes or untitled buffer)
        if let Some(draft_name) = &tab_rec.draft_file {
            let draft_path = d_dir.join(draft_name);
            if let Ok(draft_text) = fs::read_to_string(&draft_path) {
                let tab = Tab::from_draft(
                    tab_rec.id,
                    tab_rec.path,
                    tab_rec.title,
                    draft_text,
                    tab_rec.is_dirty,
                    tab_rec.cursor_line,
                    tab_rec.cursor_col,
                );
                tabs.push(tab);
                continue;
            }
        }

        // Priority 2: If path is set and exists on disk, load directly from disk
        if let Some(path) = &tab_rec.path {
            if path.exists() {
                if let Ok(file_text) = fs::read_to_string(path) {
                    let tab = Tab::from_draft(
                        tab_rec.id,
                        Some(path.clone()),
                        tab_rec.title,
                        file_text,
                        false,
                        tab_rec.cursor_line,
                        tab_rec.cursor_col,
                    );
                    tabs.push(tab);
                    continue;
                }
            }
        }

        // Priority 3: Fallback empty untitled buffer
        let tab = Tab::from_draft(
            tab_rec.id,
            None,
            tab_rec.title,
            String::new(),
            false,
            0,
            0,
        );
        tabs.push(tab);
    }

    if tabs.is_empty() {
        None
    } else {
        let active_index = if record.active_tab < tabs.len() {
            record.active_tab
        } else {
            0
        };
        Some((tabs, active_index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_session_save_and_restore() {
        let temp_dir = env::temp_dir().join(format!("editor_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let s_dir = temp_dir.join("session");
        let d_dir = temp_dir.join("drafts");

        fs::create_dir_all(&s_dir).unwrap();
        fs::create_dir_all(&d_dir).unwrap();

        // Create a test file
        let file_path = temp_dir.join("example.rs");
        fs::write(&file_path, "fn hello() { println!(\"world\"); }").unwrap();

        let tab1 = Tab::from_file(file_path.clone()).unwrap();
        let mut tab2 = Tab::new_empty(1);
        tab2.content = iced::widget::text_editor::Content::with_text("Unsaved quick notes");
        tab2.is_dirty = true;

        let tabs = vec![tab1, tab2];
        save_session_to(&tabs, 1, &s_dir, &d_dir);

        // Verify session.json was created
        assert!(s_dir.join("session.json").exists());

        // Restore session
        let (restored_tabs, active) = load_session_from(&s_dir, &d_dir).expect("Session should be restored");
        assert_eq!(restored_tabs.len(), 2);
        assert_eq!(active, 1);
        assert_eq!(restored_tabs[0].title, "example.rs");
        assert_eq!(restored_tabs[1].content.text(), "Unsaved quick notes");
        assert!(restored_tabs[1].is_dirty);

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir);
    }
}
