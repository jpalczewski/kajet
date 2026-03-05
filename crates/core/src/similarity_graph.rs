use anyhow::{Context, Result, ensure};
use memmap2::Mmap;
use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub const KJSG_MAGIC: [u8; 4] = *b"KJSG";
pub const KJSG_VERSION: u16 = 1;
const KJSG_HEADER_BYTES: usize = 16;
#[allow(dead_code)]
const CHUNK_ENTRY_BYTES: usize = 20; // 4+2+4+2+2+4+2

/// Read-side interface — implemented by `CsrGraph`, mockable in tests.
pub trait SimilarityGraph: Send + Sync {
    /// Return K nearest neighbors for chunk at given dense index.
    /// Returns slice of (neighbor_index, similarity_score) sorted by similarity desc.
    fn neighbors(&self, chunk_idx: u32) -> &[(u32, f32)];

    /// Total number of chunks in the graph.
    fn len(&self) -> u32;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Mapping: dense chunk index → document index.
    fn chunk_to_doc(&self, chunk_idx: u32) -> u16;

    // Identity (KJSG v2 only) — defaults for v1 backward compat
    fn has_identity(&self) -> bool {
        false
    }
    fn chunk_note_path(&self, _chunk_idx: u32) -> &str {
        ""
    }
    fn chunk_breadcrumb(&self, _chunk_idx: u32) -> &str {
        ""
    }
    fn chunk_excerpt(&self, _chunk_idx: u32) -> &str {
        ""
    }
    fn chunk_index_in_doc(&self, _chunk_idx: u32) -> u16 {
        0
    }
}

/// Build-side interface — takes embeddings, produces a serializable graph.
pub trait GraphBuilder: Send + Sync {
    /// Build KNN graph from embedding matrix.
    /// embeddings: row-major [N × D] flat array, L2-normalized.
    /// Returns serializable graph.
    fn build(
        &self,
        embeddings: &[f32], // flat [N * D]
        n_chunks: u32,
        dim: u32,
        k: u32,               // neighbors per chunk
        chunk_to_doc: &[u16], // mapping chunk → document
    ) -> Result<CsrGraph>;
}

/// Per-chunk identity data stored in KJSG v2.
/// Offsets point into the string table.
#[derive(Debug, Clone)]
pub struct ChunkEntry {
    pub note_path_offset: u32,
    pub note_path_len: u16,
    pub breadcrumb_offset: u32,
    pub breadcrumb_len: u16,
    pub chunk_index: u16,
    pub content_offset: u32,
    pub content_len: u16,
}

#[derive(Debug, Clone)]
pub struct CsrGraph {
    pub k: u32,
    pub n_chunks: u32,
    pub dim: u32,
    offsets: Vec<u32>,
    adj: Vec<(u32, f32)>,
    chunk_to_doc: Vec<u16>,
    string_table: Vec<u8>,          // flat UTF-8 string data
    chunk_entries: Vec<ChunkEntry>, // per-chunk offsets into string_table
}

impl SimilarityGraph for CsrGraph {
    fn neighbors(&self, chunk_idx: u32) -> &[(u32, f32)] {
        if chunk_idx >= self.n_chunks {
            debug_assert!(
                chunk_idx < self.n_chunks,
                "chunk_idx out of bounds: {chunk_idx} >= {}",
                self.n_chunks
            );
            return &[];
        }

        let idx = chunk_idx as usize;
        let start = self.offsets[idx] as usize;
        let end = self.offsets[idx + 1] as usize;
        &self.adj[start..end]
    }

    fn len(&self) -> u32 {
        self.n_chunks
    }

    fn chunk_to_doc(&self, chunk_idx: u32) -> u16 {
        if chunk_idx >= self.n_chunks {
            debug_assert!(
                chunk_idx < self.n_chunks,
                "chunk_idx out of bounds: {chunk_idx} >= {}",
                self.n_chunks
            );
            return 0;
        }
        self.chunk_to_doc[chunk_idx as usize]
    }

    fn has_identity(&self) -> bool {
        !self.chunk_entries.is_empty()
    }

    fn chunk_note_path(&self, chunk_idx: u32) -> &str {
        self.chunk_entries
            .get(chunk_idx as usize)
            .map(|e| self.read_str(e.note_path_offset, e.note_path_len))
            .unwrap_or("")
    }

    fn chunk_breadcrumb(&self, chunk_idx: u32) -> &str {
        self.chunk_entries
            .get(chunk_idx as usize)
            .map(|e| self.read_str(e.breadcrumb_offset, e.breadcrumb_len))
            .unwrap_or("")
    }

    fn chunk_excerpt(&self, chunk_idx: u32) -> &str {
        self.chunk_entries
            .get(chunk_idx as usize)
            .map(|e| self.read_str(e.content_offset, e.content_len))
            .unwrap_or("")
    }

    fn chunk_index_in_doc(&self, chunk_idx: u32) -> u16 {
        self.chunk_entries
            .get(chunk_idx as usize)
            .map(|e| e.chunk_index)
            .unwrap_or(0)
    }
}

impl CsrGraph {
    pub fn new_fixed_k(
        k: u32,
        n_chunks: u32,
        dim: u32,
        offsets: Vec<u32>,
        adj: Vec<(u32, f32)>,
        chunk_to_doc: Vec<u16>,
    ) -> Result<Self> {
        let graph = Self {
            k,
            n_chunks,
            dim,
            offsets,
            adj,
            chunk_to_doc,
            string_table: Vec::new(),
            chunk_entries: Vec::new(),
        };
        graph.validate_fixed_k()?;
        Ok(graph)
    }

    pub fn with_identity(mut self, string_table: Vec<u8>, chunk_entries: Vec<ChunkEntry>) -> Self {
        self.string_table = string_table;
        self.chunk_entries = chunk_entries;
        self
    }

    pub fn has_identity(&self) -> bool {
        !self.chunk_entries.is_empty()
    }

    fn read_str(&self, offset: u32, len: u16) -> &str {
        let start = offset as usize;
        let end = start + len as usize;
        if end <= self.string_table.len() {
            std::str::from_utf8(&self.string_table[start..end]).unwrap_or("")
        } else {
            ""
        }
    }

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        self.validate_fixed_k()?;

        let k_u16 = u16::try_from(self.k).context("Similarity graph K too large")?;
        let version: u16 = if self.chunk_entries.is_empty() { 1 } else { 2 };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Failed to create similarity graph output directory '{}'",
                    parent.display()
                )
            })?;
        }

        let tmp_path = path.with_extension("kjsg.tmp");
        let mut file = File::create(&tmp_path).with_context(|| {
            format!(
                "Failed to create similarity graph temp file '{}'",
                tmp_path.display()
            )
        })?;

        // Header (16 bytes)
        file.write_all(&KJSG_MAGIC)?;
        file.write_all(&version.to_le_bytes())?;
        file.write_all(&k_u16.to_le_bytes())?;
        file.write_all(&self.n_chunks.to_le_bytes())?;
        file.write_all(&self.dim.to_le_bytes())?;

        // v1 body
        for offset in &self.offsets {
            file.write_all(&offset.to_le_bytes())?;
        }
        for (neighbor, _) in &self.adj {
            file.write_all(&neighbor.to_le_bytes())?;
        }
        for (_, sim) in &self.adj {
            file.write_all(&sim.to_le_bytes())?;
        }
        for doc in &self.chunk_to_doc {
            file.write_all(&doc.to_le_bytes())?;
        }

        // v2 extension: string table + chunk entries
        if version == 2 {
            let str_len = self.string_table.len() as u32;
            file.write_all(&str_len.to_le_bytes())?;
            file.write_all(&self.string_table)?;

            for entry in &self.chunk_entries {
                file.write_all(&entry.note_path_offset.to_le_bytes())?;
                file.write_all(&entry.note_path_len.to_le_bytes())?;
                file.write_all(&entry.breadcrumb_offset.to_le_bytes())?;
                file.write_all(&entry.breadcrumb_len.to_le_bytes())?;
                file.write_all(&entry.chunk_index.to_le_bytes())?;
                file.write_all(&entry.content_offset.to_le_bytes())?;
                file.write_all(&entry.content_len.to_le_bytes())?;
            }
        }

        drop(file);
        std::fs::rename(&tmp_path, path).with_context(|| {
            format!(
                "Failed to move similarity graph into place: '{}' -> '{}'",
                tmp_path.display(),
                path.display()
            )
        })?;

        Ok(())
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("Failed to open similarity graph '{}'", path.display()))?;
        let mmap = unsafe { Mmap::map(&file)? };
        let data: &[u8] = &mmap;

        ensure!(
            data.len() >= KJSG_HEADER_BYTES,
            "Similarity graph file too small ({} bytes)",
            data.len()
        );

        let mut cursor: usize = 0;
        let magic = <[u8; 4]>::try_from(&data[cursor..cursor + 4])?;
        cursor += 4;
        ensure!(magic == KJSG_MAGIC, "Invalid KJSG magic: {magic:?}");

        let version = read_u16_le(data, &mut cursor)?;
        ensure!(
            version == 1 || version == 2,
            "Unsupported KJSG version: {version}"
        );

        let k = read_u16_le(data, &mut cursor)? as u32;
        let n_chunks = read_u32_le(data, &mut cursor)?;
        let dim = read_u32_le(data, &mut cursor)?;

        let n = n_chunks as usize;
        let k_usize = k as usize;

        let v1_body_bytes = 4 * (n + 1) // offsets
            + 4 * (n * k_usize) // neighbors
            + 4 * (n * k_usize) // similarities
            + 2 * n; // chunk_to_doc

        ensure!(
            data.len() >= KJSG_HEADER_BYTES + v1_body_bytes,
            "Similarity graph file size mismatch: expected at least {} bytes, got {} bytes",
            KJSG_HEADER_BYTES + v1_body_bytes,
            data.len()
        );

        let offsets = read_u32_vec_le(data, &mut cursor, n + 1)?;
        let neighbors = read_u32_vec_le(data, &mut cursor, n * k_usize)?;
        let similarities = read_f32_vec_le(data, &mut cursor, n * k_usize)?;
        let chunk_to_doc = read_u16_vec_le(data, &mut cursor, n)?;

        let mut adj = Vec::with_capacity(neighbors.len());
        for idx in 0..neighbors.len() {
            adj.push((neighbors[idx], similarities[idx]));
        }

        let (string_table, chunk_entries) = if version >= 2 {
            let str_len = read_u32_le(data, &mut cursor)? as usize;
            ensure!(
                cursor + str_len <= data.len(),
                "Similarity graph string table exceeds file bounds"
            );
            let st = data[cursor..cursor + str_len].to_vec();
            cursor += str_len;

            let mut entries = Vec::with_capacity(n);
            for _ in 0..n {
                let note_path_offset = read_u32_le(data, &mut cursor)?;
                let note_path_len = read_u16_le(data, &mut cursor)?;
                let breadcrumb_offset = read_u32_le(data, &mut cursor)?;
                let breadcrumb_len = read_u16_le(data, &mut cursor)?;
                let chunk_index = read_u16_le(data, &mut cursor)?;
                let content_offset = read_u32_le(data, &mut cursor)?;
                let content_len = read_u16_le(data, &mut cursor)?;
                entries.push(ChunkEntry {
                    note_path_offset,
                    note_path_len,
                    breadcrumb_offset,
                    breadcrumb_len,
                    chunk_index,
                    content_offset,
                    content_len,
                });
            }
            (st, entries)
        } else {
            (Vec::new(), Vec::new())
        };

        ensure!(cursor == data.len(), "Trailing bytes in similarity graph");

        let graph = Self {
            k,
            n_chunks,
            dim,
            offsets,
            adj,
            chunk_to_doc,
            string_table,
            chunk_entries,
        };
        graph.validate_fixed_k()?;
        Ok(graph)
    }

    fn validate_fixed_k(&self) -> Result<()> {
        let n = self.n_chunks as usize;
        let k = self.k as usize;

        ensure!(self.offsets.len() == n + 1, "Invalid offsets length");
        ensure!(self.chunk_to_doc.len() == n, "Invalid chunk_to_doc length");
        ensure!(self.offsets[0] == 0, "Invalid offsets[0]");

        let expected_edges = n.saturating_mul(k);
        ensure!(
            self.adj.len() == expected_edges,
            "Invalid adjacency length: expected {expected_edges}, got {}",
            self.adj.len()
        );

        for i in 0..=n {
            let expected = (i * k) as u32;
            ensure!(
                self.offsets[i] == expected,
                "Invalid offsets[{i}]: expected {expected}, got {}",
                self.offsets[i]
            );
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ChunkMetadata {
    pub breadcrumb: String,
}

pub trait ChunkFilter: Send + Sync {
    /// Return false to exclude chunk from similarity graph.
    fn include(&self, chunk: &ChunkMetadata) -> bool;
}

pub fn heading_from_breadcrumb(breadcrumb: &str) -> Option<&str> {
    breadcrumb
        .rsplit_once(" > ")
        .map(|(_, heading)| heading.trim())
        .filter(|h| !h.is_empty())
}

pub struct HeadingRegexFilter {
    patterns: Vec<Regex>,
}

impl HeadingRegexFilter {
    pub fn new(patterns: &[String]) -> Result<Self> {
        let mut compiled = Vec::with_capacity(patterns.len());
        for pattern in patterns {
            compiled.push(
                Regex::new(pattern)
                    .with_context(|| format!("Invalid boilerplate regex pattern: '{pattern}'"))?,
            );
        }
        Ok(Self { patterns: compiled })
    }

    pub fn include_breadcrumb(&self, breadcrumb: &str) -> bool {
        let Some(heading) = heading_from_breadcrumb(breadcrumb) else {
            return true;
        };
        !self.patterns.iter().any(|re| re.is_match(heading))
    }
}

impl ChunkFilter for HeadingRegexFilter {
    fn include(&self, chunk: &ChunkMetadata) -> bool {
        self.include_breadcrumb(&chunk.breadcrumb)
    }
}

/// Stable identifier for a chunk inside LanceDB: `{note_path, chunk_index}`.
///
/// This is used for building a dense-index mapping when boilerplate filtering
/// removes some chunks from the similarity graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChunkKey {
    pub note_path: String,
    pub chunk_index: u32,
}

#[derive(Debug, Default, Clone)]
pub struct DenseChunkMapping {
    pub dense_to_chunk: Vec<ChunkKey>,
    pub chunk_to_dense: HashMap<ChunkKey, u32>,
}

impl DenseChunkMapping {
    pub fn new(dense_to_chunk: Vec<ChunkKey>) -> Self {
        let mut chunk_to_dense = HashMap::with_capacity(dense_to_chunk.len());
        for (dense_idx, key) in dense_to_chunk.iter().enumerate() {
            chunk_to_dense.insert(key.clone(), dense_idx as u32);
        }
        Self {
            dense_to_chunk,
            chunk_to_dense,
        }
    }

    pub fn dense_to_chunk(&self, dense_idx: u32) -> Option<&ChunkKey> {
        self.dense_to_chunk.get(dense_idx as usize)
    }

    pub fn chunk_to_dense(&self, key: &ChunkKey) -> Option<u32> {
        self.chunk_to_dense.get(key).copied()
    }
}

fn read_u16_le(data: &[u8], cursor: &mut usize) -> Result<u16> {
    ensure!(
        *cursor + 2 <= data.len(),
        "Unexpected EOF while reading u16"
    );
    let value = u16::from_le_bytes(data[*cursor..*cursor + 2].try_into()?);
    *cursor += 2;
    Ok(value)
}

fn read_u32_le(data: &[u8], cursor: &mut usize) -> Result<u32> {
    ensure!(
        *cursor + 4 <= data.len(),
        "Unexpected EOF while reading u32"
    );
    let value = u32::from_le_bytes(data[*cursor..*cursor + 4].try_into()?);
    *cursor += 4;
    Ok(value)
}

fn read_f32_le(data: &[u8], cursor: &mut usize) -> Result<f32> {
    ensure!(
        *cursor + 4 <= data.len(),
        "Unexpected EOF while reading f32"
    );
    let value = f32::from_le_bytes(data[*cursor..*cursor + 4].try_into()?);
    *cursor += 4;
    Ok(value)
}

fn read_u16_vec_le(data: &[u8], cursor: &mut usize, len: usize) -> Result<Vec<u16>> {
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        out.push(read_u16_le(data, cursor)?);
    }
    Ok(out)
}

fn read_u32_vec_le(data: &[u8], cursor: &mut usize, len: usize) -> Result<Vec<u32>> {
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        out.push(read_u32_le(data, cursor)?);
    }
    Ok(out)
}

fn read_f32_vec_le(data: &[u8], cursor: &mut usize, len: usize) -> Result<Vec<f32>> {
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        out.push(read_f32_le(data, cursor)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csr_graph_serialization_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("similarity_graph.kjsg");

        let k = 3_u32;
        let n_chunks = 10_u32;
        let dim = 384_u32;

        let mut offsets = Vec::with_capacity(n_chunks as usize + 1);
        for i in 0..=n_chunks {
            offsets.push(i * k);
        }

        let mut adj = Vec::new();
        for chunk_idx in 0..n_chunks {
            for n in 0..k {
                adj.push(((chunk_idx + n + 1) % n_chunks, 1.0 - n as f32 * 0.1));
            }
        }

        let chunk_to_doc: Vec<u16> = (0..n_chunks).map(|i| (i % 2) as u16).collect();

        let graph = CsrGraph::new_fixed_k(k, n_chunks, dim, offsets, adj, chunk_to_doc).unwrap();

        graph.save_to_path(&path).unwrap();
        let loaded = CsrGraph::load_from_path(&path).unwrap();

        assert_eq!(loaded.k, k);
        assert_eq!(loaded.n_chunks, n_chunks);
        assert_eq!(loaded.dim, dim);
        assert_eq!(loaded.offsets, graph.offsets);
        assert_eq!(loaded.adj, graph.adj);
        assert_eq!(loaded.chunk_to_doc, graph.chunk_to_doc);
    }

    #[test]
    fn heading_regex_filter_matches_exact_heading() {
        let filter = HeadingRegexFilter::new(&[r"^Historia zmian$".to_string()]).unwrap();
        assert!(!filter.include(&ChunkMetadata {
            breadcrumb: "note.md > Historia zmian".to_string()
        }));
        assert!(filter.include(&ChunkMetadata {
            breadcrumb: "note.md > Historia infekcji".to_string()
        }));
    }

    #[test]
    fn heading_regex_filter_empty_heading_included() {
        let filter = HeadingRegexFilter::new(&[r"^Notatki$".to_string()]).unwrap();
        assert!(filter.include(&ChunkMetadata {
            breadcrumb: "note.md".to_string()
        }));
    }

    #[test]
    fn dense_index_mapping_handles_filtered_chunks() {
        let all = (0..10_u32)
            .map(|i| ChunkKey {
                note_path: "rcm.md".to_string(),
                chunk_index: i,
            })
            .collect::<Vec<_>>();
        let filtered_out = all[3].clone();
        let dense_to_chunk = all
            .into_iter()
            .enumerate()
            .filter_map(|(idx, key)| if idx == 3 { None } else { Some(key) })
            .collect::<Vec<_>>();

        let mapping = DenseChunkMapping::new(dense_to_chunk);
        assert_eq!(mapping.dense_to_chunk.len(), 9);
        assert_eq!(mapping.dense_to_chunk(0).unwrap().chunk_index, 0);
        assert_eq!(mapping.chunk_to_dense(&filtered_out), None);
    }

    #[test]
    fn csr_graph_identity_methods() {
        let mut string_table = Vec::new();

        let note_path = "martinaise/rcm-report.md";
        let breadcrumb = "martinaise/rcm-report.md > Findings";
        let excerpt = "The body was found hanging from a tree behind the Whirling-in-Rags.";

        let np_offset = string_table.len() as u32;
        string_table.extend_from_slice(note_path.as_bytes());
        let np_len = note_path.len() as u16;

        let bc_offset = string_table.len() as u32;
        string_table.extend_from_slice(breadcrumb.as_bytes());
        let bc_len = breadcrumb.len() as u16;

        let ct_offset = string_table.len() as u32;
        string_table.extend_from_slice(excerpt.as_bytes());
        let ct_len = excerpt.len() as u16;

        let entry = ChunkEntry {
            note_path_offset: np_offset,
            note_path_len: np_len,
            breadcrumb_offset: bc_offset,
            breadcrumb_len: bc_len,
            chunk_index: 2,
            content_offset: ct_offset,
            content_len: ct_len,
        };

        let graph = CsrGraph::new_fixed_k(0, 1, 384, vec![0, 0], vec![], vec![0])
            .unwrap()
            .with_identity(string_table, vec![entry]);

        assert!(graph.has_identity());
        assert_eq!(graph.chunk_note_path(0), "martinaise/rcm-report.md");
        assert_eq!(
            graph.chunk_breadcrumb(0),
            "martinaise/rcm-report.md > Findings"
        );
        assert_eq!(
            graph.chunk_excerpt(0),
            "The body was found hanging from a tree behind the Whirling-in-Rags."
        );
        assert_eq!(graph.chunk_index_in_doc(0), 2);
    }

    #[test]
    fn csr_graph_identity_empty_returns_defaults() {
        let graph = CsrGraph::new_fixed_k(0, 1, 384, vec![0, 0], vec![], vec![0]).unwrap();
        assert!(!graph.has_identity());
        assert_eq!(graph.chunk_note_path(0), "");
        assert_eq!(graph.chunk_breadcrumb(0), "");
        assert_eq!(graph.chunk_excerpt(0), "");
        assert_eq!(graph.chunk_index_in_doc(0), 0);
    }

    #[test]
    fn csr_graph_v2_serialization_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph_v2.kjsg");

        let k = 2_u32;
        let n_chunks = 3_u32;
        let dim = 4_u32;

        let offsets = vec![0, 2, 4, 6];
        let adj = vec![
            (1_u32, 0.9_f32),
            (2, 0.7),
            (0, 0.9),
            (2, 0.8),
            (1, 0.8),
            (0, 0.7),
        ];
        let chunk_to_doc = vec![0_u16, 0, 1];

        let mut string_table = Vec::new();
        let mut entries = Vec::new();

        let chunks_data = [
            (
                "revachol/case.md",
                "revachol/case.md > Evidence",
                "The murder weapon was not found at the scene.",
                0_u16,
            ),
            (
                "revachol/case.md",
                "revachol/case.md > Witnesses",
                "Kim Kitsuragi noted the absence of footprints.",
                1,
            ),
            (
                "martinaise/harbor.md",
                "martinaise/harbor.md > Description",
                "The harbor smelled of coal and dead fish.",
                0,
            ),
        ];

        for (np, bc, content, ci) in &chunks_data {
            let np_off = string_table.len() as u32;
            string_table.extend_from_slice(np.as_bytes());
            let bc_off = string_table.len() as u32;
            string_table.extend_from_slice(bc.as_bytes());
            let ct_off = string_table.len() as u32;
            string_table.extend_from_slice(content.as_bytes());

            entries.push(ChunkEntry {
                note_path_offset: np_off,
                note_path_len: np.len() as u16,
                breadcrumb_offset: bc_off,
                breadcrumb_len: bc.len() as u16,
                chunk_index: *ci,
                content_offset: ct_off,
                content_len: content.len() as u16,
            });
        }

        let graph = CsrGraph::new_fixed_k(k, n_chunks, dim, offsets, adj, chunk_to_doc)
            .unwrap()
            .with_identity(string_table, entries);

        graph.save_to_path(&path).unwrap();
        let loaded = CsrGraph::load_from_path(&path).unwrap();

        assert_eq!(loaded.k, k);
        assert_eq!(loaded.n_chunks, n_chunks);
        assert!(loaded.has_identity());
        assert_eq!(loaded.chunk_note_path(0), "revachol/case.md");
        assert_eq!(loaded.chunk_breadcrumb(1), "revachol/case.md > Witnesses");
        assert_eq!(
            loaded.chunk_excerpt(2),
            "The harbor smelled of coal and dead fish."
        );
        assert_eq!(loaded.chunk_index_in_doc(1), 1);

        // Topology still works
        let n0 = loaded.neighbors(0);
        assert_eq!(n0.len(), 2);
        assert_eq!(n0[0].0, 1);
    }

    #[test]
    fn csr_graph_v1_file_loads_without_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("graph_v1.kjsg");

        // Graph without identity writes as v1
        let graph = CsrGraph::new_fixed_k(
            1,
            2,
            4,
            vec![0, 1, 2],
            vec![(1_u32, 0.5_f32), (0, 0.5)],
            vec![0_u16, 1],
        )
        .unwrap();

        graph.save_to_path(&path).unwrap();
        let loaded = CsrGraph::load_from_path(&path).unwrap();

        assert!(!loaded.has_identity());
        assert_eq!(loaded.chunk_note_path(0), "");
        assert_eq!(loaded.neighbors(0).len(), 1);
    }
}
