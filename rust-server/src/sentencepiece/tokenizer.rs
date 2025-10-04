use std::{cell::RefCell, path::Path};

use thiserror::Error;

use super::{
    loader::{SentencePieceError, SentencePieceModel, SentencePieceType, VocabularyPiece},
    normalizer::{NormalizedString, Normalizer},
    trie::{LookupScratch, VocabularyTrie},
};

#[derive(Debug, Clone, Copy)]
pub struct EncodeOptions {
    pub add_bos: bool,
    pub add_eos: bool,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            add_bos: false,
            add_eos: false,
        }
    }
}

#[derive(Debug, Error)]
pub enum TokenizerError {
    #[error("sentencepiece model error: {0}")]
    Model(#[from] SentencePieceError),
    #[error("failed to tokenize input")]
    DecodeFailed,
}

#[derive(Debug, Clone)]
pub struct SentencePieceProcessor {
    model: SentencePieceModel,
    normalizer: Normalizer,
    trie: VocabularyTrie,
    lookup_scratch: RefCell<LookupScratch>,
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
        let trie = VocabularyTrie::from_pieces(
            model
                .vocab()
                .iter()
                .filter_map(|piece| match piece.kind {
                    SentencePieceType::Unused | SentencePieceType::Control => None,
                    _ => Some((&piece.chars[..], piece.id)),
                }),
        );

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
        self.model.piece(id).map(|p| p.piece.as_str())
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
        let normalized = self.normalize(text);
        let mut scratch = self.lookup_scratch.borrow_mut();
        encode_normalized(
            &self.trie,
            &self.model,
            &normalized,
            self.unk_id,
            self.unk_score,
            options,
            self.bos_id,
            self.eos_id,
            &mut scratch,
        )
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
    scratch: &mut LookupScratch,
) -> Result<Vec<u32>, TokenizerError> {
    let chars = normalized.chars();
    let len = chars.len();

    let mut best_scores = vec![f32::NEG_INFINITY; len + 1];
    let mut back_ptrs: Vec<Option<(usize, u32)>> = vec![None; len + 1];
    best_scores[0] = 0.0;

    for pos in 0..len {
        if best_scores[pos].is_infinite() {
            continue;
        }

        let mut matched = false;
        trie.common_prefix_search(&chars[pos..], scratch, |match_len, piece_id| {
            if let Some(piece) = model.piece(piece_id) {
                let end = pos + match_len;
                let score = best_scores[pos] + piece.score;
                if score > best_scores[end] {
                    best_scores[end] = score;
                    back_ptrs[end] = Some((pos, piece_id));
                }
                matched = true;
            }
        });

        if !matched {
            let end = pos + 1;
            let score = best_scores[pos] + unk_score;
            if score > best_scores[end] {
                best_scores[end] = score;
                back_ptrs[end] = Some((pos, unk_id));
            }
        }
    }

    if best_scores[len].is_infinite() {
        return Err(TokenizerError::DecodeFailed);
    }

    let mut tokens = Vec::new();
    let mut pos = len;
    while pos > 0 {
        let (prev, piece_id) = back_ptrs[pos].ok_or(TokenizerError::DecodeFailed)?;
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

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentencepiece::proto::{self, ModelProto};
    use std::path::Path;

    const GEMMA_MODEL: &str = "../gemini-data/tokenizer.model";

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
                let piece = processor.model().piece(id).unwrap();
                total_chars += piece.chars.len();
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
                .unwrap_or_else(|_| panic!("failed to encode sample: {input}"));
            let pieces = processor.tokens_to_pieces(&tokens);
            let actual = pieces.join(" ");
            assert_eq!(actual, expected, "input: {input}");
        }
    }
}
