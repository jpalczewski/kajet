use crate::use_cases::find_similar::FindSimilarView;

pub(crate) fn format_find_similar(view: &FindSimilarView) -> String {
    if view.source_chunks_missing {
        return t!(
            "find_similar_no_source_chunks",
            path = view.source_path.as_str()
        )
        .to_string();
    }

    let mut lines = vec![
        t!(
            "find_similar_header",
            path = view.source_path.as_str(),
            count = view.total_found,
            threshold = format!("{:.2}", view.threshold),
            aggregation = view.aggregation.as_str(),
            sort = view.sort.as_str(),
            exclude_linked = view.exclude_linked.as_str(),
            limit = view.limit
        )
        .to_string(),
        String::new(),
    ];

    lines.push(
        t!(
            "find_similar_window",
            source_chunks = view.source_chunk_total,
            fetch_limit = view.fetch_limit_per_query
        )
        .to_string(),
    );
    if view.fetch_truncated {
        lines.push(t!("find_similar_window_truncated").to_string());
    }
    lines.push(
        t!(
            "find_similar_excluded_links",
            mode = view.exclude_linked.as_str(),
            count = view.excluded_by_link_count
        )
        .to_string(),
    );
    lines.push(String::new());

    if view.results.is_empty() {
        lines.push(t!("find_similar_no_results").to_string());
        return lines.join("\n");
    }

    for (idx, item) in view.results.iter().enumerate() {
        lines.push(
            t!(
                "find_similar_item",
                index = idx + 1,
                path = item.note_path.as_str(),
                similarity = format!("{:.2}", item.similarity)
            )
            .to_string(),
        );
        if let Some(source_section) = &item.source_section {
            lines.push(
                t!(
                    "find_similar_source_section",
                    section = source_section.as_str(),
                    chunk_index = item.source_chunk_index,
                    chunk_total = item.source_chunk_total
                )
                .to_string(),
            );
        } else {
            lines.push(
                t!(
                    "find_similar_source_chunk",
                    chunk_index = item.source_chunk_index,
                    chunk_total = item.source_chunk_total
                )
                .to_string(),
            );
        }
        lines.push(
            t!(
                "find_similar_coverage",
                matched = item.matched_source_chunks,
                total = item.source_chunk_total,
                min = format!("{:.2}", item.similarity_min),
                avg = format!("{:.2}", item.similarity_avg),
                max = format!("{:.2}", item.similarity_max)
            )
            .to_string(),
        );
        if let Some(target_section) = &item.target_section {
            lines.push(
                t!(
                    "find_similar_target_section",
                    section = target_section.as_str()
                )
                .to_string(),
            );
        }
        if item.tags.is_empty() {
            lines.push(t!("find_similar_tags_none").to_string());
        } else {
            lines.push(t!("find_similar_tags", tags = item.tags.join(", ").as_str()).to_string());
        }
        lines.push(String::new());
    }

    if view.total_found > view.limit {
        lines.push(t!("find_similar_truncated", limit = view.limit).to_string());
    }

    lines.join("\n")
}
