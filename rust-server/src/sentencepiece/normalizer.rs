use unicode_normalization::UnicodeNormalization;

use super::proto::NormalizerSpec;

const DEFAULT_SPACE_CHAR: char = ' ';
const ESCAPED_WHITESPACE: char = '\u{2581}';

#[derive(Debug, Clone)]
pub struct Normalizer {
    add_dummy_prefix: bool,
    remove_extra_whitespaces: bool,
    escape_whitespaces: bool,
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
        Self {
            add_dummy_prefix: spec.add_dummy_prefix.unwrap_or(true),
            remove_extra_whitespaces: spec.remove_extra_whitespaces.unwrap_or(true),
            escape_whitespaces: spec.escape_whitespaces.unwrap_or(true),
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

        for (byte_idx, ch) in input.char_indices() {
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
}
