pub fn format_vault_tree(tree: &kajet_parser::VaultFolder) -> String {
    let showing = count_notes(tree);
    let header = if let Some(total) = tree.total_notes_in_vault {
        if showing < total {
            t!("tree_header_truncated", showing = showing, total = total).to_string()
        } else {
            t!("tree_header", count = total).to_string()
        }
    } else {
        t!("tree_header", count = showing).to_string()
    };

    let mut lines = vec![header];
    format_tree_recursive(tree, &mut lines, 0);
    lines.join("\n")
}

fn count_notes(folder: &kajet_parser::VaultFolder) -> usize {
    folder.note_count + folder.subfolders.iter().map(count_notes).sum::<usize>()
}

fn format_tree_recursive(
    folder: &kajet_parser::VaultFolder,
    lines: &mut Vec<String>,
    indent: usize,
) {
    let prefix = "  ".repeat(indent);

    for file in &folder.files {
        lines.push(format!("{prefix}{file}"));
    }

    for sub in &folder.subfolders {
        let sub_total = count_notes(sub);
        lines.push(format!("{prefix}{}/ ({sub_total})", sub.name));
        format_tree_recursive(sub, lines, indent + 1);
    }
}

pub fn format_tree_too_large(
    chars: usize,
    max_chars: usize,
    depth: usize,
    has_files: bool,
) -> String {
    let mut text = t!("tree_too_large", chars = chars, max_chars = max_chars).to_string();
    text.push('\n');
    text.push_str(&t!("tree_suggestion_path"));
    text.push('\n');
    text.push_str(&t!("tree_suggestion_depth", depth = depth));

    if has_files {
        text.push('\n');
        text.push_str(&t!("tree_suggestion_show"));
    }

    text
}
