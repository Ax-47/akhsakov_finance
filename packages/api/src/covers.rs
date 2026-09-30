//! Cover images for portfolios. Each is one small image file in a `covers`
//! folder next to the database, so it doesn't need a table (and backups,
//! which copy only the database, stay small). The app resizes an image
//! before sending it; the server only checks its type and size.

use dioxus::prelude::*;
use uuid::Uuid;

/// Largest image accepted, after the app has resized it.
pub const MAX_COVER_BYTES: usize = 1_500_000;

/// The image types accepted, as `(MIME type, file extension)`.
#[cfg(feature = "server")]
const TYPES: [(&str, &str); 3] = [("image/jpeg", "jpg"), ("image/png", "png"), ("image/webp", "webp")];

/// The portfolio's cover as a `data:` URL, or `None` if it has none.
#[post("/api/portfolios/cover")]
pub async fn get_portfolio_cover(id: Uuid) -> Result<Option<String>, ServerFnError> {
    Ok(store::get(&store::dir(), id))
}

/// Sets the cover from a `data:image/…;base64,…` URL.
#[post("/api/portfolios/cover/set")]
pub async fn set_portfolio_cover(id: Uuid, data_url: String) -> Result<(), ServerFnError> {
    store::set(&store::dir(), id, &data_url).map_err(ServerFnError::new)
}

#[post("/api/portfolios/cover/remove")]
pub async fn remove_portfolio_cover(id: Uuid) -> Result<(), ServerFnError> {
    store::remove(&store::dir(), id);
    Ok(())
}

#[cfg(feature = "server")]
pub(crate) mod store {
    use super::{MAX_COVER_BYTES, TYPES};
    use base64::{engine::general_purpose::STANDARD, Engine};
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    /// `covers/` beside the database file.
    pub fn dir() -> PathBuf {
        crate::database::Database::default_path()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
            .join("covers")
    }

    fn files(dir: &Path, id: Uuid) -> impl Iterator<Item = (PathBuf, &'static str)> + '_ {
        TYPES.iter().map(move |(mime, ext)| (dir.join(format!("{id}.{ext}")), *mime))
    }

    pub fn get(dir: &Path, id: Uuid) -> Option<String> {
        files(dir, id).find_map(|(path, mime)| {
            let bytes = std::fs::read(path).ok()?;
            Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
        })
    }

    pub fn set(dir: &Path, id: Uuid, data_url: &str) -> Result<(), String> {
        let (head, body) = data_url
            .strip_prefix("data:")
            .and_then(|rest| rest.split_once(";base64,"))
            .ok_or("That isn't an image.")?;
        let ext = TYPES
            .iter()
            .find(|(mime, _)| *mime == head)
            .map(|(_, ext)| *ext)
            .ok_or("Use a JPEG, PNG or WebP image.")?;
        let bytes = STANDARD.decode(body).map_err(|_| "That image is damaged.")?;
        if bytes.len() > MAX_COVER_BYTES {
            return Err("That image is too large.".into());
        }
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        remove(dir, id);
        std::fs::write(dir.join(format!("{id}.{ext}")), bytes).map_err(|e| e.to_string())
    }

    pub fn remove(dir: &Path, id: Uuid) {
        for (path, _) in files(dir, id) {
            let _ = std::fs::remove_file(path);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn set_replace_get_remove() {
            let dir = std::env::temp_dir().join(format!("akhsakov-covers-{}", Uuid::new_v4()));
            let id = Uuid::new_v4();
            assert_eq!(get(&dir, id), None);

            let png = format!("data:image/png;base64,{}", STANDARD.encode([1, 2, 3]));
            set(&dir, id, &png).unwrap();
            assert_eq!(get(&dir, id).as_deref(), Some(png.as_str()));

            // A new cover in another format replaces the old one.
            let jpeg = format!("data:image/jpeg;base64,{}", STANDARD.encode([4, 5]));
            set(&dir, id, &jpeg).unwrap();
            assert_eq!(get(&dir, id).as_deref(), Some(jpeg.as_str()));

            remove(&dir, id);
            assert_eq!(get(&dir, id), None);
            let _ = std::fs::remove_dir_all(dir);
        }

        #[test]
        fn rejects_other_files() {
            let dir = std::env::temp_dir().join(format!("akhsakov-covers-{}", Uuid::new_v4()));
            let id = Uuid::new_v4();
            assert!(set(&dir, id, "hello").is_err());
            assert!(set(&dir, id, "data:image/svg+xml;base64,PHN2Zz4=").is_err(), "SVG can carry scripts");
            assert!(set(&dir, id, "data:image/png;base64,***").is_err());
            let big = format!("data:image/png;base64,{}", STANDARD.encode(vec![0u8; MAX_COVER_BYTES + 1]));
            assert!(set(&dir, id, &big).is_err());
            assert_eq!(get(&dir, id), None);
        }
    }
}
