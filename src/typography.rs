use iced::Font;

/// macOS interface face used by every UI surface outside the document buffer.
pub const UI: Font = Font {
    weight: iced::font::Weight::Medium,
    ..Font::with_name("Helvetica Neue")
};
pub const MONO: Font = Font::with_name("JetBrains Mono");

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_faces_share_the_expected_monospace_family() {
        use cosmic_text::fontdb::{Database, Source};
        let mut db = Database::new();
        let faces: [&[u8]; 4] = [
            include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
            include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf"),
            include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf"),
            include_bytes!("../assets/fonts/JetBrainsMono-BoldItalic.ttf"),
        ];
        for bytes in faces {
            let ids = db.load_font_source(Source::Binary(std::sync::Arc::new(bytes.to_vec())));
            assert_eq!(ids.len(), 1);
            let face = db.face(ids[0]).unwrap();
            assert!(face.monospaced);
            assert!(
                face.families
                    .iter()
                    .any(|(name, _)| name == "JetBrains Mono")
            );
        }
    }
}
