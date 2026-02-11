use crate::domain::search_input::{PreparedSearchInput, SearchKind, SearchMode};
use crate::filters;
use crate::format::{format_entries, format_results};
use crate::ports::SearchPorts;
use kajet_core::types::QueryEvent;

pub(crate) enum SearchUseCaseOutput {
    Search {
        query: String,
        mode: SearchMode,
        limit: usize,
        results: usize,
        summary: String,
    },
    Browse {
        from: Option<chrono::NaiveDate>,
        to: Option<chrono::NaiveDate>,
        folder: Option<String>,
        tags: Option<Vec<String>>,
        limit: usize,
        results: usize,
        summary: String,
    },
}

pub(crate) async fn execute_search(
    ports: &impl SearchPorts,
    input: PreparedSearchInput,
) -> anyhow::Result<SearchUseCaseOutput> {
    match input.kind {
        SearchKind::Query { query, mode } => {
            let fetch_limit = if input.has_filters {
                input.limit * ports.filter_overfetch_multiplier()
            } else {
                input.limit
            };

            let mut results = ports.run_search(&query, mode, fetch_limit).await?;

            if input.has_filters {
                let all_docs = ports.get_all_documents().await?;
                filters::filter_search_results(
                    &mut results,
                    &all_docs,
                    input.folder.as_deref(),
                    input.from_ts,
                    input.to_ts,
                    input.tags.as_deref(),
                    input.limit,
                );
            }

            let summary = format_results(&query, &results);
            ports.send_query_event(QueryEvent {
                query: query.clone(),
                num_results: results.len(),
                timestamp: chrono::Utc::now(),
            });

            Ok(SearchUseCaseOutput::Search {
                query,
                mode,
                limit: input.limit,
                results: results.len(),
                summary,
            })
        }
        SearchKind::Browse => {
            let has_date_or_folder = input.from_ts.is_some()
                || input.to_ts.is_some()
                || input.folder.as_deref().is_some();
            let fetch_limit = if has_date_or_folder {
                input.limit * ports.filter_overfetch_multiplier()
            } else {
                ports
                    .tags_only_fetch_limit()
                    .max(input.limit * ports.filter_overfetch_multiplier())
            };

            let mut docs = ports
                .query_documents(
                    input.from_ts,
                    input.to_ts,
                    input.folder.as_deref(),
                    fetch_limit,
                )
                .await?;
            filters::filter_browse_results(&mut docs, input.tags.as_deref(), input.limit);

            let summary = format_entries(input.from_date, input.to_date, &docs);
            ports.send_query_event(QueryEvent {
                query: input.browse_event_query(),
                num_results: docs.len(),
                timestamp: chrono::Utc::now(),
            });

            Ok(SearchUseCaseOutput::Browse {
                from: input.from_date,
                to: input.to_date,
                folder: input.folder,
                tags: input.tags,
                limit: input.limit,
                results: docs.len(),
                summary,
            })
        }
    }
}
