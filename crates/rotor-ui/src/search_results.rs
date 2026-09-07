use rotor_runtime::{QueryId, SearchBatch, SearchResultItem};
use std::collections::HashSet;

pub const MAX_RESULTS: usize = 100;

#[derive(Default)]
pub struct SearchResults {
    pub items: Vec<SearchResultItem>,
    pub selected: usize,
    pub loading: bool,
    pub exhausted: bool,
    active: Option<(QueryId, String)>,
}

impl SearchResults {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn begin(&mut self, id: QueryId, query: String, append: bool) {
        if !append {
            self.reset();
        }
        self.active = Some((id, query));
        self.loading = true;
    }

    pub fn accept(&mut self, batch: &SearchBatch) -> bool {
        if !self
            .active
            .as_ref()
            .is_some_and(|(id, query)| *id == batch.id && *query == batch.query)
        {
            return false;
        }
        if !batch.append {
            self.items.clear();
            self.selected = 0;
        }
        self.loading = false;
        self.exhausted = batch.items.is_empty();
        let mut paths: HashSet<_> = self
            .items
            .iter()
            .map(|item| item.file_path.clone())
            .collect();
        for item in &batch.items {
            if self.items.len() == MAX_RESULTS {
                break;
            }
            if paths.insert(item.file_path.clone()) {
                self.items.push(item.clone());
            }
        }
        self.selected = self.selected.min(self.items.len().saturating_sub(1));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(path: &str) -> SearchResultItem {
        SearchResultItem {
            path: String::new(),
            file_path: path.into(),
            file_name: path.into(),
            rank: 0,
            icon_data: None,
            alias: None,
        }
    }
    #[test]
    fn late_identical_query_cannot_replace_newer_results() {
        let mut results = SearchResults::default();
        results.begin(QueryId(2), "same".into(), false);
        assert!(!results.accept(&SearchBatch {
            id: QueryId(1),
            query: "same".into(),
            items: vec![item("old")],
            append: false
        }));
        assert!(results.loading);
        assert!(results.items.is_empty());
    }
    #[test]
    fn pages_are_deduplicated_bounded_and_cleared_on_new_query() {
        let mut results = SearchResults::default();
        results.begin(QueryId(1), "a".into(), false);
        results.accept(&SearchBatch {
            id: QueryId(1),
            query: "a".into(),
            items: (0..80).map(|id| item(&id.to_string())).collect(),
            append: false,
        });
        results.begin(QueryId(2), "a".into(), true);
        results.accept(&SearchBatch {
            id: QueryId(2),
            query: "a".into(),
            items: (40..150).map(|id| item(&id.to_string())).collect(),
            append: true,
        });
        assert_eq!(results.items.len(), MAX_RESULTS);
        assert_eq!(results.items.last().unwrap().file_path, "99");
        results.selected = 99;
        results.begin(QueryId(3), "b".into(), false);
        assert!(results.items.is_empty());
        assert_eq!(results.selected, 0);
        results.accept(&SearchBatch {
            id: QueryId(3),
            query: "b".into(),
            items: vec![],
            append: false,
        });
        assert!(results.exhausted);
    }
}
