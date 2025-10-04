use std::{cell::RefCell, path::Path};

use thiserror::Error;

use super::{
    loader::{SentencePieceError, SentencePieceModel, SentencePieceType},
    normalizer::{NormalizedString, Normalizer},
    trie::{LookupScratch, VocabularyTrie},
};

const GEMMA_MODEL: &str = "../gemini-data/tokenizer.model";
const UNK_PENALTY: f32 = 10.0;
const SCORE_EPS: f32 = 1e-6;

#[derive(Debug, Clone, Copy)]
pub enum DecodeStrategy {
    BestPath,
    Sample {
        temperature: f32,
        nbest_size: usize,
    },
    NBest {
        size: usize,
    },
}

impl Default for DecodeStrategy {
    fn default() -> Self {
        DecodeStrategy::BestPath
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EncodeOptions {
    pub add_bos: bool,
    pub add_eos: bool,
    pub strategy: DecodeStrategy,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            add_bos: false,
            add_eos: false,
            strategy: DecodeStrategy::BestPath,
        }
    }
}

#[derive(Debug, Error)]
pub enum TokenizerError {
    #[error("sentencepiece model error: {0}")]
    Model(#[from] SentencePieceError),
    #[error("failed to tokenize input")]
    DecodeFailed,
    #[error("strategy not yet supported: {0:?}")]
    UnsupportedStrategy(DecodeStrategy),
}

#[derive(Debug, Clone)]
pub struct SentencePieceProcessor {
    model: SentencePieceModel,
    normalizer: Normalizer,
    trie: VocabularyTrie,
    lookup_scratch: RefCell<LookupScratch>,
    viterbi_workspace: RefCell<ViterbiWorkspace>,
    unk_id: u32,
    unk_score: f32,
    bos_id: Option<u32>,
    eos_id: Option<u32>,
    options: EncodeOptions,
}

impl SentencePieceProcessor {
    pub fn new(model: SentencePieceModel) -> Self {
        Self::with_options(model, EncodeOptions::default())
    }

    pub fn with_options(model: SentencePieceModel, options: EncodeOptions) -> Self {
        let normalizer = Normalizer::from_spec(model.normalizer_spec());
        let trie = {
            let storage = model.storage();
            VocabularyTrie::from_pieces(model.vocab().iter().filter_map(|piece| {
                match piece.kind {
                    SentencePieceType::Unused | SentencePieceType::Control => None,
                    _ => Some((storage.piece_chars(piece), piece.id)),
                }
            }))
        };

        let unk_id = model.unk_id;
        let unk_score = model
            .piece(unk_id)
            .map(|p| p.score)
            .unwrap_or(-100.0);
        let bos_id = model.bos_id;
        let eos_id = model.eos_id;

        Self {
            model,
            normalizer,
            trie,
            lookup_scratch: RefCell::new(LookupScratch::default()),
            viterbi_workspace: RefCell::new(ViterbiWorkspace::default()),
            unk_id,
            unk_score,
            bos_id,
            eos_id,
            options,
        }
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, TokenizerError> {
        let model = SentencePieceModel::load_from_file(path)?;
        Ok(Self::new(model))
    }

    pub fn normalize(&self, text: &str) -> NormalizedString {
        self.normalizer.normalize(text)
    }

    pub fn model(&self) -> &SentencePieceModel {
        &self.model
    }

    pub fn unk_id(&self) -> u32 {
        self.unk_id
    }

    pub fn piece_text(&self, id: u32) -> Option<&str> {
        self.model.piece_text(id)
    }

    pub fn tokens_to_pieces(&self, ids: &[u32]) -> Vec<String> {
        ids.iter()
            .map(|&id| self.piece_text(id).unwrap_or("<unk>").to_string())
            .collect()
    }

    pub fn encode(&self, text: &str) -> Result<Vec<u32>, TokenizerError> {
        self.encode_with(text, self.options)
    }

    pub fn encode_with(&self, text: &str, options: EncodeOptions) -> Result<Vec<u32>, TokenizerError> {
        let mut out = Vec::new();
        self.encode_into(text, options, &mut out)?;
        Ok(out)
    }

    pub fn encode_into(
        &self,
        text: &str,
        options: EncodeOptions,
        out: &mut Vec<u32>,
    ) -> Result<(), TokenizerError> {
        let normalized = self.normalize(text);
        let mut scratch = self.lookup_scratch.borrow_mut();
        let mut workspace = self.viterbi_workspace.borrow_mut();
        let tokens = match options.strategy {
            DecodeStrategy::BestPath => encode_normalized(
                &self.trie,
                &self.model,
                &normalized,
                self.unk_id,
                self.unk_score,
                options,
                self.bos_id,
                self.eos_id,
                &mut workspace,
                &mut scratch,
            ),
            strategy => Err(TokenizerError::UnsupportedStrategy(strategy)),
        }?;

        out.clear();
        out.extend_from_slice(tokens);
        Ok(())
    }
}

fn encode_normalized(
    trie: &VocabularyTrie,
    model: &SentencePieceModel,
    normalized: &NormalizedString,
    unk_id: u32,
    unk_score: f32,
    options: EncodeOptions,
    bos_id: Option<u32>,
    eos_id: Option<u32>,
    workspace: &mut ViterbiWorkspace,
    scratch: &mut LookupScratch,
) -> Result<&[u32], TokenizerError> {
    let chars = normalized.chars();
    let len = chars.len();

    workspace.prepare(len);
    workspace.best_scores[0] = 0.0;
    scratch.matches.clear();

    for pos in 0..len {
        if workspace.best_scores[pos].is_infinite() {
            continue;
        }

        let mut matched = false;
        trie.common_prefix_search(&chars[pos..], scratch, |match_len, piece_id| {
            if let Some(piece) = model.piece(piece_id) {
                let end = pos + match_len;
                let score = workspace.best_scores[pos] + piece.score;
                let should_replace = if score > workspace.best_scores[end] + SCORE_EPS {
                    true
                } else if (score - workspace.best_scores[end]).abs() <= SCORE_EPS {
                    match workspace.back_ptrs[end] {
                        Some((prev_start, prev_piece)) => {
                            let prev_len = end - prev_start;
                            if match_len > prev_len {
                                true
                            } else if match_len == prev_len {
                                piece_id < prev_piece
                            } else {
                                false
                            }
                        }
                        None => true,
                    }
                } else {
                    false
                };

                if should_replace {
                    workspace.best_scores[end] = score;
                    workspace.back_ptrs[end] = Some((pos, piece_id));
                }
                matched = true;
            }
        });

        if !matched {
            let end = pos + 1;
            let score = workspace.best_scores[pos] + unk_score - UNK_PENALTY;
            let should_replace = if score > workspace.best_scores[end] + SCORE_EPS {
                true
            } else if (score - workspace.best_scores[end]).abs() <= SCORE_EPS {
                match workspace.back_ptrs[end] {
                    Some((prev_start, prev_piece)) => {
                        let prev_len = end - prev_start;
                        if 1 > prev_len {
                            true
                        } else if prev_len == 1 {
                            unk_id < prev_piece
                        } else {
                            false
                        }
                    }
                    None => true,
                }
            } else {
                false
            };

            if should_replace {
                workspace.best_scores[end] = score;
                workspace.back_ptrs[end] = Some((pos, unk_id));
            }
        }
    }

    if workspace.best_scores[len].is_infinite() {
        return Err(TokenizerError::DecodeFailed);
    }

    let tokens = &mut workspace.token_buffer;
    tokens.clear();
    let mut pos = len;
    while pos > 0 {
        let (prev, piece_id) = workspace.back_ptrs[pos].ok_or(TokenizerError::DecodeFailed)?;
        tokens.push(piece_id);
        pos = prev;
    }
    tokens.reverse();

    if options.add_bos {
        if let Some(id) = bos_id {
            tokens.insert(0, id);
        }
    }

    if options.add_eos {
        if let Some(id) = eos_id {
            tokens.push(id);
        }
    }

    Ok(&workspace.token_buffer)
}

#[derive(Debug, Default)]
struct ViterbiWorkspace {
    best_scores: Vec<f32>,
    back_ptrs: Vec<Option<(usize, u32)>>,
    token_buffer: Vec<u32>,
}

impl ViterbiWorkspace {
    fn prepare(&mut self, len: usize) {
        let cap = len + 1;
        if self.best_scores.len() < cap {
            self.best_scores.resize(cap, f32::NEG_INFINITY);
        }
        if self.back_ptrs.len() < cap {
            self.back_ptrs.resize(cap, None);
        }
        self.best_scores[..cap].fill(f32::NEG_INFINITY);
        self.back_ptrs[..cap].fill(None);
        if self.token_buffer.capacity() < cap + 2 {
            self.token_buffer.reserve(cap + 2 - self.token_buffer.capacity());
        }
        self.token_buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentencepiece::proto::{self, ModelProto};
    use std::path::Path;

    fn build_test_model() -> SentencePieceModel {
        let pieces = vec![
            make_piece("<unk>", -10.0, proto::model_proto::sentence_piece::Type::Unknown),
            make_piece("<s>", 0.0, proto::model_proto::sentence_piece::Type::Control),
            make_piece("</s>", 0.0, proto::model_proto::sentence_piece::Type::Control),
            make_piece("▁", -1.0, proto::model_proto::sentence_piece::Type::Normal),
            make_piece("▁Hello", -0.1, proto::model_proto::sentence_piece::Type::Normal),
            make_piece("world", -0.2, proto::model_proto::sentence_piece::Type::Normal),
            make_piece("▁world", -0.3, proto::model_proto::sentence_piece::Type::Normal),
            make_piece("!", -0.5, proto::model_proto::sentence_piece::Type::Normal),
        ];

        let trainer = proto::TrainerSpec {
            unk_id: Some(0),
            bos_id: Some(1),
            eos_id: Some(2),
            pad_id: Some(-1),
            ..Default::default()
        };
        let normalizer = proto::NormalizerSpec {
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };

        let proto = ModelProto {
            pieces,
            trainer_spec: Some(trainer),
            normalizer_spec: Some(normalizer),
            ..Default::default()
        };

        SentencePieceModel::from_proto(proto).expect("model")
    }

    fn make_piece(
        text: &str,
        score: f32,
        kind: proto::model_proto::sentence_piece::Type,
    ) -> proto::model_proto::SentencePiece {
        proto::model_proto::SentencePiece {
            piece: Some(text.to_string()),
            score: Some(score),
            r#type: Some(kind as i32),
            ..Default::default()
        }
    }

    #[test]
    fn encodes_basic_sentence() {
        let model = build_test_model();
        let processor = SentencePieceProcessor::new(model.clone());
        let tokens = processor.encode("Hello world!").expect("tokens");
        let pieces = processor.tokens_to_pieces(&tokens);
        assert_eq!(pieces, vec!["▁Hello", "▁world", "!"]);
    }

    #[test]
    fn encodes_with_bos_eos() {
        let model = build_test_model();
        let processor = SentencePieceProcessor::with_options(
            model,
            EncodeOptions {
                add_bos: true,
                add_eos: true,
            },
        );
        let tokens = processor.encode("Hello").expect("tokens");
        assert_eq!(tokens.first(), processor.bos_id);
        assert_eq!(tokens.last(), processor.eos_id);
    }

    #[test]
    fn falls_back_to_unknown() {
        let model = build_test_model();
        let processor = SentencePieceProcessor::new(model);
        let tokens = processor.encode("Zzz").expect("tokens");
        assert!(tokens.iter().all(|&id| id == processor.unk_id()));
        assert!(tokens.len() > 0);
    }

    #[test]
    fn gemma_model_round_trip() {
        let path = Path::new("../gemini-data/tokenizer.model");
        let processor = SentencePieceProcessor::from_file(path).expect("load gemma model");
        let text = "Hello world!";
        let tokens = processor.encode(text).expect("encode");
        assert!(!tokens.is_empty());

        let normalized = processor.normalize(text);
        let mut total_chars = 0;
        for &id in &tokens {
            if id == processor.unk_id() {
                total_chars += 1;
            } else {
                let piece = processor.model().piece_chars(id).unwrap();
                total_chars += piece.len();
            }
        }
        assert_eq!(total_chars, normalized.len());
    }

    #[test]
    fn gemma_self_test_samples_match() {
        let path = Path::new(GEMMA_MODEL);
        let processor = SentencePieceProcessor::from_file(path).expect("load gemma model");
        assert_self_test_samples(&processor);
    }

    #[test]
    fn prefers_longer_piece_on_tie() {
        let model = build_tie_test_model();
        let processor = SentencePieceProcessor::new(model);
        let tokens = processor.encode("Hello").expect("tokens");
        let pieces = processor.tokens_to_pieces(&tokens);
        assert_eq!(pieces, vec!["▁Hello"]);
    }

    #[test]
    fn applies_unknown_penalty() {
        let model = build_minimal_model();
        let processor = SentencePieceProcessor::new(model);
        let input = "XYZ";
        let tokens = processor.encode(input).expect("tokens");
        assert_eq!(tokens.len(), input.chars().count());
        assert!(tokens.iter().all(|&id| id == processor.unk_id()));
    }

    fn assert_self_test_samples(processor: &SentencePieceProcessor) {
        let self_test = processor
            .model()
            .self_test_data()
            .expect("model missing self-test data");
        assert!(!self_test.samples.is_empty(), "no self-test samples present");

        for sample in &self_test.samples {
            let input = sample.input.as_deref().unwrap_or("");
            let expected = sample.expected.as_deref().unwrap_or("");
            let tokens = processor
                .encode_with(input, EncodeOptions::default())
