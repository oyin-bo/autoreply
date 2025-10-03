package sentencepiece

import (
    "context"
    "errors"
    "fmt"
    "os"
    "sync"
)

// ErrModelInvalid is returned when the provided SentencePiece model cannot be parsed.
var ErrModelInvalid = errors.New("sentencepiece: invalid model")

// ErrEncodeOverflow is returned when Encode would exceed the configured token limit.
var ErrEncodeOverflow = errors.New("sentencepiece: encode overflow")

// Option configures a Processor.
type Option func(*ProcessorConfig)

// WithTokenLimit bounds the number of tokens returned by Encode.
func WithTokenLimit(limit int) Option {
    return func(cfg *ProcessorConfig) {
        cfg.TokenLimit = limit
    }
}

// ProcessorConfig captures optional runtime behaviour tweaks.
type ProcessorConfig struct {
    TokenLimit int
    // AllowFallback controls whether unknown spans are emitted as the configured UNK token.
    AllowFallback bool
}

// Processor performs SentencePiece tokenisation using a loaded model.
type Processor struct {
    cfg        ProcessorConfig
    model      *ModelProto
    normalizer normalizer
    trie       *doubleArrayTrie
    pieceIndex map[string]int32
    idToPiece  []Piece

    tokensPool sync.Pool
    piecesPool sync.Pool
}

// LoadProcessor reads a serialized SentencePiece model from disk and constructs a Processor.
func LoadProcessor(modelPath string, opts ...Option) (*Processor, error) {
    data, err := os.ReadFile(modelPath)
    if err != nil {
        return nil, fmt.Errorf("sentencepiece: read model: %w", err)
    }
    return NewProcessorFromModel(data, opts...)
}

// NewProcessorFromModel constructs a Processor from the given serialized model bytes.
func NewProcessorFromModel(data []byte, opts ...Option) (*Processor, error) {
    mp, err := parseModel(data)
    if err != nil {
        return nil, err
    }
    return newProcessorFromModelProto(mp, opts...)
}

func newProcessorFromModelProto(mp *ModelProto, opts ...Option) (*Processor, error) {
    if mp == nil {
        return nil, ErrModelInvalid
    }

    cfg := ProcessorConfig{AllowFallback: true}
    for _, opt := range opts {
        if opt != nil {
            opt(&cfg)
        }
    }

    trie, err := buildTrie(mp.Pieces)
    if err != nil {
        return nil, err
    }

    pieceIndex := make(map[string]int32, len(mp.Pieces))
    idToPiece := make([]Piece, len(mp.Pieces))
    for i, p := range mp.Pieces {
        pieceIndex[p.Piece] = int32(i)
        idToPiece[i] = p
    }

    proc := &Processor{
        cfg:        cfg,
        model:      mp,
        normalizer: newNormalizer(mp.NormalizerSpec, mp.TrainerSpec),
        trie:       trie,
        pieceIndex: pieceIndex,
        idToPiece:  idToPiece,
        tokensPool: sync.Pool{New: func() any { return make([]int32, 0, 64) }},
        piecesPool: sync.Pool{New: func() any { return make([]string, 0, 64) }},
    }

    return proc, nil
}

// Encode returns token ids for the provided input string.
func (p *Processor) Encode(ctx context.Context, input string) ([]int32, error) {
    if p == nil {
        return nil, ErrModelInvalid
    }
    if err := ctx.Err(); err != nil {
        return nil, err
    }

    normalized := p.normalizer.normalize(input)
    runes := []rune(normalized)

    tokens := p.tokensPool.Get().([]int32)
    tokens = tokens[:0]

    for pos := 0; pos < len(runes); {
        if err := ctx.Err(); err != nil {
            p.tokensPool.Put(tokens[:0])
            return nil, err
        }

        id, span := p.trie.longestMatch(runes, pos)
        if span == 0 {
            if !p.cfg.AllowFallback {
                p.tokensPool.Put(tokens[:0])
                return nil, fmt.Errorf("sentencepiece: no match at position %d", pos)
            }
            tokens = append(tokens, p.model.TrainerSpec.UnkID)
            pos++
            continue
        }

        tokens = append(tokens, id)
        pos += span

        if p.cfg.TokenLimit > 0 && len(tokens) > p.cfg.TokenLimit {
            p.tokensPool.Put(tokens[:0])
            return nil, ErrEncodeOverflow
        }
    }

    out := make([]int32, len(tokens))
    copy(out, tokens)
    p.tokensPool.Put(tokens[:0])
    return out, nil
}

// EncodePieces mirrors Encode but returns the surface pieces instead of ids.
func (p *Processor) EncodePieces(ctx context.Context, input string) ([]string, error) {
    if p == nil {
        return nil, ErrModelInvalid
    }
    if err := ctx.Err(); err != nil {
        return nil, err
    }

    normalized := p.normalizer.normalize(input)
    runes := []rune(normalized)

    pieces := p.piecesPool.Get().([]string)
    pieces = pieces[:0]

    for pos := 0; pos < len(runes); {
        if err := ctx.Err(); err != nil {
            p.piecesPool.Put(pieces[:0])
            return nil, err
        }

        id, span := p.trie.longestMatch(runes, pos)
        if span == 0 {
            if !p.cfg.AllowFallback {
                p.piecesPool.Put(pieces[:0])
                return nil, fmt.Errorf("sentencepiece: no match at position %d", pos)
            }
            pieces = append(pieces, p.model.TrainerSpec.UnkPiece)
            pos++
            continue
        }

        pieces = append(pieces, p.idToPiece[id].Piece)
        pos += span
    }

    out := make([]string, len(pieces))
    copy(out, pieces)
    p.piecesPool.Put(pieces[:0])
    return out, nil
}

