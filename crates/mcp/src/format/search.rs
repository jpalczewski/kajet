use kajet_core::search::SearchResult;

pub fn format_results(query: &str, results: &[SearchResult]) -> String {
    if results.is_empty() {
        return t!("no_results", query = query).to_string();
    }

    let formatted = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut parts = format!(
                "{}\n{}\n{}\n\n{}",
                t!(
                    "result_header",
                    index = i + 1,
                    score = format!("{:.4}", r.score)
                ),
                t!("result_path", path = &r.note_path),
                t!("result_section", breadcrumb = &r.breadcrumb),
                r.content,
            );

            if !r.links.is_empty() {
                let link_list: Vec<String> = r
                    .links
                    .iter()
                    .map(|l| match (&l.alias, &l.resolved_path) {
                        (Some(alias), Some(path)) => {
                            format!("  - {} → {} ({})", l.target, path, alias)
                        }
                        (None, Some(path)) => format!("  - {} → {}", l.target, path),
                        (Some(alias), None) => format!("  - {} ({})", l.target, alias),
                        (None, None) => format!("  - {}", l.target),
                    })
                    .collect();

                parts.push_str(&format!(
                    "\n\n{}\n{}",
                    t!("result_links"),
                    link_list.join("\n")
                ));
            }

            parts
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        "{}\n\n{}",
        t!("found_results", count = results.len(), query = query),
        formatted
    )
}
