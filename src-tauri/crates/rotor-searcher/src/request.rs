use crate::file_data::SearchResultItem;

/// Process-local identity; repeated text searches must still have distinct IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QueryId(pub u64);

#[derive(Clone, Copy, Debug)]
pub struct SearchUnavailable;

impl std::fmt::Display for SearchUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("search service is stopped")
    }
}
impl std::error::Error for SearchUnavailable {}

#[derive(Clone, Debug)]
pub struct SearchRequest {
    pub id: QueryId,
    pub query: String,
}

#[derive(Clone)]
pub struct SearchBatch {
    pub id: QueryId,
    pub query: String,
    pub items: Vec<SearchResultItem>,
    pub append: bool,
}
