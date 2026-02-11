use crate::domain::tags_input::TagListDetail;

pub(crate) fn format_list_tags(
    tag_map: &[(String, Vec<String>)],
    detail: TagListDetail,
    folder: Option<&str>,
) -> String {
    if tag_map.is_empty() {
        return t!("list_tags_empty").to_string();
    }

    let header = match folder {
        Some(f) => t!("list_tags_folder", folder = f, count = tag_map.len()).to_string(),
        None => t!("list_tags_header", count = tag_map.len()).to_string(),
    };

    let body = match detail {
        TagListDetail::Names => tag_map
            .iter()
            .map(|(tag, _)| format!("#{tag}"))
            .collect::<Vec<_>>()
            .join(", "),
        TagListDetail::Full => tag_map
            .iter()
            .map(|(tag, paths)| {
                let files = paths.join(", ");
                format!("#{tag} ({} notes): {files}", paths.len())
            })
            .collect::<Vec<_>>()
            .join("\n"),
        TagListDetail::Counts => tag_map
            .iter()
            .map(|(tag, paths)| format!("#{tag}: {} notes", paths.len()))
            .collect::<Vec<_>>()
            .join("\n"),
    };

    format!("{header}\n{body}")
}
