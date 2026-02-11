use crate::ports::IndexPorts;

pub(crate) enum ReindexOutput {
    SingleFile { path: String, summary: String },
    Full { summary: String },
}

pub(crate) async fn execute_reindex(
    ports: &impl IndexPorts,
    path: Option<String>,
) -> anyhow::Result<ReindexOutput> {
    match path {
        Some(path) => {
            let rel_paths = vec![path.clone()];
            ports.reindex_files(&rel_paths).await?;
            Ok(ReindexOutput::SingleFile {
                summary: t!("reindex_file_complete", path = &path).to_string(),
                path,
            })
        }
        None => {
            let exclude = ports.exclude_folders();
            let stats = ports.full_reindex(&exclude).await?;
            Ok(ReindexOutput::Full {
                summary: t!(
                    "reindex_complete",
                    documents = stats.total_documents,
                    chunks = stats.total_chunks
                )
                .to_string(),
            })
        }
    }
}

pub(crate) async fn execute_index_status(ports: &impl IndexPorts) -> anyhow::Result<String> {
    let stats = ports.get_index_stats().await?;
    let last_indexed = match stats.last_indexed {
        Some(time) => t!("index_status_last_indexed", time = time.to_rfc3339()).to_string(),
        None => t!("index_status_never").to_string(),
    };

    Ok(format!(
        "{}\n{}\n{}\n{}",
        t!("index_status_header"),
        t!("index_status_documents", count = stats.total_documents),
        t!("index_status_chunks", count = stats.total_chunks),
        last_indexed,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kajet_core::types::IndexStats;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct FakeIndexPorts {
        exclude: Vec<String>,
        reindexed: Arc<Mutex<Vec<String>>>,
        stats: IndexStats,
    }

    impl FakeIndexPorts {
        fn with_stats(stats: IndexStats) -> Self {
            Self {
                exclude: vec!["Templates".to_string()],
                reindexed: Arc::new(Mutex::new(Vec::new())),
                stats,
            }
        }
    }

    impl IndexPorts for FakeIndexPorts {
        fn exclude_folders(&self) -> Vec<String> {
            self.exclude.clone()
        }

        async fn reindex_files(&self, rel_paths: &[String]) -> anyhow::Result<()> {
            self.reindexed
                .lock()
                .expect("lock")
                .extend(rel_paths.to_vec());
            Ok(())
        }

        async fn full_reindex(&self, _exclude_folders: &[String]) -> anyhow::Result<IndexStats> {
            Ok(self.stats.clone())
        }

        async fn get_index_stats(&self) -> anyhow::Result<IndexStats> {
            Ok(self.stats.clone())
        }
    }

    #[tokio::test]
    async fn reindex_single_file_calls_reindex_files() {
        let ports = FakeIndexPorts::with_stats(IndexStats {
            total_documents: 0,
            total_chunks: 0,
            last_indexed: None,
        });
        let out = execute_reindex(&ports, Some("journal/today.md".to_string()))
            .await
            .expect("ok");

        match out {
            ReindexOutput::SingleFile { path, summary } => {
                assert_eq!(path, "journal/today.md");
                assert!(summary.contains("journal/today.md"));
            }
            ReindexOutput::Full { .. } => panic!("expected single file reindex"),
        }

        let reindexed = ports.reindexed.lock().expect("lock");
        assert_eq!(reindexed.as_slice(), &["journal/today.md".to_string()]);
    }

    #[tokio::test]
    async fn reindex_full_returns_stats_summary() {
        let ports = FakeIndexPorts::with_stats(IndexStats {
            total_documents: 12,
            total_chunks: 34,
            last_indexed: None,
        });

        let out = execute_reindex(&ports, None).await.expect("ok");
        match out {
            ReindexOutput::Full { summary } => {
                assert!(summary.contains("12"));
                assert!(summary.contains("34"));
            }
            ReindexOutput::SingleFile { .. } => panic!("expected full reindex"),
        }
    }

    #[tokio::test]
    async fn index_status_includes_counts_and_timestamp() {
        let ts = chrono::DateTime::parse_from_rfc3339("2026-02-11T12:34:56Z")
            .expect("valid ts")
            .with_timezone(&chrono::Utc);
        let ports = FakeIndexPorts::with_stats(IndexStats {
            total_documents: 2,
            total_chunks: 5,
            last_indexed: Some(ts),
        });

        let text = execute_index_status(&ports).await.expect("ok");
        assert!(text.contains("2"));
        assert!(text.contains("5"));
        assert!(text.contains("2026-02-11T12:34:56+00:00"));
    }
}
