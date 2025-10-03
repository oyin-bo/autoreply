use std::collections::HashMap;
use std::path::Path;
use std::{fs, io};

use thiserror::Error;

use super::proto::{self, model_proto, ModelProto};

#[derive(Debug, Error)]
pub enum SentencePieceError {
    #[error("failed to read model file: {0}")]
    Io(#[from] io::Error),
    #[error("failed to decode model protobuf: {0}")]
    Decode(#[from] prost::DecodeError),
    #[error("model missing trainer spec")]
    MissingTrainerSpec,
    #[error("model missing normalizer spec")]
    MissingNormalizerSpec,
    #[error("model has empty vocabulary")]
    EmptyVocabulary,
    #[error("piece '{0}' is empty")]
    EmptyPiece(String),
}

#[derive(Debug, Clone)]
pub struct VocabularyPiece {
    pub id: u32,
    pub piece: String,
    pub chars: Vec<char>,
    pub score: f32,
    pub kind: SentencePieceType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SentencePieceType {
    Normal,
    Unknown,
    Control,
    UserDefined,
    Byte,
    Unused,
}

impl From<model_proto::sentence_piece::Type> for SentencePieceType {
    fn from(value: model_proto::sentence_piece::Type) -> Self {
        use model_proto::sentence_piece::Type;
        match value {
            Type::Normal => SentencePieceType::Normal,
            Type::Unknown => SentencePieceType::Unknown,
            Type::Control => SentencePieceType::Control,
            Type::UserDefined => SentencePieceType::UserDefined,
            Type::Byte => SentencePieceType::Byte,
            Type::Unused => SentencePieceType::Unused,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SentencePieceModel {
    pub proto: ModelProto,
    pub vocab: Vec<VocabularyPiece>,
    pub piece_index: HashMap<String, u32>,
    pub unk_id: u32,
    pub bos_id: Option<u32>,
    pub eos_id: Option<u32>,
    pub pad_id: Option<u32>,
}

impl SentencePieceModel {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, SentencePieceError> {
        let bytes = fs::read(path)?;
        Self::load_from_bytes(&bytes)
    }

    pub fn load_from_bytes(bytes: &[u8]) -> Result<Self, SentencePieceError> {
        let proto = ModelProto::decode(bytes)?;
        Self::from_proto(proto)
    }

    pub fn from_proto(proto: ModelProto) -> Result<Self, SentencePieceError> {
        if proto.trainer_spec.is_none() {
            return Err(SentencePieceError::MissingTrainerSpec);
        }
        if proto.normalizer_spec.is_none() {
            return Err(SentencePieceError::MissingNormalizerSpec);
        }

        let vocab = build_vocab(&proto)?;
        let piece_index = build_piece_index(&vocab);
        let trainer = proto.trainer_spec.as_ref().unwrap();

        let unk_id = trainer.unk_id as u32;
        let bos_id = option_id(trainer.bos_id);
        let eos_id = option_id(trainer.eos_id);
        let pad_id = option_id(trainer.pad_id);

        Ok(Self {
            proto,
            vocab,
            piece_index,
            unk_id,
            bos_id,
            eos_id,
            pad_id,
        })
    }

    pub fn vocab(&self) -> &[VocabularyPiece] {
        &self.vocab
    }

    pub fn piece(&self, id: u32) -> Option<&VocabularyPiece> {
        self.vocab.get(id as usize)
    }

    pub fn trainer_spec(&self) -> &proto::TrainerSpec {
        self.proto
            .trainer_spec
            .as_ref()
            .expect("trainer_spec validated during construction")
    }

    pub fn normalizer_spec(&self) -> &proto::NormalizerSpec {
        self.proto
            .normalizer_spec
            .as_ref()
            .expect("normalizer_spec validated during construction")
    }
}

fn build_vocab(proto: &ModelProto) -> Result<Vec<VocabularyPiece>, SentencePieceError> {
    if proto.pieces.is_empty() {
        return Err(SentencePieceError::EmptyVocabulary);
    }

    let mut vocab = Vec::with_capacity(proto.pieces.len());
    for (idx, piece) in proto.pieces.iter().enumerate() {
        let piece_text = piece.piece.clone().unwrap_or_default();
        if piece_text.is_empty() {
            return Err(SentencePieceError::EmptyPiece(format!("id {}", idx)));
        }
        let chars: Vec<char> = piece_text.chars().collect();
        vocab.push(VocabularyPiece {
            id: idx as u32,
            piece: piece_text,
            chars,
            score: piece.score.unwrap_or(0.0),
            kind: piece_kind(piece),
        });
    }

    Ok(vocab)
}

fn piece_kind(piece: &model_proto::SentencePiece) -> SentencePieceType {
    piece
        .r#type
        .and_then(model_proto::sentence_piece::Type::from_i32)
        .map(SentencePieceType::from)
        .unwrap_or(SentencePieceType::Normal)
}

fn build_piece_index(vocab: &[VocabularyPiece]) -> HashMap<String, u32> {
    let mut index = HashMap::with_capacity(vocab.len());
    for item in vocab {
        index.insert(item.piece.clone(), item.id);
    }
    index
}

fn option_id(raw: Option<i32>) -> Option<u32> {
    raw.and_then(|id| if id >= 0 { Some(id as u32) } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proto::{NormalizerSpec, TrainerSpec};

    fn dummy_proto() -> ModelProto {
        ModelProto {
            pieces: vec![
                model_proto::SentencePiece {
                    piece: Some("<unk>".to_string()),
                    score: Some(0.0),
                    r#type: Some(model_proto::sentence_piece::Type::Unknown as i32),
                    ..Default::default()
                },
                model_proto::SentencePiece {
                    piece: Some("hello".to_string()),
                    score: Some(-1.0),
                    r#type: Some(model_proto::sentence_piece::Type::Normal as i32),
                    ..Default::default()
                },
            ],
            trainer_spec: Some(TrainerSpec {
                unk_id: Some(0),
                bos_id: Some(1),
                eos_id: Some(2),
                pad_id: Some(-1),
                ..Default::default()
            }),
            normalizer_spec: Some(NormalizerSpec::default()),
            ..Default::default()
        }
    }

    #[test]
    fn builds_vocab_index_and_special_ids() {
        let proto = dummy_proto();
        let model = SentencePieceModel::from_proto(proto).expect("model");
        assert_eq!(model.vocab.len(), 2);
        assert_eq!(model.unk_id, 0);
        assert_eq!(model.bos_id, Some(1));
        assert_eq!(model.eos_id, Some(2));
        assert_eq!(model.pad_id, None);
        assert_eq!(model.piece_index.get("hello"), Some(&1));
    }

    #[test]
    fn rejects_missing_trainer_spec() {
        let mut proto = dummy_proto();
        proto.trainer_spec = None;
        let err = SentencePieceModel::from_proto(proto).unwrap_err();
        assert!(matches!(err, SentencePieceError::MissingTrainerSpec));
    }

    #[test]
    fn rejects_missing_normalizer_spec() {
        let mut proto = dummy_proto();
        proto.normalizer_spec = None;
        let err = SentencePieceModel::from_proto(proto).unwrap_err();
        assert!(matches!(err, SentencePieceError::MissingNormalizerSpec));
    }

    #[test]
    fn rejects_empty_vocab() {
        let mut proto = dummy_proto();
        proto.pieces.clear();
        let err = SentencePieceModel::from_proto(proto).unwrap_err();
        assert!(matches!(err, SentencePieceError::EmptyVocabulary));
    }

    #[test]
    fn rejects_empty_piece_text() {
        let mut proto = dummy_proto();
        proto.pieces[1].piece = Some(String::new());
        let err = SentencePieceModel::from_proto(proto).unwrap_err();
        assert!(matches!(err, SentencePieceError::EmptyPiece(_)));
    }
}
