mod analytics;
mod entries;
mod examine;
mod explore_connections;
mod notes;
mod search;
mod tags;
mod tree;

pub use entries::format_entries;
pub use notes::{format_create_result, format_edit_result, format_edit_tags_result};
pub use search::format_results;
pub use tree::{format_tree_too_large, format_vault_tree};

pub(crate) use analytics::{RecentContextEntryView, RecentContextView, format_recent_context};
pub(crate) use examine::{ExamineBatchSection, format_examine_batch, format_examine_result};
pub(crate) use explore_connections::format_explore_connections;
pub(crate) use tags::format_list_tags;

#[cfg(test)]
mod tests;
