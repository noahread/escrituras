use anyhow::{anyhow, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use ndarray::{Array2, ArrayView1};
use ndarray_npy::ReadNpyExt;
use serde::Deserialize;
use std::cmp::Ordering;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Mutex;

/// Download the embedding model for semantic search (called during installation)
pub fn download_embedding_model() -> Result<()> {
    println!("Downloading embedding model for semantic search...");

    // Show progress since we're not in TUI mode
    let options = InitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(true);

    TextEmbedding::try_new(options).map_err(|e| anyhow!("Failed to download model: {}", e))?;

    println!("✓ Embedding model cached successfully");
    Ok(())
}

#[derive(Deserialize)]
struct Metadata {
    verse_title: String,
}

/// Embeddings database for semantic search using local ONNX model
pub struct EmbeddingsDb {
    /// One row per verse, normalized to unit length at load time so that a
    /// dot product with a normalized query is its cosine similarity.
    embeddings: Array2<f32>,
    verse_titles: Vec<String>,
    /// Lazily loaded on first query. Behind a mutex because fastembed needs
    /// `&mut` to embed, while searches only need shared access to the db.
    model: Mutex<Option<TextEmbedding>>,
}

impl EmbeddingsDb {
    /// Load embeddings from .npy file and metadata from JSON
    pub fn load(data_dir: &Path) -> Result<Self> {
        let embeddings_path = data_dir.join("scripture_embeddings.npy");
        let metadata_path = data_dir.join("scripture_metadata.json");

        // Load embeddings from .npy file
        let embeddings_file = File::open(&embeddings_path).map_err(|e| {
            anyhow!(
                "Failed to open embeddings file {:?}: {}",
                embeddings_path,
                e
            )
        })?;
        let embeddings: Array2<f32> = Array2::read_npy(embeddings_file)
            .map_err(|e| anyhow!("Failed to read .npy file: {}", e))?;

        // Load metadata from JSON
        let metadata_file = File::open(&metadata_path)
            .map_err(|e| anyhow!("Failed to open metadata file {:?}: {}", metadata_path, e))?;
        let metadata: Vec<Metadata> = serde_json::from_reader(BufReader::new(metadata_file))?;

        let verse_titles: Vec<String> = metadata.into_iter().map(|m| m.verse_title).collect();

        if embeddings.nrows() != verse_titles.len() {
            return Err(anyhow!(
                "Embeddings count ({}) doesn't match metadata count ({})",
                embeddings.nrows(),
                verse_titles.len()
            ));
        }

        Ok(Self::from_parts(embeddings, verse_titles))
    }

    fn from_parts(mut embeddings: Array2<f32>, verse_titles: Vec<String>) -> Self {
        for mut row in embeddings.rows_mut() {
            normalize(row.as_slice_mut().expect("npy rows are contiguous"));
        }
        Self {
            embeddings,
            verse_titles,
            model: Mutex::new(None),
        }
    }

    /// Embed query text using local ONNX model, loading the model on first use
    pub fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        let mut model = self
            .model
            .lock()
            .map_err(|_| anyhow!("Embedding model lock poisoned"))?;

        if model.is_none() {
            // Model will be downloaded to ~/.cache/fastembed/ on first use (~33MB)
            // Disable download progress to avoid corrupting TUI display
            let options =
                InitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(false);
            *model = Some(
                TextEmbedding::try_new(options)
                    .map_err(|e| anyhow!("Failed to load embedding model: {}", e))?,
            );
        }

        let embeddings = model
            .as_mut()
            .expect("model initialized above")
            .embed(vec![text], None)
            .map_err(|e| anyhow!("Failed to embed query: {}", e))?;

        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No embedding returned"))
    }

    /// Find verses semantically similar to query
    /// Returns (verse_title, similarity_score) pairs sorted by similarity (highest first)
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<(String, f32)>> {
        let query_emb = self.embed_query(query)?;
        Ok(self.rank(query_emb, limit))
    }

    /// Rank all verses by cosine similarity to an embedded query
    fn rank(&self, mut query_emb: Vec<f32>, limit: usize) -> Vec<(String, f32)> {
        normalize(&mut query_emb);
        let scores = self.embeddings.dot(&ArrayView1::from(&query_emb));

        let mut ranked: Vec<(usize, f32)> = scores.iter().copied().enumerate().collect();
        let by_score_desc =
            |a: &(usize, f32), b: &(usize, f32)| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal);

        // Partition out the top `limit` before sorting just those
        if limit < ranked.len() {
            ranked.select_nth_unstable_by(limit, by_score_desc);
            ranked.truncate(limit);
        }
        ranked.sort_by(by_score_desc);

        ranked
            .into_iter()
            .map(|(i, score)| (self.verse_titles[i].clone(), score))
            .collect()
    }
}

/// Scale a vector to unit length (zero vectors are left unchanged)
fn normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    fn test_db() -> EmbeddingsDb {
        EmbeddingsDb::from_parts(
            array![
                [1.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [-3.0, 0.0, 0.0],
                [0.0, 0.0, 0.0]
            ],
            vec!["a".into(), "b".into(), "c".into(), "zero".into()],
        )
    }

    #[test]
    fn test_normalize() {
        let mut v = vec![3.0, 4.0];
        normalize(&mut v);
        assert!((v[0] - 0.6).abs() < 0.0001);
        assert!((v[1] - 0.8).abs() < 0.0001);
    }

    #[test]
    fn test_normalize_zero_vector() {
        let mut v = vec![0.0, 0.0];
        normalize(&mut v);
        assert_eq!(v, [0.0, 0.0]);
    }

    #[test]
    fn test_rank_orders_by_cosine_similarity() {
        let db = test_db();
        // Query is not unit length; ranking must be by cosine, not raw dot product
        let ranked = db.rank(vec![2.0, 1.0, 0.0], 4);
        let titles: Vec<&str> = ranked.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(titles, ["a", "b", "zero", "c"]);
        assert!((ranked[0].1 - 2.0 / 5f32.sqrt()).abs() < 0.0001);
        assert!((ranked[3].1 + 2.0 / 5f32.sqrt()).abs() < 0.0001);
    }

    #[test]
    fn test_rank_respects_limit() {
        let db = test_db();
        let ranked = db.rank(vec![0.0, 1.0, 0.0], 1);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].0, "b");
        assert!((ranked[0].1 - 1.0).abs() < 0.0001);
    }
}
