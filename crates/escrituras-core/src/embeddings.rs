use anyhow::{anyhow, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use ndarray::Array2;
use ndarray_npy::ReadNpyExt;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Download the embedding model for semantic search (called during installation)
pub fn download_embedding_model() -> Result<()> {
    println!("Downloading embedding model for semantic search...");

    // Show progress since we're not in TUI mode
    let options = InitOptions::new(EmbeddingModel::BGESmallENV15)
        .with_show_download_progress(true);

    TextEmbedding::try_new(options)
        .map_err(|e| anyhow!("Failed to download model: {}", e))?;

    println!("✓ Embedding model cached successfully");
    Ok(())
}

#[derive(Deserialize)]
struct Metadata {
    verse_title: String,
}

/// Embeddings database for semantic search using local ONNX model
pub struct EmbeddingsDb {
    embeddings: Array2<f32>,
    verse_titles: Vec<String>,
    model: Option<TextEmbedding>,
}

impl EmbeddingsDb {
    /// Load embeddings from .npy file and metadata from JSON
    pub fn load(data_dir: &Path) -> Result<Self> {
        let embeddings_path = data_dir.join("scripture_embeddings.npy");
        let metadata_path = data_dir.join("scripture_metadata.json");

        // Load embeddings from .npy file
        let embeddings_file = File::open(&embeddings_path)
            .map_err(|e| anyhow!("Failed to open embeddings file {:?}: {}", embeddings_path, e))?;
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

        Ok(Self {
            embeddings,
            verse_titles,
            model: None,
        })
    }

    /// Initialize the embedding model (lazy-loaded on first query)
    fn ensure_model(&mut self) -> Result<()> {
        if self.model.is_none() {
            // Model will be downloaded to ~/.cache/fastembed/ on first use (~33MB)
            // Disable download progress to avoid corrupting TUI display
            let options = InitOptions::new(EmbeddingModel::BGESmallENV15)
                .with_show_download_progress(false);
            self.model = Some(
                TextEmbedding::try_new(options)
                    .map_err(|e| anyhow!("Failed to load embedding model: {}", e))?,
            );
        }
        Ok(())
    }

    /// Embed query text using local ONNX model
    pub fn embed_query(&mut self, text: &str) -> Result<Vec<f32>> {
        self.ensure_model()?;

        let model = self.model.as_mut().unwrap();
        let embeddings = model
            .embed(vec![text], None)
            .map_err(|e| anyhow!("Failed to embed query: {}", e))?;

        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No embedding returned"))
    }

    /// Find verses semantically similar to query
    /// Returns (verse_title, similarity_score) pairs sorted by similarity (highest first)
    pub fn search(&mut self, query: &str, limit: usize) -> Result<Vec<(String, f32)>> {
        let query_emb = self.embed_query(query)?;

        // Compute cosine similarity against all embeddings
        let mut scores: Vec<(usize, f32)> = self
            .embeddings
            .rows()
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                let score = cosine_similarity(row.as_slice().unwrap(), &query_emb);
                (i, score)
            })
            .collect();

        // Sort by similarity (highest first)
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(limit);

        Ok(scores
            .into_iter()
            .map(|(i, score)| (self.verse_titles[i].clone(), score))
            .collect())
    }
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }
}
