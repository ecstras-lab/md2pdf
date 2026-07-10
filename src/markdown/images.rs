//! Finds the images a note embeds, and turns every other embed down by name.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Raster and vector formats Typst can decode.
const IMAGE_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "svg", "webp"];

/// Recognised only so an embed can be turned down for what it is rather than
/// for being missing.
const VIDEO_EXTENSIONS: [&str; 6] = ["mp4", "mov", "webm", "mkv", "avi", "m4v"];
const AUDIO_EXTENSIONS: [&str; 5] = ["mp3", "wav", "ogg", "m4a", "flac"];

pub(super) struct Images {
    base_dir: PathBuf,
    working_dir: PathBuf,
    pub(super) files: Vec<(String, Vec<u8>)>,
    resolved: HashMap<String, String>,
}

impl Images {
    pub(super) fn new(base_dir: &Path) -> Self {
        Self {
            base_dir: base_dir.to_path_buf(),
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            files: Vec::new(),
            resolved: HashMap::new(),
        }
    }

    /// Loads a local image and returns the virtual path Typst should read.
    pub(super) fn resolve(
        &mut self,
        source: &str,
    ) -> Result<String, String> {
        if let Some(existing) = self.resolved.get(source) {
            return Ok(existing.clone());
        }

        if source.starts_with("http://") || source.starts_with("https://") {
            return Err(format!("remote image not embedded: {source}"));
        }

        if source.starts_with("data:") {
            return Err(format!("inline data URI not embedded: {source}"));
        }

        // Judge the format from the name, so a video or a bare `![[Note]]`
        // is turned down for what it is rather than for being missing.
        let extension = Path::new(source)
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();

        if !IMAGE_EXTENSIONS.contains(&extension.as_str()) {
            return Err(match extension.as_str() {
                "" => format!("note embeds are not supported: {source}"),
                extension if VIDEO_EXTENSIONS.contains(&extension) => {
                    format!("video embeds are not supported: {source}")
                }
                extension if AUDIO_EXTENSIONS.contains(&extension) => {
                    format!("audio embeds are not supported: {source}")
                }
                extension => format!(".{extension} is not an embeddable image: {source}"),
            });
        }

        let found = self
            .candidates(source)
            .into_iter()
            .find(|path| path.is_file())
            .ok_or_else(|| format!("image not found: {source}"))?;

        let bytes =
            std::fs::read(&found).map_err(|error| format!("{}: {error}", found.display()))?;

        let virtual_path = format!("/images/{}.{extension}", self.files.len());
        self.files.push((virtual_path.clone(), bytes));
        self.resolved
            .insert(source.to_owned(), virtual_path.clone());

        Ok(virtual_path)
    }

    fn candidates(
        &self,
        source: &str,
    ) -> Vec<PathBuf> {
        let normalized = source.replace('\\', "/");
        let decoded = urlencoding::decode(&normalized)
            .map(|value| value.into_owned())
            .unwrap_or_else(|_| normalized.clone());

        let mut names = vec![source.to_owned(), normalized, decoded];
        names.dedup();

        names
            .iter()
            .flat_map(|name| [self.base_dir.join(name), self.working_dir.join(name)])
            .collect()
    }
}
