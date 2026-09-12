use pinyin::ToPinyin;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SearchAlias {
    pub text: String,
    pub display_alias_index: Option<usize>,
}

pub(super) struct PreparedSearchName {
    pub filter: u32,
    pub aliases: Option<Box<[SearchAlias]>>,
}

pub(super) fn prepare_search_name(
    file_name: &str,
    display_aliases: Option<&[String]>,
) -> PreparedSearchName {
    let mut filter = make_filter(file_name);
    let mut search_aliases = Vec::new();

    if let Some(display_aliases) = display_aliases {
        for (index, alias) in display_aliases.iter().enumerate() {
            filter |= make_filter(alias);
            push_pinyin_aliases(alias, Some(index), &mut search_aliases);
        }
    }

    push_pinyin_aliases(file_name, None, &mut search_aliases);
    for alias in &search_aliases {
        filter |= make_filter(&alias.text);
    }

    PreparedSearchName {
        filter,
        aliases: (!search_aliases.is_empty()).then(|| search_aliases.into_boxed_slice()),
    }
}

/// Prepare wildcard segments once and reuse the ASCII normalization buffer for
/// every candidate. Unicode names keep Rust's contextual lowercase semantics.
pub(super) struct SearchQuery {
    filter: u32,
    parts: Vec<String>,
    scratch: String,
}

impl SearchQuery {
    pub fn new(query: &str) -> Self {
        let lower = query.to_lowercase();
        Self {
            filter: make_filter(&lower),
            parts: lower
                .split('*')
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect(),
            scratch: String::new(),
        }
    }

    fn matches(&mut self, name: &str) -> bool {
        if name.is_ascii() {
            self.scratch.clear();
            self.scratch.push_str(name);
            self.scratch.make_ascii_lowercase();
        } else {
            self.scratch = name.to_lowercase();
        }
        matches_parts(&self.scratch, &self.parts)
    }

    pub fn match_name(
        &mut self,
        file_name: &str,
        display_aliases: Option<&[String]>,
        search_aliases: Option<&[SearchAlias]>,
        filter: u32,
    ) -> Option<Option<String>> {
        if (filter & self.filter) != self.filter {
            return None;
        }
        if self.matches(file_name) {
            return Some(None);
        }
        if let Some(aliases) = display_aliases {
            for alias in aliases {
                if self.matches(alias) {
                    return Some(Some(alias.clone()));
                }
            }
        }
        if let Some(aliases) = search_aliases {
            for alias in aliases {
                // Pinyin aliases were already lowercased during indexing.
                if matches_parts(&alias.text, &self.parts) {
                    let display_alias = alias
                        .display_alias_index
                        .and_then(|index| display_aliases.and_then(|aliases| aliases.get(index)))
                        .cloned();
                    return Some(display_alias);
                }
            }
        }
        None
    }
}

fn matches_parts(mut name: &str, parts: &[String]) -> bool {
    for part in parts {
        let Some(index) = name.find(part.as_str()) else {
            return false;
        };
        name = &name[index + part.len()..];
    }
    true
}

// Calculates a 32bit value that is used to filter out many files before comparing their filenames.
pub(super) fn make_filter(str: &str) -> u32 {
    /*
    Creates an address that is used to filter out strings that don't contain the queried characters
    Explanation of the meaning of the single bits:
    0-25 a-z
    26 0-9
    27 other ASCII
    28 not in ASCII
    */
    let mut address: u32 = 0;
    for c in str.chars().flat_map(char::to_lowercase) {
        if c == '*' {
            continue; // Reserved for wildcard
        } else if c.is_ascii_lowercase() {
            address |= 1 << (c as u32 - 97);
        } else if c.is_ascii_digit() {
            address |= 1 << 26;
        } else if c < 127u8 as char {
            address |= 1 << 27;
        } else {
            address |= 1 << 28;
        }
    }
    address
}

fn push_pinyin_aliases(
    source: &str,
    display_alias_index: Option<usize>,
    aliases: &mut Vec<SearchAlias>,
) {
    let mut full = String::new();
    let mut initials = String::new();
    let mut has_pinyin = false;

    for ch in source.chars() {
        if let Some(pinyin) = ch.to_pinyin() {
            has_pinyin = true;
            full.push_str(pinyin.plain());
            initials.push_str(pinyin.first_letter());
        } else {
            for lower in ch.to_lowercase() {
                full.push(lower);
                initials.push(lower);
            }
        }
    }

    if !has_pinyin {
        return;
    }

    push_unique_alias(aliases, full, display_alias_index);
    push_unique_alias(aliases, initials, display_alias_index);
}

fn push_unique_alias(
    aliases: &mut Vec<SearchAlias>,
    text: String,
    display_alias_index: Option<usize>,
) {
    if text.is_empty() || aliases.iter().any(|alias| alias.text == text) {
        return;
    }

    aliases.push(SearchAlias {
        text,
        display_alias_index,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn match_query(
        file_name: &str,
        display_aliases: Option<&[String]>,
        query: &str,
    ) -> Option<Option<String>> {
        let prepared = prepare_search_name(file_name, display_aliases);
        SearchQuery::new(query).match_name(
            file_name,
            display_aliases,
            prepared.aliases.as_deref(),
            prepared.filter,
        )
    }

    #[test]
    fn wildcard_order_case_and_unicode_match_existing_semantics() {
        let names = [
            "",
            "a",
            "AaAa.txt",
            "Report-2026.PDF",
            "a*b.txt",
            "你好🦀.PNG",
            "ΟΣ.txt",
            "ΟΣΑ.txt",
            "İstanbul.txt",
            "Straße.txt",
            "ÉCOLE.txt",
        ];
        let queries = [
            "",
            "*",
            "**",
            "a",
            "aa*aa",
            "aa*aaa",
            "*a**.TXT*",
            "report*pdf",
            "pdf*report",
            "你*🦀",
            "ΟΣ",
            "οσ",
            "ος",
            "İ",
            "i̇",
            "straße",
            "STRASSE",
            "école",
            "missing",
        ];
        for query in queries {
            let lower = query.to_lowercase();
            let mut prepared = SearchQuery::new(query);
            for name in names {
                // Reference the previous sequential wildcard contract, including
                // Unicode expansions and contextual Greek final sigma.
                let name_lower = name.to_lowercase();
                let mut offset = 0;
                let expected = lower.split('*').all(|part| {
                    if let Some(index) = name_lower[offset..].find(part) {
                        offset += index + part.len();
                        true
                    } else {
                        false
                    }
                });
                assert_eq!(
                    prepared
                        .match_name(name, None, None, make_filter(name))
                        .is_some(),
                    expected,
                    "name={name:?}, query={query:?}"
                );
            }
        }
    }

    #[test]
    fn filename_and_display_alias_precede_pinyin_aliases() {
        let aliases = ["微信截图".into(), "微信".into()];
        assert_eq!(match_query("wx.app", Some(&aliases), "wx"), Some(None));
        assert_eq!(
            match_query("WeChat.app", Some(&aliases), "微信"),
            Some(Some("微信截图".into()))
        );
        assert_eq!(
            match_query("WeChat.app", Some(&aliases), "w*x"),
            Some(Some("微信截图".into()))
        );
    }

    #[test]
    fn matches_full_pinyin_alias() {
        assert_eq!(match_query("微信.app", None, "weixin"), Some(None));
    }

    #[test]
    fn matches_pinyin_initials_alias() {
        assert_eq!(match_query("微信.app", None, "wx"), Some(None));
    }

    #[test]
    fn matches_pinyin_alias_with_wildcard() {
        assert_eq!(match_query("微信截图.png", None, "w*jt"), Some(None));
    }

    #[test]
    fn keeps_display_alias_for_translated_app_name() {
        let aliases = ["微信".to_string()];
        assert_eq!(
            match_query("WeChat.app", Some(&aliases), "wx"),
            Some(Some("微信".to_string()))
        );
    }
}
