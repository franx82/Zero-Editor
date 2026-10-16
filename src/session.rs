use crate::storage::atomic_write;
use serde::{Deserialize, Serialize};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

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
    #[serde(default)]
    pub encoding: Option<crate::tab::FileEncoding>,
    #[serde(default)]
    pub manual_syntax: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub active_tab: usize,
    pub tabs: Vec<TabRecord>,
}

pub fn session_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("ZERO_EDITOR_STATE_DIR") {
        return PathBuf::from(path);
    }
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

// A process owns the session for its entire lifetime. A second instance can edit
// files, but must not overwrite the first instance's drafts.
pub fn lock_session() -> io::Result<fs::File> {
    let dir = session_dir();
    fs::create_dir_all(&dir)?;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("session.lock"))?;
    file.try_lock()
        .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(file)
}

pub fn load_session() -> io::Result<Option<(Vec<Tab>, usize)>> {
    load_session_from(&session_dir(), &drafts_dir())
}

pub struct Snapshot {
    record: SessionRecord,
    drafts: Vec<(String, String)>,
}

impl Snapshot {
    #[cfg(test)]
    pub fn capture(tabs: &[Tab], active_tab: usize) -> Self {
        Self::capture_iter(tabs.iter(), active_tab)
    }

    pub fn capture_iter<'a>(tabs: impl IntoIterator<Item = &'a Tab>, active_tab: usize) -> Self {
        let mut records = Vec::new();
        let mut drafts = Vec::new();
        for tab in tabs {
            let (cursor_line, cursor_col) = tab.cursor_position();
            let draft_file = if tab.is_dirty || tab.path.is_none() {
                let text = tab.content.text();
                let mut hash = std::collections::hash_map::DefaultHasher::new();
                tab.id.hash(&mut hash);
                text.hash(&mut hash);
                let name = format!("draft-{:016x}.txt", hash.finish());
                drafts.push((name.clone(), text));
                Some(name)
            } else {
                None
            };
            records.push(TabRecord {
                id: tab.id.clone(),
                path: tab.path.clone(),
                title: tab.title.clone(),
                is_dirty: tab.is_dirty,
                cursor_line,
                cursor_col,
                draft_file,
                encoding: Some(tab.encoding),
                manual_syntax: tab.manual_syntax.clone(),
            });
        }
        Self {
            record: SessionRecord {
                active_tab: active_tab.min(records.len().saturating_sub(1)),
                tabs: records,
            },
            drafts,
        }
    }

    pub fn save(self) -> io::Result<()> {
        self.save_to(&session_dir(), &drafts_dir())
    }

    pub fn save_to(self, s_dir: &Path, d_dir: &Path) -> io::Result<()> {
        fs::create_dir_all(s_dir)?;
        fs::create_dir_all(d_dir)?;
        // Immutable, content-addressed drafts: the previous manifest remains valid
        // until the new manifest has been committed successfully.
        for (name, text) in &self.drafts {
            let path = d_dir.join(name);
            match fs::read(&path) {
                Ok(bytes) if bytes == text.as_bytes() => {}
                Ok(_) => {
                    return Err(io::Error::other(
                        "Existing draft is incompatible and has been preserved without overwriting",
                    ));
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    atomic_write(&path, text.as_bytes())?
                }
                Err(e) => return Err(e),
            }
        }
        let session_file = s_dir.join("session.json");
        // Keep a complete previous generation for recovery, including its drafts.
        let previous = match fs::read(&session_file) {
            Ok(bytes) => {
                let record: SessionRecord =
                    serde_json::from_slice(&bytes).map_err(io::Error::other)?;
                atomic_write(&s_dir.join("session.previous.json"), &bytes)?;
                Some(record)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        let json = serde_json::to_vec_pretty(&self.record).map_err(io::Error::other)?;
        atomic_write(&session_file, &json)?;
        let mut keep: Vec<String> = self.drafts.iter().map(|(n, _)| n.clone()).collect();
        if let Some(previous) = previous {
            keep.extend(previous.tabs.into_iter().filter_map(|r| r.draft_file));
        }
        for entry in fs::read_dir(d_dir)?.flatten() {
            if let Some(name) = entry.file_name().to_str()
                && name.starts_with("draft-")
                && name.ends_with(".txt")
                && !keep.iter().any(|n| n == name)
            {
                let _ = fs::remove_file(entry.path());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub fn save_session_to(
    tabs: &[Tab],
    active_tab: usize,
    s_dir: &Path,
    d_dir: &Path,
) -> io::Result<()> {
    Snapshot::capture(tabs, active_tab).save_to(s_dir, d_dir)
}

pub fn load_session_from(s_dir: &Path, d_dir: &Path) -> io::Result<Option<(Vec<Tab>, usize)>> {
    let session_file = s_dir.join("session.json");

    if !session_file.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(session_file)?;
    let record: SessionRecord = serde_json::from_str(&content).map_err(io::Error::other)?;

    if record.tabs.is_empty() {
        return Ok(None);
    }

    let mut tabs = Vec::new();

    for tab_rec in record.tabs {
        let encoding = tab_rec
            .encoding
            .unwrap_or(crate::tab::FileEncoding::Utf8 { bom: false });

        // Priority 1: Check if draft exists (unsaved changes or untitled buffer)
        if let Some(draft_name) = &tab_rec.draft_file {
            let draft_path = d_dir.join(draft_name);
            {
                let draft_text = fs::read_to_string(&draft_path)?;
                let mut tab = Tab::from_draft(
                    tab_rec.id,
                    tab_rec.path,
                    tab_rec.title,
                    draft_text,
                    tab_rec.is_dirty,
                    (tab_rec.cursor_line, tab_rec.cursor_col),
                    encoding,
                );
                tab.manual_syntax = tab_rec.manual_syntax;
                tabs.push(tab);
                continue;
            }
        }

        // Priority 2: If path is set and exists on disk, load directly from disk with encoding detection
        if let Some(path) = &tab_rec.path
            && path.exists()
            && let Ok((file_text, detected_enc)) = crate::tab::read_file_with_encoding(path)
        {
            let mut tab = Tab::from_draft(
                tab_rec.id,
                Some(path.clone()),
                tab_rec.title,
                file_text,
                false,
                (tab_rec.cursor_line, tab_rec.cursor_col),
                detected_enc,
            );
            tab.manual_syntax = tab_rec.manual_syntax;
            tabs.push(tab);
            continue;
        }

        // Priority 3: Fallback empty untitled buffer
        let tab = Tab::from_draft(
            tab_rec.id,
            None,
            tab_rec.title,
            String::new(),
            false,
            (0, 0),
            encoding,
        );
        tabs.push(tab);
    }

    if tabs.is_empty() {
        Ok(None)
    } else {
        let active_index = if record.active_tab < tabs.len() {
            record.active_tab
        } else {
            0
        };
        Ok(Some((tabs, active_index)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_session_save_and_restore() {
        let temp_dir = env::temp_dir().join(format!(
            "editor_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
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
        save_session_to(&tabs, 1, &s_dir, &d_dir).unwrap();

        // Verify session.json was created
        assert!(s_dir.join("session.json").exists());

        // Restore session
        let (restored_tabs, active) = load_session_from(&s_dir, &d_dir)
            .unwrap()
            .expect("Session should be restored");
        assert_eq!(restored_tabs.len(), 2);
        assert_eq!(active, 1);
        assert_eq!(restored_tabs[0].title, "example.rs");
        assert_eq!(restored_tabs[1].content.text(), "Unsaved quick notes");
        assert!(restored_tabs[1].is_dirty);

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir);
    }
    #[test]
    fn previous_generation_and_its_drafts_remain_recoverable() {
        let root = env::temp_dir().join(crate::storage::unique_id());
        let drafts = root.join("drafts");
        let mut tab = Tab::new_empty(0);
        tab.content = iced::widget::text_editor::Content::with_text("first");
        tab.is_dirty = true;
        save_session_to(&[tab], 0, &root, &drafts).unwrap();
        let old: SessionRecord =
            serde_json::from_slice(&fs::read(root.join("session.json")).unwrap()).unwrap();
        let (mut tabs, _) = load_session_from(&root, &drafts).unwrap().unwrap();
        tabs[0].content = iced::widget::text_editor::Content::with_text("second");
        save_session_to(&tabs, 0, &root, &drafts).unwrap();
        assert_eq!(
            fs::read_to_string(drafts.join(old.tabs[0].draft_file.as_ref().unwrap())).unwrap(),
            "first"
        );
        assert_eq!(
            load_session_from(&root, &drafts).unwrap().unwrap().0[0]
                .content
                .text(),
            "second"
        );
        assert!(root.join("session.previous.json").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_draft_write_preserves_committed_session() {
        let root = env::temp_dir().join(crate::storage::unique_id());
        let drafts = root.join("drafts");
        let mut tab = Tab::new_empty(0);
        tab.content = iced::widget::text_editor::Content::with_text("safe");
        tab.is_dirty = true;
        save_session_to(&[tab], 0, &root, &drafts).unwrap();
        let before = fs::read(root.join("session.json")).unwrap();
        let (tabs, _) = load_session_from(&root, &drafts).unwrap().unwrap();
        let obstruction = root.join("not-a-directory");
        fs::write(&obstruction, b"blocked").unwrap();
        assert!(save_session_to(&tabs, 0, &root, &obstruction).is_err());
        assert_eq!(fs::read(root.join("session.json")).unwrap(), before);
        assert_eq!(
            load_session_from(&root, &drafts).unwrap().unwrap().0[0]
                .content
                .text(),
            "safe"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_draft_is_an_error_instead_of_silent_data_loss() {
        let root = env::temp_dir().join(crate::storage::unique_id());
        let drafts = root.join("drafts");
        let tab = Tab::new_empty(0);
        save_session_to(&[tab], 0, &root, &drafts).unwrap();
        fs::remove_dir_all(&drafts).unwrap();
        assert!(load_session_from(&root, &drafts).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
