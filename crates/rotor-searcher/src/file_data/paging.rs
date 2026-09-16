use super::volume::{SearchCursor, SearchPage, SearchResultItem};
use std::collections::VecDeque;

#[derive(Default)]
struct VolumePage {
    items: VecDeque<SearchResultItem>,
    cursor: Option<SearchCursor>,
    exhausted: bool,
}

/// Retain only unconsumed rows, at most one batch per volume. A volume must
/// have a head (or be exhausted) before selecting the globally highest rank.
#[derive(Default)]
pub(super) struct MergePages {
    volumes: Vec<VolumePage>,
}

impl MergePages {
    pub fn needs_refill(&self) -> bool {
        (0..self.volumes.len()).any(|index| self.needs_page(index))
    }
    pub fn new(count: usize) -> Self {
        Self {
            volumes: (0..count).map(|_| VolumePage::default()).collect(),
        }
    }

    pub fn needs_page(&self, index: usize) -> bool {
        let page = &self.volumes[index];
        page.items.is_empty() && !page.exhausted
    }

    pub fn cursor(&self, index: usize) -> Option<SearchCursor> {
        self.volumes[index].cursor.clone()
    }

    pub fn accept(&mut self, index: usize, page: Option<SearchPage>) {
        let target = &mut self.volumes[index];
        match page {
            Some(page) => {
                target.exhausted = page.exhausted || page.items.is_empty();
                target.items = page.items.into();
                target.cursor = page.cursor;
            }
            None => target.exhausted = true,
        }
    }

    pub fn finish_missing(&mut self) {
        for page in &mut self.volumes {
            if page.items.is_empty() {
                page.exhausted = true;
            }
        }
    }

    pub fn pop_best(&mut self) -> Option<SearchResultItem> {
        debug_assert!(self
            .volumes
            .iter()
            .all(|page| page.exhausted || !page.items.is_empty()));
        // Resolve equal ranks by stable volume order, independent of worker timing.
        let best = self
            .volumes
            .iter()
            .enumerate()
            .filter_map(|(index, page)| page.items.front().map(|item| (index, item.rank)))
            .max_by_key(|(index, rank)| (*rank, std::cmp::Reverse(*index)))?
            .0;
        self.volumes[best].items.pop_front()
    }

    #[cfg(test)]
    pub fn buffered_len(&self) -> usize {
        self.volumes.iter().map(|page| page.items.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uneven_volumes_merge_all_pages_without_reordering_or_duplicates() {
        for batch in [1, 7, 20] {
            let sources = [vec![60; 53], vec![20; 39], vec![60, 50, 40, 20], vec![]];
            let mut positions = [0; 4];
            let mut pages = MergePages::new(4);
            let mut actual = Vec::new();
            loop {
                // Reverse completion order must not affect equal-rank ordering.
                for index in (0..sources.len()).rev() {
                    if !pages.needs_page(index) {
                        continue;
                    }
                    let source = &sources[index];
                    let end = (positions[index] + batch).min(source.len());
                    let items = (positions[index]..end)
                        .map(|offset| SearchResultItem {
                            rank: source[offset],
                            file_path: format!("{index}/{offset}"),
                            path: String::new(),
                            file_name: String::new(),
                            icon: None,
                            alias: None,
                        })
                        .collect();
                    positions[index] = end;
                    pages.accept(
                        index,
                        Some(SearchPage {
                            items,
                            cursor: None,
                            exhausted: end == source.len(),
                        }),
                    );
                }
                assert!(pages.buffered_len() <= sources.len() * batch);
                let Some(item) = pages.pop_best() else {
                    break;
                };
                actual.push((item.rank, item.file_path));
            }
            let mut expected: Vec<_> = sources
                .iter()
                .enumerate()
                .flat_map(|(index, ranks)| {
                    ranks
                        .iter()
                        .enumerate()
                        .map(move |(offset, rank)| (*rank, format!("{index}/{offset}")))
                })
                .collect();
            expected.sort_by_key(|(rank, _)| std::cmp::Reverse(*rank));
            assert_eq!(actual, expected);
        }
    }
}
