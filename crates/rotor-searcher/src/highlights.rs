use pinyin::ToPinyin;
use std::ops::Range;

/// Byte ranges in the original display text, using the search query syntax.
#[derive(Default)]
pub struct SearchHighlights {
    name: Vec<String>,
    path: Vec<String>,
}

impl SearchHighlights {
    pub fn new(query: &str) -> Self {
        let normalized = query.replace('\\', "/").to_lowercase();
        let (path, name) = normalized.rsplit_once('/').unwrap_or(("", &normalized));
        let parts = |text: &str| {
            text.split('*')
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect()
        };
        Self {
            name: parts(name),
            path: parts(path),
        }
    }

    pub fn name_ranges(&self, text: &str) -> Vec<Range<usize>> {
        if self.name.is_empty() {
            return Vec::new();
        }
        // Match the same representations, in the same order, as the index.
        for mode in [Mode::Literal, Mode::Pinyin, Mode::Initials] {
            let (normalized, offsets) = normalize(text, mode);
            if let Some(ranges) = match_ranges(&normalized, &offsets, &self.name) {
                return ranges;
            }
            if text.is_ascii() {
                break;
            }
        }
        Vec::new()
    }

    pub fn path_ranges(&self, text: &str) -> Vec<Range<usize>> {
        let (normalized, offsets) = normalize(text, Mode::Path);
        match_ranges(&normalized, &offsets, &self.path).unwrap_or_default()
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Literal,
    Path,
    Pinyin,
    Initials,
}

fn normalize(text: &str, mode: Mode) -> (String, Vec<Range<usize>>) {
    let mut normalized = String::new();
    let mut offsets = Vec::new();
    for (start, ch) in text.char_indices() {
        let value = match mode {
            Mode::Path if ch == '\\' => "/".to_owned(),
            Mode::Pinyin | Mode::Initials => ch.to_pinyin().map_or_else(
                || ch.to_lowercase().collect(),
                |pinyin| match mode {
                    Mode::Initials => pinyin.first_letter().to_owned(),
                    _ => pinyin.plain().to_owned(),
                },
            ),
            _ => ch.to_lowercase().collect(),
        };
        offsets.extend(std::iter::repeat_n(
            start..start + ch.len_utf8(),
            value.len(),
        ));
        normalized.push_str(&value);
    }
    // Preserve contextual lowercase (for example Greek final sigma).
    if matches!(mode, Mode::Literal | Mode::Path) {
        normalized = text.to_lowercase();
        if matches!(mode, Mode::Path) {
            normalized = normalized.replace('\\', "/");
        }
    }
    (normalized, offsets)
}

fn match_ranges(
    text: &str,
    offsets: &[Range<usize>],
    parts: &[String],
) -> Option<Vec<Range<usize>>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    if parts.is_empty() {
        return Some(ranges);
    }
    let mut cursor = 0;
    // Highlight each complete occurrence; wildcard segments must stay ordered.
    loop {
        let mut occurrence = Vec::new();
        for part in parts {
            let Some(index) = text[cursor..].find(part.as_str()) else {
                return (!ranges.is_empty()).then_some(ranges);
            };
            let start = cursor + index;
            cursor = start + part.len();
            occurrence.push(offsets[start].start..offsets[cursor - 1].end);
        }
        for range in occurrence {
            if let Some(last) = ranges.last_mut().filter(|last| range.start <= last.end) {
                last.end = last.end.max(range.end);
            } else {
                ranges.push(range);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_case_wildcards_and_repeated_occurrences() {
        assert_eq!(
            SearchHighlights::new("rot").name_ranges("Rotor-ROTOR"),
            vec![0..3, 6..9]
        );
        assert_eq!(
            SearchHighlights::new("r*t").name_ranges("report.txt"),
            vec![0..1, 5..6]
        );
        assert_eq!(
            SearchHighlights::new("t*r").name_ranges("rotor"),
            vec![2..3, 4..5]
        );
        assert!(SearchHighlights::new("z*r").name_ranges("rotor").is_empty());
        assert!(SearchHighlights::new("***").name_ranges("rotor").is_empty());
    }

    #[test]
    fn maps_unicode_and_pinyin_back_to_original_bytes() {
        assert_eq!(SearchHighlights::new("i").name_ranges("İ.txt"), vec![0..2]);
        assert_eq!(SearchHighlights::new("ος").name_ranges("ΟΣ"), vec![0..4]);
        assert_eq!(
            SearchHighlights::new("微信").name_ranges("微信.txt"),
            vec![0..6]
        );
        assert_eq!(
            SearchHighlights::new("weixin").name_ranges("微信.txt"),
            vec![0..6]
        );
        assert_eq!(
            SearchHighlights::new("w*jt").name_ranges("微信截图.png"),
            vec![0..3, 6..12]
        );
    }

    #[test]
    fn separates_path_and_filename_segments() {
        let query = SearchHighlights::new("docs\\*.RS");
        assert_eq!(query.path_ranges("C:\\Docs\\src"), vec![3..7]);
        assert_eq!(query.name_ranges("main.rs"), vec![4..7]);
        assert!(SearchHighlights::new("main")
            .path_ranges("C:\\main")
            .is_empty());
    }
}
