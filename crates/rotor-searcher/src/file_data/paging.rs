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

#[cfg(all(test, target_os = "windows"))]
mod relevance_tests {
    use super::*;
    use crate::file_data::{
        excluded_dirs::ExcludedDirs,
        volume::{default_file_map, ntfs_file_map},
    };
    use std::{collections::HashSet, sync::atomic::AtomicBool};

    #[test]
    fn direct_parent_qualifier_precedes_ancestor_across_backend_pages() {
        let mut ntfs = ntfs_file_map::FileMap::new();
        ntfs.insert(1, "X:".into(), 0);
        ntfs.insert(2, "docs".into(), 1);
        ntfs.insert(3, "nested".into(), 2);
        ntfs.insert(4, "report.exe".into(), 3);
        ntfs.insert(5, "report-long-name.txt".into(), 2);
        let mut portable = default_file_map::FileMap::new();
        portable.insert("report.exe".into(), "Y:/docs/nested".into());
        portable.insert("report-long-name.txt".into(), "Y:/docs".into());
        let cancel = AtomicBool::new(false);
        let excluded = ExcludedDirs::default();
        for batch in [1, 2, 20] {
            let mut pages = MergePages::new(2);
            let mut results = Vec::new();
            loop {
                if pages.needs_page(0) {
                    pages.accept(
                        0,
                        ntfs.search(
                            "docs/report",
                            pages.cursor(0).as_ref(),
                            batch,
                            &cancel,
                            &excluded,
                        ),
                    );
                }
                if pages.needs_page(1) {
                    pages.accept(
                        1,
                        portable.search("docs/report", pages.cursor(1).as_ref(), batch, &cancel),
                    );
                }
                let Some(item) = pages.pop_best() else {
                    break;
                };
                results.push(item);
                assert!(results.len() <= 4);
            }
            assert_eq!(results.len(), 4);
            assert!(results[..2]
                .iter()
                .all(|item| item.file_name == "report-long-name.txt"));
            assert!(results[2..]
                .iter()
                .all(|item| item.file_name == "report.exe"));
        }
    }

    #[test]
    fn real_backends_merge_relevance_across_tiers_and_page_sizes() {
        let mut ntfs = ntfs_file_map::FileMap::new();
        let mut portable = default_file_map::FileMap::new();
        ntfs.insert(1, "X:".into(), 0);
        ntfs.insert(2, "docs".into(), 1);
        ntfs.insert(3, "other".into(), 1);
        for (offset, name) in [
            "report",
            "REPORT",
            "report-2026.txt",
            "a-report.lnk",
            "report.exe",
            "报告.txt",
        ]
        .iter()
        .enumerate()
        {
            ntfs.insert(offset as u64 + 4, (*name).into(), 2);
            portable.insert((*name).into(), "Y:/docs".into());
        }
        ntfs.insert(20, "report".into(), 3);
        portable.insert("report".into(), "Y:/other".into());
        let cancel = AtomicBool::new(false);
        let excluded = ExcludedDirs::default();
        for (query, count) in [
            ("report", 12),
            ("docs/report", 10),
            ("docs\\report", 10),
            ("bao", 2),
            ("*report*", 12),
            ("missing", 0),
        ] {
            let mut reference = None;
            for batch in [1, 2, 7, 255] {
                let mut pages = MergePages::new(2);
                let mut results = Vec::new();
                loop {
                    // The portable worker completes first; volume order still wins ties.
                    if pages.needs_page(1) {
                        pages.accept(
                            1,
                            portable.search(query, pages.cursor(1).as_ref(), batch, &cancel),
                        );
                    }
                    if pages.needs_page(0) {
                        pages.accept(
                            0,
                            ntfs.search(query, pages.cursor(0).as_ref(), batch, &cancel, &excluded),
                        );
                    }
                    let Some(item) = pages.pop_best() else {
                        break;
                    };
                    results.push((item.rank, item.file_path));
                    assert!(results.len() <= 14, "pagination did not terminate");
                }
                assert_eq!(results.len(), count, "{query}");
                assert!(results.windows(2).all(|pair| pair[0].0 >= pair[1].0));
                assert_eq!(
                    results
                        .iter()
                        .map(|row| &row.1)
                        .collect::<HashSet<_>>()
                        .len(),
                    count
                );
                if let Some(reference) = &reference {
                    assert_eq!(&results, reference);
                } else {
                    reference = Some(results);
                }
            }
            if query == "report" {
                let rows = reference.unwrap();
                assert!(rows[..6].iter().all(|row| row.0 >= 512));
                assert!(rows[6..10].iter().all(|row| row.0 >= 256 && row.0 < 512));
                assert!(rows[10..].iter().all(|row| row.0 < 256));
            }
        }
        let cancelled = AtomicBool::new(true);
        assert!(ntfs
            .search("report", None, 1, &cancelled, &excluded)
            .is_none());
        assert!(portable.search("report", None, 1, &cancelled).is_none());
    }
}
