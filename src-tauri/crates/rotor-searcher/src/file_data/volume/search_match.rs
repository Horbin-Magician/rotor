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

pub(super) fn match_indexed_name(
    file_name: &str,
    display_aliases: Option<&[String]>,
    search_aliases: Option<&[SearchAlias]>,
    filter: u32,
    query_lower: &str,
    query_filter: u32,
) -> Option<Option<String>> {
    if (filter & query_filter) != query_filter {
        return None;
    }

    if match_str(file_name, query_lower) {
        return Some(None);
    }

    if let Some(display_aliases) = display_aliases {
        for alias in display_aliases {
            if match_str(alias, query_lower) {
                return Some(Some(alias.clone()));
            }
        }
    }

    if let Some(search_aliases) = search_aliases {
        for alias in search_aliases {
            if match_str(&alias.text, query_lower) {
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
    let len = str.len();
    if len == 0 {
        return 0;
    }
    let mut address: u32 = 0;
    let str_lower = str.to_lowercase();

    for c in str_lower.chars() {
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

// Return true if contain query.
fn match_str(contain: &str, query_lower: &str) -> bool {
    let lower_contain = contain.to_lowercase();
    let mut offset = 0;
    for s in query_lower.split('*') {
        // for wildcard
        if let Some(index) = lower_contain[offset..].find(s) {
            offset += index + s.len();
        } else {
            return false;
        }
    }
    true
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
        let query_lower = query.to_lowercase();
        let query_filter = make_filter(&query_lower);

        match_indexed_name(
            file_name,
            display_aliases,
            prepared.aliases.as_deref(),
            prepared.filter,
            &query_lower,
            query_filter,
        )
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
