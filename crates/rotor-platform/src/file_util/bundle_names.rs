use std::{collections::HashMap, error::Error, fs, io::Cursor, path::Path};

pub fn get_app_trans_names(app_path: &Path) -> Result<HashMap<String, String>, Box<dyn Error>> {
    let mut names = HashMap::new();
    for entry in fs::read_dir(app_path.join("Contents/Resources"))? {
        // One unreadable localization must not discard the others.
        let Ok(entry) = entry else { continue };
        let file_name = entry.file_name();
        let Some(language) = file_name
            .to_str()
            .and_then(|name| name.strip_suffix(".lproj"))
        else {
            continue;
        };
        let Ok(bytes) = fs::read(entry.path().join("InfoPlist.strings")) else {
            continue;
        };
        if let Some(name) = bundle_name(&bytes) {
            names.insert(language.to_owned(), name);
        }
    }
    Ok(names)
}

fn bundle_name(bytes: &[u8]) -> Option<String> {
    // Compiled .strings files may be binary or XML property lists.
    let plist = if bytes.starts_with(b"bplist00") {
        plist::Value::from_reader(Cursor::new(bytes))
    } else {
        plist::Value::from_reader_xml(bytes)
    };
    if let Ok(value) = plist {
        let dictionary = value.as_dictionary()?;
        return ["CFBundleDisplayName", "CFBundleName"]
            .into_iter()
            .filter_map(|key| dictionary.get(key).and_then(plist::Value::as_string))
            .find(|name| !name.trim().is_empty())
            .map(str::to_owned);
    }

    let encoding = if bytes.starts_with(&[0xff, 0xfe]) {
        encoding_rs::UTF_16LE
    } else if bytes.starts_with(&[0xfe, 0xff]) {
        encoding_rs::UTF_16BE
    } else if bytes.get(1) == Some(&0) {
        // Preserve support for BOM-less UTF-16 files beginning with an ASCII key/comment.
        encoding_rs::UTF_16LE
    } else if bytes.first() == Some(&0) {
        encoding_rs::UTF_16BE
    } else {
        encoding_rs::UTF_8
    };
    let (text, _, errors) = encoding.decode(bytes);
    if errors {
        return None;
    }
    let mut rest = text.as_ref();
    let mut display_name = None;
    let mut short_name = None;
    while !skip_trivia(&mut rest)?.is_empty() {
        let key = string_token(&mut rest)?;
        rest = skip_trivia(&mut rest)?.strip_prefix('=')?;
        let value = string_token(&mut rest)?;
        rest = skip_trivia(&mut rest)?.strip_prefix(';')?;
        if !value.trim().is_empty() {
            match key.as_str() {
                "CFBundleDisplayName" => display_name = Some(value),
                "CFBundleName" => short_name = Some(value),
                _ => {}
            }
        }
    }
    display_name.or(short_name)
}

fn skip_trivia<'a>(rest: &mut &'a str) -> Option<&'a str> {
    loop {
        *rest = rest.trim_start();
        if let Some(comment) = rest.strip_prefix("//") {
            *rest = comment.find('\n').map_or("", |end| &comment[end..]);
        } else if let Some(comment) = rest.strip_prefix("/*") {
            *rest = &comment[comment.find("*/")? + 2..];
        } else {
            return Some(rest);
        }
    }
}

// The text form of .strings is a sequence of key = value; entries, without
// a plist dictionary wrapper. Parse tokens so comments, multiline assignments
// and escaped quotes/Unicode cannot be mistaken for keys or delimiters.
fn string_token(rest: &mut &str) -> Option<String> {
    skip_trivia(rest)?;
    if !rest.starts_with('"') {
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || "_$/:.-".contains(c)))
            .unwrap_or(rest.len());
        if end == 0 {
            return None;
        }
        let token = rest[..end].to_owned();
        *rest = &rest[end..];
        return Some(token);
    }
    *rest = &rest[1..];
    let mut units = Vec::new();
    while let Some(c) = rest.chars().next() {
        *rest = &rest[c.len_utf8()..];
        let c = match c {
            '"' => return String::from_utf16(&units).ok(),
            '\\' => {
                let escaped = rest.chars().next()?;
                *rest = &rest[escaped.len_utf8()..];
                match escaped {
                    'U' | 'u' => {
                        units.push(u16::from_str_radix(rest.get(..4)?, 16).ok()?);
                        *rest = &rest[4..];
                        continue;
                    }
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    'a' => '\u{7}',
                    'b' => '\u{8}',
                    'f' => '\u{c}',
                    'v' => '\u{b}',
                    '\\' | '"' | '\'' => escaped,
                    _ => return None,
                }
            }
            _ => c,
        };
        units.extend_from_slice(c.encode_utf16(&mut [0; 2]));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_utf8_and_utf16_names_with_or_without_bom() {
        let source = "/* localized */\n\"CFBundleDisplayName\" = \"微信\";";
        let mut encodings = vec![source.as_bytes().to_vec()];
        encodings.push([b"\xef\xbb\xbf".as_slice(), source.as_bytes()].concat());
        for big_endian in [false, true] {
            let bytes: Vec<_> = source
                .encode_utf16()
                .flat_map(|unit| {
                    if big_endian {
                        unit.to_be_bytes()
                    } else {
                        unit.to_le_bytes()
                    }
                })
                .collect();
            let bom = if big_endian {
                [0xfe, 0xff]
            } else {
                [0xff, 0xfe]
            };
            encodings.push([bom.as_slice(), &bytes].concat());
            encodings.push(bytes);
        }
        for bytes in encodings {
            assert_eq!(bundle_name(&bytes).as_deref(), Some("微信"));
        }
    }

    #[test]
    fn parses_comments_multiline_entries_and_escapes() {
        let source = r#"
            /* CFBundleDisplayName = "wrong"; */
            CFBundleDisplayNameSuffix = "wrong";
            CFBundleName = "fallback"; // comment
            "CFBundleDisplayName"
                = "\U5FAE\U4FE1 \"Mac\" \\ \UD83D\UDE00";
        "#;
        assert_eq!(
            bundle_name(source.as_bytes()).as_deref(),
            Some("微信 \"Mac\" \\ 😀")
        );
    }

    #[test]
    fn falls_back_to_bundle_name_and_prefers_display_name() {
        for source in [
            "CFBundleName=\"微信\";",
            "CFBundleDisplayName=\" \";CFBundleName=\"微信\";",
            "CFBundleDisplayName=\"微信\";CFBundleName=WeChat;",
        ] {
            assert_eq!(bundle_name(source.as_bytes()).as_deref(), Some("微信"));
        }
    }

    #[test]
    fn reads_xml_and_binary_property_lists() {
        for key in ["CFBundleDisplayName", "CFBundleName"] {
            let mut dictionary = plist::Dictionary::new();
            dictionary.insert(key.into(), plist::Value::String("微信".into()));
            let value = plist::Value::Dictionary(dictionary);
            let mut xml = Vec::new();
            value.to_writer_xml(&mut xml).unwrap();
            let mut binary = Vec::new();
            value.to_writer_binary(&mut binary).unwrap();
            for bytes in [xml, binary] {
                assert_eq!(bundle_name(&bytes).as_deref(), Some("微信"));
            }
        }
    }

    #[test]
    fn ignores_invalid_localizations_without_losing_valid_names() {
        let root = tempfile::tempdir().unwrap();
        for (language, bytes) in [
            ("zh_CN", b"CFBundleName=\"\\U5FAE\\U4FE1\";".as_slice()),
            ("broken", b"\xff\xfe\x00".as_slice()),
            ("truncated", b"CFBundleName=\"unfinished".as_slice()),
        ] {
            let directory = root
                .path()
                .join(format!("Contents/Resources/{language}.lproj"));
            fs::create_dir_all(&directory).unwrap();
            fs::write(directory.join("InfoPlist.strings"), bytes).unwrap();
        }
        let names = get_app_trans_names(root.path()).unwrap();
        assert_eq!(names, HashMap::from([("zh_CN".into(), "微信".into())]));
    }
}
