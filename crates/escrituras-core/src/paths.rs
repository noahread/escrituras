//! Locating the scripture data and embeddings on disk.

use crate::embeddings::EmbeddingsDb;
use crate::scripture::ScriptureDb;
use anyhow::{bail, Result};
use std::path::PathBuf;

/// Scripture JSON, relative to a data root
pub const SCRIPTURES_FILE: &str = "lds-scriptures-2020.12.08/json/lds-scriptures-json.txt";
/// Directory holding the precomputed embeddings, relative to a data root
pub const EMBEDDINGS_DIR: &str = "data";
const EMBEDDINGS_FILE: &str = "scripture_embeddings.npy";

/// Where the data files were found
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataPaths {
    pub scriptures: PathBuf,
    /// `None` when no embeddings are installed (semantic search is optional)
    pub embeddings_dir: Option<PathBuf>,
}

impl DataPaths {
    /// Look in the current directory (a development checkout) and then in
    /// the installed location, `<config dir>/escrituras` (see install.sh).
    pub fn discover() -> Result<Self> {
        Self::find_in(&Self::default_roots())
    }

    /// The roots searched by [`DataPaths::discover`], in order
    pub fn default_roots() -> Vec<PathBuf> {
        let mut roots = vec![PathBuf::from(".")];
        if let Some(config_dir) = dirs::config_dir() {
            roots.push(config_dir.join("escrituras"));
        }
        roots
    }

    /// Search `roots` in order. The scriptures and the embeddings are
    /// located independently, so each comes from the first root that has it.
    /// App bundles can pass their resource directory here.
    pub fn find_in(roots: &[PathBuf]) -> Result<Self> {
        let Some(scriptures) = roots
            .iter()
            .map(|root| root.join(SCRIPTURES_FILE))
            .find(|path| path.exists())
        else {
            bail!("Scripture data not found. Run install.sh or place data in lds-scriptures-2020.12.08/");
        };

        let embeddings_dir = roots
            .iter()
            .map(|root| root.join(EMBEDDINGS_DIR))
            .find(|dir| dir.join(EMBEDDINGS_FILE).exists());

        Ok(Self {
            scriptures,
            embeddings_dir,
        })
    }

    /// Load the scripture database
    pub async fn load_scriptures(&self) -> Result<ScriptureDb> {
        let mut db = ScriptureDb::new();
        db.load_from_json(&self.scriptures).await?;
        Ok(db)
    }

    /// Load the embeddings if they are installed and readable
    pub fn load_embeddings(&self) -> Option<EmbeddingsDb> {
        self.embeddings_dir
            .as_deref()
            .and_then(|dir| EmbeddingsDb::load(dir).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(path: PathBuf) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "").unwrap();
    }

    #[test]
    fn test_prefers_earlier_root() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        for root in [first.path(), second.path()] {
            touch(root.join(SCRIPTURES_FILE));
            touch(root.join(EMBEDDINGS_DIR).join(EMBEDDINGS_FILE));
        }

        let paths =
            DataPaths::find_in(&[first.path().to_path_buf(), second.path().to_path_buf()]).unwrap();
        assert_eq!(paths.scriptures, first.path().join(SCRIPTURES_FILE));
        assert_eq!(
            paths.embeddings_dir,
            Some(first.path().join(EMBEDDINGS_DIR))
        );
    }

    #[test]
    fn test_finds_files_in_different_roots() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        touch(first.path().join(SCRIPTURES_FILE));
        touch(second.path().join(EMBEDDINGS_DIR).join(EMBEDDINGS_FILE));

        let paths =
            DataPaths::find_in(&[first.path().to_path_buf(), second.path().to_path_buf()]).unwrap();
        assert_eq!(paths.scriptures, first.path().join(SCRIPTURES_FILE));
        assert_eq!(
            paths.embeddings_dir,
            Some(second.path().join(EMBEDDINGS_DIR))
        );
    }

    #[test]
    fn test_embeddings_are_optional() {
        let root = tempfile::tempdir().unwrap();
        touch(root.path().join(SCRIPTURES_FILE));

        let paths = DataPaths::find_in(&[root.path().to_path_buf()]).unwrap();
        assert_eq!(paths.embeddings_dir, None);
    }

    #[test]
    fn test_missing_scriptures_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        touch(root.path().join(EMBEDDINGS_DIR).join(EMBEDDINGS_FILE));

        assert!(DataPaths::find_in(&[root.path().to_path_buf()]).is_err());
    }
}
