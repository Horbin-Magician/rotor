use rotor_runtime::{QueryId, SearchBatch, SearchResultItem};
use std::collections::HashSet;

pub const MAX_RESULTS: usize = 100;

#[derive(Default)]
pub struct SearchResults {
    pub items: Vec<SearchResultItem>,
    pub selected: usize,
    pub loading: bool,
    pub replacing: bool,
    pub exhausted: bool,
    active: Option<(QueryId, String)>,
    icon_generation: Option<QueryId>,
}

impl SearchResults {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn begin(&mut self, id: QueryId, query: String, append: bool) {
        if !append {
            // Keep the previous rows visible until the replacement arrives so
            // typing does not repeatedly collapse and expand the launcher.
            self.selected = 0;
            self.exhausted = false;
            self.icon_generation = None;
        }
        self.active = Some((id, query));
        self.loading = true;
        self.replacing = !append;
    }

    pub fn accepts_icons(&self, id: QueryId) -> bool {
        self.icon_generation == Some(id)
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
            self.icon_generation = Some(batch.id);
            self.items.clear();
            self.selected = 0;
        }
        self.loading = false;
        self.replacing = false;
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
            alias: None,
        }
    }
    #[test]
    fn icon_generation_survives_paging_and_rejects_repeated_query_replacements() {
        let mut results = SearchResults::default();
        results.begin(QueryId(1), "same".into(), false);
        results.accept(&SearchBatch {
            id: QueryId(1),
            query: "same".into(),
            items: vec![item("first")],
            append: false,
        });
        assert!(results.accepts_icons(QueryId(1)));
        results.begin(QueryId(2), "same".into(), true);
        results.accept(&SearchBatch {
            id: QueryId(2),
            query: "same".into(),
            items: vec![item("second")],
            append: true,
        });
        assert!(results.accepts_icons(QueryId(1)));
        assert!(!results.accepts_icons(QueryId(2)));
        results.begin(QueryId(3), "other".into(), false);
        assert!(!results.accepts_icons(QueryId(1)));
        results.begin(QueryId(4), "same".into(), false);
        results.accept(&SearchBatch {
            id: QueryId(4),
            query: "same".into(),
            items: vec![item("fresh")],
            append: false,
        });
        assert!(results.accepts_icons(QueryId(4)));
        assert!(!results.accepts_icons(QueryId(1)));
        results.reset();
        assert!(!results.accepts_icons(QueryId(4)));
    }

    #[test]
    fn accepted_results_keep_batch_rows_and_reject_stale_batches() {
        let mut results = SearchResults::default();
        results.begin(QueryId(2), "fixture".into(), false);
        let mut batch = SearchBatch {
            id: QueryId(1),
            query: "fixture".into(),
            items: vec![item("fixture")],
            append: false,
        };
        assert!(!results.accept(&batch));
        assert!(results.items.is_empty());
        batch.id = QueryId(2);
        assert!(results.accept(&batch));
        assert_eq!(results.items[0].file_path, "fixture");
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
    fn pages_are_bounded_and_remain_visible_until_the_new_query_arrives() {
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
        assert_eq!(results.items.len(), MAX_RESULTS);
        assert_eq!(results.selected, 0);
        assert!(results.replacing);
        assert!(!results.accept(&SearchBatch {
            id: QueryId(2),
            query: "a".into(),
            items: vec![item("late-page")],
            append: true,
        }));
        results.accept(&SearchBatch {
            id: QueryId(3),
            query: "b".into(),
            items: vec![],
            append: false,
        });
        assert!(results.exhausted);
        assert!(!results.replacing);
        assert!(results.items.is_empty());
    }
}
