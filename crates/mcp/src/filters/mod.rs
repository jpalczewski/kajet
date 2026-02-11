mod aggregate;
mod browse;
mod edit;
mod search;
mod tags;
mod types;

pub use aggregate::aggregate_tags;
pub use browse::filter_browse_results;
pub use edit::apply_tag_edits;
pub use search::filter_search_results;
pub use tags::tags_match;
pub use types::{TagEditResult, TagStats};

#[cfg(test)]
mod tests;
