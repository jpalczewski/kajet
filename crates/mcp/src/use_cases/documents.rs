use crate::domain::documents_input::ExamineContentMode;
use crate::format::{ExamineBatchSection, format_examine_batch, format_examine_result};
use crate::ports::DocumentsPorts;
use kajet_core::search::ExamineManyResult;

pub(crate) struct ExamineOutput {
    pub summary: String,
}

pub(crate) async fn execute_examine(
    ports: &impl DocumentsPorts,
    paths: &[String],
    content_mode: ExamineContentMode,
    offset: usize,
    length: usize,
) -> anyhow::Result<ExamineOutput> {
    let mut sections = Vec::with_capacity(paths.len());
    for item in ports.examine_many(paths).await? {
        match item {
            ExamineManyResult::Success {
                requested_path,
                document,
            } => {
                let text = format_examine_result(&document, content_mode, offset, length);
                sections.push(ExamineBatchSection::Success {
                    requested_path,
                    body: text,
                });
            }
            ExamineManyResult::Error {
                requested_path,
                error,
            } => {
                sections.push(ExamineBatchSection::Error {
                    requested_path,
                    error,
                });
            }
        }
    }

    Ok(ExamineOutput {
        summary: format_examine_batch(&sections),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kajet_core::search::ExamineManyResult;
    use kajet_core::types::Document;
    use std::collections::HashMap;

    #[derive(Clone)]
    struct FakeDocumentsPorts {
        docs: HashMap<String, Document>,
    }

    impl DocumentsPorts for FakeDocumentsPorts {
        async fn examine_many(&self, paths: &[String]) -> anyhow::Result<Vec<ExamineManyResult>> {
            let mut out = Vec::with_capacity(paths.len());
            for path in paths {
                match self.docs.get(path) {
                    Some(doc) => out.push(ExamineManyResult::Success {
                        requested_path: path.clone(),
                        document: doc.clone(),
                    }),
                    None => out.push(ExamineManyResult::Error {
                        requested_path: path.clone(),
                        error: format!("Document not found: {path}"),
                    }),
                }
            }
            Ok(out)
        }
    }

    fn sample_doc() -> Document {
        Document {
            source_file: "notes/demo.md".to_string(),
            full_text: "Hello world from kajet".to_string(),
            title: "Demo".to_string(),
            tags: vec!["rust".to_string()],
            content_hash: "hash".to_string(),
            last_modified: 0.0,
            outgoing_links: vec!["Target".to_string()],
            backlinks: vec![],
        }
    }

    #[tokio::test]
    async fn execute_examine_formats_summary_mode() {
        let ports = FakeDocumentsPorts {
            docs: HashMap::from([(String::from("demo"), sample_doc())]),
        };
        let out = execute_examine(
            &ports,
            &[String::from("demo")],
            ExamineContentMode::Summary,
            0,
            500,
        )
        .await
        .expect("ok");

        assert!(out.summary.contains("Examined 1 documents"));
        assert!(out.summary.contains("=== demo ==="));
        assert!(out.summary.contains("Demo"));
        assert!(out.summary.contains("rust"));
        assert!(out.summary.contains("Hello world from kajet"));
    }

    #[tokio::test]
    async fn execute_examine_formats_slice_mode() {
        let ports = FakeDocumentsPorts {
            docs: HashMap::from([(String::from("demo"), sample_doc())]),
        };
        let out = execute_examine(
            &ports,
            &[String::from("demo")],
            ExamineContentMode::Slice,
            6,
            5,
        )
        .await
        .expect("ok");

        assert!(out.summary.contains("world"));
    }

    #[tokio::test]
    async fn execute_examine_partial_success_includes_error_section() {
        let ports = FakeDocumentsPorts {
            docs: HashMap::from([(String::from("demo"), sample_doc())]),
        };
        let out = execute_examine(
            &ports,
            &[String::from("demo"), String::from("missing.md")],
            ExamineContentMode::Summary,
            0,
            500,
        )
        .await
        .expect("ok");

        assert!(out.summary.contains("Examined 2 documents"));
        assert!(out.summary.contains("=== demo ==="));
        assert!(out.summary.contains("=== missing.md ==="));
        assert!(out.summary.contains("ERROR:"));
        assert!(out.summary.contains("Document not found: missing.md"));
    }
}
