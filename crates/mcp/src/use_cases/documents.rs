use crate::domain::documents_input::ExamineContentMode;
use crate::format::format_examine_result;
use crate::ports::DocumentsPorts;

pub(crate) struct ExamineOutput {
    pub summary: String,
}

pub(crate) async fn execute_examine(
    ports: &impl DocumentsPorts,
    path: &str,
    content_mode: ExamineContentMode,
    offset: usize,
    length: usize,
) -> anyhow::Result<ExamineOutput> {
    let result = ports.examine(path).await?;
    let text = format_examine_result(&result.document, content_mode, offset, length);
    Ok(ExamineOutput { summary: text })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kajet_core::search::ExamineResult;
    use kajet_core::types::Document;

    #[derive(Clone)]
    struct FakeDocumentsPorts {
        doc: Document,
    }

    impl DocumentsPorts for FakeDocumentsPorts {
        async fn examine(&self, _path: &str) -> anyhow::Result<ExamineResult> {
            Ok(ExamineResult {
                document: self.doc.clone(),
            })
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
        let ports = FakeDocumentsPorts { doc: sample_doc() };
        let out = execute_examine(&ports, "demo", ExamineContentMode::Summary, 0, 500)
            .await
            .expect("ok");

        assert!(out.summary.contains("Demo"));
        assert!(out.summary.contains("rust"));
        assert!(out.summary.contains("Hello world from kajet"));
    }

    #[tokio::test]
    async fn execute_examine_formats_slice_mode() {
        let ports = FakeDocumentsPorts { doc: sample_doc() };
        let out = execute_examine(&ports, "demo", ExamineContentMode::Slice, 6, 5)
            .await
            .expect("ok");

        assert!(out.summary.contains("world"));
    }
}
