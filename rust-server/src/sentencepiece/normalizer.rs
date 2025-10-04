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
}

impl NormalizerWorkspace {
    fn reserve(&mut self, capacity: usize, add_dummy_prefix: bool) {
        let needed = capacity + if add_dummy_prefix { 1 } else { 0 };
        self.units.reserve(needed.saturating_sub(self.units.capacity()));
        self.scratch.reserve(needed.saturating_sub(self.scratch.capacity()));
        self.chars.reserve(needed.saturating_sub(self.chars.capacity()));
        self.positions
            .reserve(needed.saturating_sub(self.positions.capacity()));
    }

    fn prepare(&mut self, capacity: usize, add_dummy_prefix: bool) {
        self.reserve(capacity, add_dummy_prefix);
        self.units.clear();
        self.scratch.clear();
        self.chars.clear();
        self.positions.clear();
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
    }
}

pub struct NormalizedString<'a> {
    workspace: RefMut<'a, NormalizerWorkspace>,
}

impl<'a> NormalizedString<'a> {
    pub fn chars(&self) -> &[char] {
        &self.workspace.chars
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

    pub fn to_string(&self) -> String {
        self.workspace.chars.iter().collect()
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
            workspace.units.push(CharUnit {
                ch: DEFAULT_SPACE_CHAR,
                origin: 0,
                is_dummy_prefix: true,
            });
        }

        let mut iter = input.char_indices().peekable();
        while let Some((byte_idx, ch)) = iter.next() {
            if let Some((replacement, consumed_bytes)) = self.rules.apply(&input[byte_idx..]) {
                if !replacement.is_empty() {
                    for &sub_ch in replacement {
                        workspace.units.push(CharUnit {
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

            workspace.units.push(CharUnit {
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
    replacement: Option<Vec<char>>,
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
            let dst_chars = hex_sequence_to_chars(dst_part);

            if src_chars.is_empty() {
                continue;
            }

{{ ... }}
            (" ｸﾞｰｸﾞﾙ ", "▁グーグル"),
        ];

        for (input, expected) in cases {
            let normalized = norm.normalize(input);
            assert_eq!(normalized.chars().iter().collect::<String>(), expected, "input: {input:?}");
        }
    }
    
    #[test]
    fn drops_control_characters_like_reference() {
        let norm = reference_normalizer();
        for &codepoint in &[0x7F, 0x8F, 0x9F, 0x0B] {
            let input = char::from_u32(codepoint).unwrap().to_string();
            let normalized = norm.normalize(&input);
            assert!(normalized.chars().iter().collect::<String>().is_empty(), "codepoint: U+{codepoint:04X}");
            assert!(normalized.is_empty(), "codepoint: U+{codepoint:04X}");
        }
    }
}
