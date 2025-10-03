use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TrieNode {
    pub children: HashMap<char, TrieNode>,
    pub terminal_id: Option<u32>,
}

impl TrieNode {
    pub fn new() -> Self {
        Self {
            children: HashMap::new(),
            terminal_id: None,
        }
    }

    pub fn insert(&mut self, chars: &[char], vocab_id: u32) {
        let mut node = self;
        for &ch in chars {
            node = node.children.entry(ch).or_insert_with(TrieNode::new);
        }
        node.terminal_id = Some(vocab_id);
    }

    pub fn common_prefix_search<'a>(&'a self, chars: &'a [char], mut callback: impl FnMut(usize, u32)) {
        let mut node = self;
        for (idx, &ch) in chars.iter().enumerate() {
            match node.children.get(&ch) {
                Some(child) => {
                    node = child;
                    if let Some(id) = node.terminal_id {
                        callback(idx + 1, id);
                    }
                }
                None => break,
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct VocabularyTrie {
    root: TrieNode,
}

impl VocabularyTrie {
    pub fn from_iter(pieces: impl IntoIterator<Item = (Vec<char>, u32)>) -> Self {
        let mut root = TrieNode::new();
        for (chars, id) in pieces {
            root.insert(&chars, id);
        }
        Self { root }
    }

    pub fn common_prefix_search<'a>(&'a self, chars: &'a [char], callback: impl FnMut(usize, u32)) {
        self.root.common_prefix_search(chars, callback);
    }
}
