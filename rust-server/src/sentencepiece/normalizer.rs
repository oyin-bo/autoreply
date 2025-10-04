use std::{cell::{RefCell, RefMut}, collections::HashMap};

use unicode_normalization::UnicodeNormalization;

use super::proto::NormalizerSpec;

const DEFAULT_SPACE_CHAR: char = ' ';
const ESCAPED_WHITESPACE: char = '\u{2581}';

#[derive(Debug, Clone)]
struct CharUnit {
    ch: char,
    origin: usize,
    is_dummy_prefix: bool,
}

#[derive(Debug, Default, Clone)]
struct NormalizerWorkspace {
    units: Vec<CharUnit>,
    scratch: Vec<CharUnit>,
    chars: Vec<char>,
    positions: Vec<usize>,
    string: String,
}

impl NormalizerWorkspace {
    fn reserve(&mut self, capacity: usize, add_dummy_prefix: bool) {
        let needed = capacity + if add_dummy_prefix { 1 } else { 0 };
        self.units.reserve(needed.saturating_sub(self.units.capacity()));
        self.scratch.reserve(needed.saturating_sub(self.scratch.capacity()));
        self.chars.reserve(needed.saturating_sub(self.chars.capacity()));
        self.positions
            .reserve(needed.saturating_sub(self.positions.capacity()));
        self.string
            .reserve(needed.saturating_sub(self.string.capacity()));
    }

    fn prepare(&mut self, capacity: usize, add_dummy_prefix: bool) {
        self.reserve(capacity, add_dummy_prefix);
        self.units.clear();
        self.scratch.clear();
        self.chars.clear();
        self.positions.clear();
        self.string.clear();
    }

    fn push_unit(&mut self, unit: CharUnit) {
        self.units.push(unit);
    }

    fn apply_nfkc(&mut self) {
        self.scratch.clear();
        for unit in self.units.drain(..) {
            for normalized in std::iter::once(unit.ch).nfkc() {
                self.scratch.push(CharUnit {
                    ch: normalized,
                    origin: unit.origin,
                    is_dummy_prefix: unit.is_dummy_prefix,
                });
            }
        }
        std::mem::swap(&mut self.units, &mut self.scratch);
    }

    fn collapse_whitespace(&mut self) {
        self.scratch.clear();
        let mut prev_was_space = false;

        for unit in self.units.drain(..) {
            if unit.ch.is_whitespace() && !unit.is_dummy_prefix {
                if self.scratch.is_empty() || prev_was_space {
                    continue;
                }
                prev_was_space = true;
                self.scratch.push(CharUnit {
                    ch: DEFAULT_SPACE_CHAR,
                    origin: unit.origin,
                    is_dummy_prefix: false,
                });
            } else {
                prev_was_space = unit.ch.is_whitespace();
                self.scratch.push(unit);
            }
        }

        while matches!(self.scratch.last(), Some(last) if last.ch.is_whitespace() && !last.is_dummy_prefix) {
            self.scratch.pop();
        }

        std::mem::swap(&mut self.units, &mut self.scratch);
    }

    fn rebuild_outputs(&mut self) {
        self.chars.clear();
        self.positions.clear();
        self.chars.extend(self.units.iter().map(|u| u.ch));
        self.positions.extend(self.units.iter().map(|u| u.origin));
        self.string.clear();
        self.string.extend(self.chars.iter().copied());
    }
}

pub struct NormalizedString<'a> {
    workspace: RefMut<'a, NormalizerWorkspace>,
}

impl<'a> NormalizedString<'a> {
    pub fn as_str(&self) -> &str {
        &self.workspace.string
    }

    pub fn chars(&self) -> &[char] {
        &self.workspace.chars
    }

    pub fn to_string(&self) -> String {
        self.workspace.string.clone()
    }

    pub fn positions(&self) -> &[usize] {
        &self.workspace.positions
    }

    pub fn len(&self) -> usize {
        self.workspace.chars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.workspace.chars.is_empty()
    }

    pub fn slice(&self, start: usize, end: usize) -> String {
        self.workspace.chars[start..end].iter().collect()
    }
}

#[derive(Debug, Clone)]
pub struct Normalizer {
    add_dummy_prefix: bool,
    remove_extra_whitespaces: bool,
    escape_whitespaces: bool,
    rules: NormalizationTrie,
    workspace: RefCell<NormalizerWorkspace>,
}

impl Normalizer {
    pub fn from_spec(spec: &NormalizerSpec) -> Self {
        let rules = NormalizationTrie::from_spec(spec);
        Self {
            add_dummy_prefix: spec.add_dummy_prefix.unwrap_or(true),
            remove_extra_whitespaces: spec.remove_extra_whitespaces.unwrap_or(true),
            escape_whitespaces: spec.escape_whitespaces.unwrap_or(true),
            rules,
            workspace: RefCell::new(NormalizerWorkspace::default()),
        }
    }

    pub fn reserve(&self, capacity: usize) {
        let mut workspace = self.workspace.borrow_mut();
        workspace.reserve(capacity, self.add_dummy_prefix);
    }

    pub fn prewarm(&self, capacity: usize) {
        self.reserve(capacity);
        let _ = self.normalize(&"x".repeat(capacity));
    }

    pub fn normalize(&self, input: &str) -> NormalizedString {
        let mut workspace = self.workspace.borrow_mut();
        workspace.prepare(input.len(), self.add_dummy_prefix);

        if self.add_dummy_prefix {
            workspace.push_unit(CharUnit {
                ch: DEFAULT_SPACE_CHAR,
                origin: 0,
                is_dummy_prefix: true,
            });
        }

        let mut iter = input.char_indices().peekable();
        while let Some((byte_idx, ch)) = iter.next() {
            if let Some((replacement, consumed_bytes)) = self.rules.apply(&input[byte_idx..]) {
                if !replacement.is_empty() {
                    for sub_ch in replacement.chars() {
                        workspace.push_unit(CharUnit {
                            ch: sub_ch,
                            origin: byte_idx,
                            is_dummy_prefix: false,
                        });
                    }
                }

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

            workspace.push_unit(CharUnit {
                ch,
                origin: byte_idx,
                is_dummy_prefix: false,
            });
        }

        workspace.apply_nfkc();
        if self.remove_extra_whitespaces {
            workspace.collapse_whitespace();
        }

        if self.escape_whitespaces {
            for unit in &mut workspace.units {
                if unit.ch == DEFAULT_SPACE_CHAR {
                    unit.ch = ESCAPED_WHITESPACE;
                }
            }
        }

        workspace.rebuild_outputs();

        NormalizedString { workspace }
    }
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

            let src_chars = parse_hex_chars(src_part);
            let dst_string = hex_sequence_to_string(dst_part);

            if src_chars.is_empty() {
                continue;
            }

            self.insert(&src_chars, dst_string);
        }
    }

    fn insert(&mut self, src_chars: &[char], replacement: String) {
        let mut node = &mut self.root;
        for &ch in src_chars {
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

        last_match
    }
}

fn parse_hex_chars(part: &str) -> Vec<char> {
    let mut chars = Vec::new();
    for hex in part.split_whitespace() {
        if hex.starts_with('#') {
            break;
        }
        if hex.is_empty() {
            continue;
        }
        if let Ok(value) = u32::from_str_radix(hex, 16) {
            if let Some(ch) = char::from_u32(value) {
                chars.push(ch);
            }
        }
    }
    chars
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentencepiece::loader::SentencePieceModel;
    use std::path::Path;

    fn default_spec() -> NormalizerSpec {
        NormalizerSpec {
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        }
    }

    fn reference_normalizer() -> Normalizer {
        let path = Path::new("../sentencepiece/python/test/test_model.model");
        let model = SentencePieceModel::load_from_file(path).expect("load reference model");
        Normalizer::from_spec(model.normalizer_spec())
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

    #[test]
    fn applies_multi_codepoint_rule() {
        let spec = NormalizerSpec {
            add_dummy_prefix: Some(false),
            remove_extra_whitespaces: Some(false),
            escape_whitespaces: Some(false),
            normalization_rule_tsv: Some("41 0301\t62\n".to_string()),
            ..Default::default()
        };

        let norm = Normalizer::from_spec(&spec);
        let result = norm.normalize("A\u{0301}");
        assert_eq!(result.to_string(), "b");
        assert_eq!(result.positions(), &[0]);
    }

    #[test]
    fn matches_reference_basic_cases() {
        let norm = reference_normalizer();
        let cases = [
            ("", ""),
            ("      ", ""),
            ("ABC", "▁ABC"),
            (" ABC ", "▁ABC"),
            ("   ABC   ", "▁ABC"),
            ("①②③", "▁123"),
            (" ｸﾞｰｸﾞﾙ ", "▁グーグル"),
        ];

        for (input, expected) in cases {
            let normalized = norm.normalize(input);
            assert_eq!(normalized.to_string(), expected, "input: {input:?}");
        }
    }

    #[test]
    fn drops_control_characters_like_reference() {
        let norm = reference_normalizer();
        for &codepoint in &[0x7F, 0x8F, 0x9F, 0x0B] {
            let input = char::from_u32(codepoint).unwrap().to_string();
            let normalized = norm.normalize(&input);
            assert!(normalized.is_empty(), "codepoint: U+{codepoint:04X}");
        }
    }
}
