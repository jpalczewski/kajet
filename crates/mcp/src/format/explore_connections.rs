use crate::use_cases::explore_connections::{ExploreConnectionsView, ExploreNodeView};

pub(crate) fn format_explore_connections(view: &ExploreConnectionsView) -> String {
    let mut lines = vec![
        t!(
            "explore_header",
            path = view.start_path.as_str(),
            depth = view.depth_limit,
            limit = view.node_limit,
            mode = view.filter_mode.as_str(),
            dedup = view.dedup
        )
        .to_string(),
        String::new(),
    ];

    render_node(&view.root, 0, &mut lines);
    lines.push(String::new());

    if view.filtered_results_empty {
        lines.push(t!("explore_no_filtered_results").to_string());
    }

    if view.shown_nodes == view.unique_nodes {
        lines.push(
            t!(
                "explore_summary",
                unique = view.unique_nodes,
                depth = view.max_depth_reached
            )
            .to_string(),
        );
    } else {
        lines.push(
            t!(
                "explore_summary_with_shown",
                unique = view.unique_nodes,
                shown = view.shown_nodes,
                depth = view.max_depth_reached
            )
            .to_string(),
        );
    }

    if view.truncated {
        lines.push(t!("explore_limit_reached", limit = view.node_limit).to_string());
    }

    lines.join("\n")
}

fn render_node(node: &ExploreNodeView, indent: usize, out: &mut Vec<String>) {
    let prefix = "  ".repeat(indent);
    let cycle = if node.cycle {
        t!("explore_cycle_suffix").to_string()
    } else {
        String::new()
    };

    let line = if let Some(direction) = node.direction {
        format!(
            "{prefix}{} {} (depth {}, {} connections{})",
            direction.arrow(),
            node.path,
            node.depth,
            node.degree,
            cycle
        )
    } else {
        format!(
            "{prefix}{} ({} connections{})",
            node.path, node.degree, cycle
        )
    };

    out.push(line);

    if let Some(section) = &node.context {
        out.push(format!(
            "{}  {}",
            prefix,
            t!("explore_section", section = section.as_str())
        ));
    }

    for child in &node.children {
        render_node(child, indent + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::explore_input::ExploreFilterMode;
    use crate::use_cases::explore_connections::{ConnectionDirection, ExploreNodeView};

    #[test]
    fn formats_tree_with_context_and_cycle_marker() {
        rust_i18n::set_locale("en");

        let view = ExploreConnectionsView {
            start_path: "topic-a.md".to_string(),
            depth_limit: 2,
            node_limit: 50,
            filter_mode: ExploreFilterMode::Display,
            dedup: true,
            root: ExploreNodeView {
                path: "topic-a.md".to_string(),
                depth: 0,
                degree: 2,
                direction: None,
                context: None,
                cycle: false,
                children: vec![ExploreNodeView {
                    path: "hub-note.md".to_string(),
                    depth: 1,
                    degree: 5,
                    direction: Some(ConnectionDirection::Backlink),
                    context: Some("Core concepts".to_string()),
                    cycle: true,
                    children: vec![],
                }],
            },
            unique_nodes: 2,
            shown_nodes: 2,
            max_depth_reached: 1,
            truncated: false,
            filtered_results_empty: false,
        };

        let text = format_explore_connections(&view);
        assert!(text.contains("Exploring connections from: topic-a.md"));
        assert!(text.contains("← hub-note.md"));
        assert!(text.contains("section: \"Core concepts\""));
        assert!(text.contains("cycle detected"));
    }
}
