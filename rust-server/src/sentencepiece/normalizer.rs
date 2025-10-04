use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;

use super::proto::NormalizerSpec;

const DEFAULT_SPACE_CHAR: char = ' ';
const ESCAPED_WHITESPACE: char = '\u{2581}';

#[derive(Debug, Clone)]
pub struct Normalizer {
    add_dummy_prefix: bool,
    remove_extra_whitespaces: bool,
    escape_whitespaces: bool,
    rules: NormalizationTrie,
}

#[derive(Debug, Clone)]
pub struct NormalizedString {
    string: String,
    chars: Vec<char>,
    positions: Vec<usize>,
}

impl NormalizedString {
    pub fn as_str(&self) -> &str {
        &self.string
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    pub fn to_string(&self) -> String {
        self.string.clone()
    }

    pub fn positions(&self) -> &[usize] {
        &self.positions
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn slice(&self, start: usize, end: usize) -> String {
        self.chars[start..end].iter().collect()
    }
}

impl Normalizer {
    pub fn from_spec(spec: &NormalizerSpec) -> Self {
        let rules = NormalizationTrie::from_spec(spec);
        Self {
            add_dummy_prefix: spec.add_dummy_prefix.unwrap_or(true),
            remove_extra_whitespaces: spec.remove_extra_whitespaces.unwrap_or(true),
            escape_whitespaces: spec.escape_whitespaces.unwrap_or(true),
            rules,
        }
    }

    pub fn normalize(&self, input: &str) -> NormalizedString {
        let mut units = Vec::new();

        if self.add_dummy_prefix {
            units.push(CharUnit {
                ch: DEFAULT_SPACE_CHAR,
                origin: 0,
                is_dummy_prefix: true,
            });
        }

        let mut iter = input.char_indices().peekable();
        while let Some((byte_idx, ch)) = iter.next() {
            if let Some((replacement, consumed_bytes)) =
                self.rules.apply(&input[byte_idx..])
            {
                if !replacement.is_empty() {
                    for sub_ch in replacement.chars() {
                        units.push(CharUnit {
                            ch: sub_ch,
                            origin: byte_idx,
                            is_dummy_prefix: false,
                        });
                    }
                }

                // advance iterator to account for consumed bytes
                let target_end = byte_idx + consumed_bytes;
                while let Some(&(next_idx, _)) = iter.peek() {
                    if next_idx < target_end {
                        iter.next();
                    } else {
                        break;
                    }
                }
                continue;
            }

            units.push(CharUnit {
                ch,
                origin: byte_idx,
                is_dummy_prefix: false,
            });
        }

        units = apply_nfkc(units);

        if self.remove_extra_whitespaces {
            units = collapse_whitespace(units);
        }

        if self.escape_whitespaces {
            for unit in &mut units {
                if unit.ch == DEFAULT_SPACE_CHAR {
                    unit.ch = ESCAPED_WHITESPACE;
                }
            }
        }

        let chars: Vec<char> = units.iter().map(|u| u.ch).collect();
        let string: String = chars.iter().collect();
        let positions: Vec<usize> = units.iter().map(|u| u.origin).collect();

        NormalizedString {
            string,
            chars,
            positions,
        }
    }
}

#[derive(Debug, Clone)]
struct CharUnit {
    ch: char,
    origin: usize,
    is_dummy_prefix: bool,
}

#[derive(Debug, Clone, Default)]
struct NormalizationTrie {
    root: TrieNode,
}

#[derive(Debug, Clone, Default)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    replacement: Option<String>,
}

impl NormalizationTrie {
    fn from_spec(spec: &NormalizerSpec) -> Self {
        let mut trie = Self::default();
        if let Some(tables) = spec.normalization_rule_tsv.as_deref() {
            trie.load_rules(tables);
        }
        trie
    }

    fn load_rules(&mut self, tsv_data: &str) {
        for line in tsv_data.lines() {
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }

            let mut parts = line.splitn(2, '\t');
            let Some(src_part) = parts.next() else { continue };
            let Some(dst_part) = parts.next() else { continue };

            let src_bytes = parse_hex_sequence(src_part);
            let dst_string = hex_sequence_to_string(dst_part);

            if src_bytes.is_empty() {
                continue;
            }

            self.insert(&src_bytes, dst_string);
        }
    }

    fn insert(&mut self, src_bytes: &[u8], replacement: String) {
        let mut node = &mut self.root;
        let src_str = match std::str::from_utf8(src_bytes) {
            Ok(s) => s,
            Err(_) => return,
        };

        for ch in src_str.chars() {
            node = node.children.entry(ch).or_insert_with(TrieNode::default);
        }
        node.replacement = Some(replacement);
    }

    fn apply(&self, remaining: &str) -> Option<(String, usize)> {
        let mut node = &self.root;
        let mut last_match: Option<(String, usize)> = None;

        for (idx, ch) in remaining.char_indices() {
            let Some(next) = node.children.get(&ch) else {
                break;
            };

            node = next;
            if let Some(rep) = &node.replacement {
                let offset = idx + ch.len_utf8();
                last_match = Some((rep.clone(), offset));
            }
        }

        if last_match.is_none() {
            if let Some(rep) = &node.replacement {
                let bytes = remaining
                    .chars()
                    .next()
                    .map(|ch| ch.len_utf8())
                    .unwrap_or(0);
                return Some((rep.clone(), bytes));
            }
        }

        last_match
    }
}

fn parse_hex_sequence(part: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for hex in part.split_whitespace() {
        if hex.starts_with('#') {
            break;
        }
        if let Ok(value) = u32::from_str_radix(hex, 16) {
            if let Some(ch) = char::from_u32(value) {
                let mut buf = [0u8; 4];
                let encoded = ch.encode_utf8(&mut buf);
                bytes.extend_from_slice(encoded.as_bytes());
            }
        }
    }
    bytes
}

fn hex_sequence_to_string(part: &str) -> String {
    let mut out = String::new();
    for hex in part.split_whitespace() {
        if hex.starts_with('#') {
            break;
        }
        if hex.is_empty() {
            continue;
        }
        if let Ok(value) = u32::from_str_radix(hex, 16) {
            if let Some(ch) = char::from_u32(value) {
                out.push(ch);
            }
        }
    }
    out
}

fn apply_nfkc(units: Vec<CharUnit>) -> Vec<CharUnit> {
    let mut out = Vec::with_capacity(units.len());
    for unit in units {
        for normalized in std::iter::once(unit.ch).nfkc() {
            out.push(CharUnit {
                ch: normalized,
                origin: unit.origin,
                is_dummy_prefix: unit.is_dummy_prefix,
            });
        }
    }
    out
}

fn collapse_whitespace(units: Vec<CharUnit>) -> Vec<CharUnit> {
    let mut out = Vec::with_capacity(units.len());
    let mut prev_was_space = false;

    for unit in units.into_iter() {
        if unit.ch.is_whitespace() && !unit.is_dummy_prefix {
            if out.is_empty() {
                continue;
            }
            if prev_was_space {
                continue;
            }
            prev_was_space = true;
            out.push(CharUnit {
                ch: DEFAULT_SPACE_CHAR,
                origin: unit.origin,
                is_dummy_prefix: false,
            });
        } else {
            prev_was_space = unit.ch.is_whitespace();
            out.push(unit);
        }
    }

    while matches!(out.last(), Some(last) if last.ch.is_whitespace() && !last.is_dummy_prefix) {
        out.pop();
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_spec() -> NormalizerSpec {
        NormalizerSpec {
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        }
    }

    #[test]
    fn applies_dummy_prefix_and_escape() {
        let norm = Normalizer::from_spec(&default_spec());
        let result = norm.normalize("Hello  Wo\u{0301}rld");
        assert_eq!(result.to_string(), "▁Hello▁Wórld");
        assert_eq!(result.positions()[0], 0);
    }

    #[test]
    fn collapses_whitespace() {
        let spec = NormalizerSpec {
            add_dummy_prefix: Some(false),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let norm = Normalizer::from_spec(&spec);
        let result = norm.normalize("hello   world  ");
        assert_eq!(result.to_string(), "hello▁world");
    }

    #[test]
    fn preserves_positions() {
        let norm = Normalizer::from_spec(&default_spec());
        let result = norm.normalize("abc");
        assert_eq!(result.positions().len(), result.chars().len());
        assert_eq!(result.positions()[0], 0);
        assert_eq!(result.positions()[1], 0);
        assert_eq!(result.positions()[2], 0);
        assert_eq!(result.positions()[3], 1);
        assert_eq!(result.positions()[4], 2);
    }

    #[test]
    fn applies_normalization_rule_tsv() {
        let spec = NormalizerSpec {
            add_dummy_prefix: Some(false),
            remove_extra_whitespaces: Some(false),
            escape_whitespaces: Some(false),
            normalization_rule_tsv: Some("41\t42\n41 41\t43\t# longest match\n".to_string()),
            ..Default::default()
        };

        let norm = Normalizer::from_spec(&spec);
        let result = norm.normalize("AAA");
        assert_eq!(result.to_string(), "CB");
    }
}
