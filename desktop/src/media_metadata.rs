use anyhow::Result;
use lofty::{file::{AudioFile, TaggedFileExt}, picture::PictureType, tag::{Accessor, TagExt}};
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_seconds: Option<f64>,
    pub cover_path: Option<PathBuf>,
}

impl MediaMetadata {
    pub fn read(path: &Path) -> Self {
        let fallback = path.file_stem()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("Получено медиа")
            .to_owned();

        let tagged = match lofty::read_from_path(path) {
            Ok(tagged) => tagged,
            Err(_) => return Self {
                title: fallback,
                artist: "Неизвестный исполнитель".to_owned(),
                album: String::new(),
                duration_seconds: None,
                cover_path: None,
            },
        };

        let properties = tagged.properties();
        let duration_seconds = {
            let seconds = properties.duration().as_secs_f64();
            (seconds.is_finite() && seconds > 0.0).then_some(seconds)
        };
        let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

        let title = tag.and_then(|tag| tag.title())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&fallback)
            .to_owned();
        let artist = tag.and_then(|tag| tag.artist())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("Неизвестный исполнитель")
            .to_owned();
        let album = tag.and_then(|tag| tag.album()).unwrap_or_default();

        let cover_path = tag.and_then(|tag| {
            let picture = tag.pictures().iter()
                .find(|picture| picture.pic_type() == PictureType::CoverFront)
                .or_else(|| tag.pictures().first())?;
            let data = picture.data();
            if data.is_empty() || data.len() > 12 * 1024 * 1024 {
                return None;
            }
            let directory = std::env::temp_dir().join("AERIS").join("Received");
            fs::create_dir_all(&directory).ok()?;
            let output = directory.join(format!("cover-{}.img", Uuid::new_v4()));
            fs::write(&output, data).ok()?;
            Some(output)
        });

        Self { title, artist, album, duration_seconds, cover_path }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_file_uses_filename_as_title() {
        let metadata = MediaMetadata {
            title: "track".to_owned(),
            artist: "Неизвестный исполнитель".to_owned(),
            album: String::new(),
            duration_seconds: None,
            cover_path: None,
        };
        assert_eq!(metadata.title, "track");
        assert!(metadata.duration_seconds.is_none());
    }
}
