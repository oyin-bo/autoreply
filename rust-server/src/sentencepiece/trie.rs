use std::ops::Range;

#[derive(Debug, Clone, Default)]
struct NodeBuilder {
    children: Vec<(char, usize)>,
    terminal_id: Option<u32>,
}

impl NodeBuilder {
    fn child_or_insert(&mut self, ch: char, arena: &mut Vec<NodeBuilder>) -> usize {
        match self.children.binary_search_by(|&(c, _)| c.cmp(&ch)) {
            Ok(idx) => self.children[idx].1,
            Err(idx) => {
                let next_index = arena.len();
                arena.push(NodeBuilder::default());
                self.children.insert(idx, (ch, next_index));
                next_index
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Node {
    terminal_id: Option<u32>,
    edge_start: u32,
    edge_len: u32,
}

impl Node {
    fn edge_range(&self) -> Range<usize> {
        let start = self.edge_start as usize;
        start..start + self.edge_len as usize
    }
}

#[derive(Debug, Clone)]
struct Edge {
    label: char,
    target: u32,
}

#[derive(Debug, Clone)]
pub struct VocabularyTrie {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

impl VocabularyTrie {
    pub fn from_pieces<'a>(pieces: impl IntoIterator<Item = (&'a [char], u32)>) -> Self {
        let mut arena = vec![NodeBuilder::default()];

        for (chars, id) in pieces {
            let mut current = 0usize;
            for &ch in chars {
                current = {
                    let node = &mut arena[current];
                    node.child_or_insert(ch, &mut arena)
                };
            }
            arena[current].terminal_id = Some(id);
        }

        let mut nodes = Vec::with_capacity(arena.len());
        let mut edges = Vec::new();

        for builder in arena.into_iter() {
            let start = edges.len() as u32;
            for (label, target) in builder.children.iter() {
                edges.push(Edge {
                    label: *label,
                    target: (*target)
                        .try_into()
                        .expect("trie node index exceeds u32 range"),
                });
            }
            nodes.push(Node {
                terminal_id: builder.terminal_id,
                edge_start: start,
                edge_len: (edges.len() as u32) - start,
            });
        }

        Self { nodes, edges }
    }

    pub fn common_prefix_search(
        &self,
        chars: &[char],
        scratch: &mut LookupScratch,
        mut callback: impl FnMut(usize, u32),
    ) {
        scratch.clear();

        let mut current = 0usize;
        for (depth, &ch) in chars.iter().enumerate() {
            let Some(next) = self.find_child(current, ch) else {
                break;
            };

            current = next;
            if let Some(id) = self.nodes[current].terminal_id {
                scratch.matches.push((depth + 1, id));
            }
        }

        for &(len, id) in &scratch.matches {
            callback(len, id);
        }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    fn find_child(&self, node_idx: usize, ch: char) -> Option<usize> {
        let node = &self.nodes[node_idx];
        let range = node.edge_range();
        self.edges[range]
            .binary_search_by(|edge| edge.label.cmp(&ch))
            .ok()
            .map(|idx| self.edges[range][idx].target as usize)
    }
}

#[derive(Debug, Default, Clone)]
pub struct LookupScratch {
    matches: Vec<(usize, u32)>,
}

impl LookupScratch {
    pub fn clear(&mut self) {
        self.matches.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_and_finds_prefixes() {
        let piece1: Vec<char> = "abc".chars().collect();
        let piece2: Vec<char> = "abcd".chars().collect();
        let piece3: Vec<char> = "abd".chars().collect();
        let trie = VocabularyTrie::from_pieces(vec![
            (&piece1[..], 1),
            (&piece2[..], 2),
            (&piece3[..], 3),
        ]);

        let mut scratch = LookupScratch::default();
        let mut results = Vec::new();
        trie.common_prefix_search(&"abcdef".chars().collect::<Vec<_>>(), &mut scratch, |len, id| {
            results.push((len, id));
        });

        assert_eq!(results, vec![(3, 1), (4, 2)]);
    }

    #[test]
    fn empty_when_branch_missing() {
        let piece: Vec<char> = "a".chars().collect();
        let trie = VocabularyTrie::from_pieces(vec![(&piece[..], 1)]);
        let mut scratch = LookupScratch::default();
        let mut results = Vec::new();
        trie.common_prefix_search(&"bc".chars().collect::<Vec<_>>(), &mut scratch, |len, id| {
            results.push((len, id));
        });
        assert!(results.is_empty());
    }
}
