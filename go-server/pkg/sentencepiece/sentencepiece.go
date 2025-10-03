package sentencepiece

import (
    "context"
    "errors"
    "fmt"
    "os"
    "sync"

    spb "github.com/oyin-bo/autoreply/go-server/pkg/sentencepiece/proto"
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
    model      *spb.ModelProto
    trainer    *spb.TrainerSpec
    normalizer *normalizer
    trie       *doubleArrayTrie
    pieceIndex map[string]int32
    idToPiece  []string

    tokensPool sync.Pool
    piecesPool sync.Pool
    runePool   sync.Pool

    unkID       int32
    unkPiece    string
    byteFallback bool
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

func newProcessorFromModelProto(mp *spb.ModelProto, opts ...Option) (*Processor, error) {
    if mp == nil {
        return nil, ErrModelInvalid
    }

    cfg := ProcessorConfig{AllowFallback: true}
    for _, opt := range opts {
        if opt != nil {
            opt(&cfg)
        }
    }

    pieces := mp.GetPieces()

    trie, err := buildTrie(pieces)
    if err != nil {
        return nil, err
    }

    pieceIndex := make(map[string]int32, len(pieces))
    idToPiece := make([]string, len(pieces))
    for i, p := range pieces {
        piece := p.GetPiece()
        pieceIndex[piece] = int32(i)
        idToPiece[i] = piece
    }

    trainer := mp.GetTrainerSpec()
    if trainer == nil {
        trainer = &spb.TrainerSpec{}
    }

    proc := &Processor{
        cfg:         cfg,
        model:       mp,
        trainer:     trainer,
        normalizer:  newNormalizer(mp.GetNormalizerSpec(), trainer),
        trie:        trie,
        pieceIndex:  pieceIndex,
        idToPiece:   idToPiece,
        tokensPool:  sync.Pool{New: func() any { return make([]int32, 0, 64) }},
        piecesPool:  sync.Pool{New: func() any { return make([]string, 0, 64) }},
        runePool:    sync.Pool{New: func() any { return make([]rune, 0, 128) }},
        unkID:       trainer.GetUnkId(),
        unkPiece:    trainer.GetUnkPiece(),
        byteFallback: trainer.GetByteFallback(),
    }

    return proc, nil
}

// Encode returns token ids for the provided input string.
func (p *Processor) Encode(ctx context.Context, input string) ([]int32, error) {
    tokens, _, err := p.tokenize(ctx, input, false)
    if err != nil {
        return nil, err
    }
    return tokens, nil
}

// EncodePieces mirrors Encode but returns the surface pieces instead of ids.
func (p *Processor) EncodePieces(ctx context.Context, input string) ([]string, error) {
    _, pieces, err := p.tokenize(ctx, input, true)
    if err != nil {
        return nil, err
    }
    return pieces, nil
}

func (p *Processor) tokenize(ctx context.Context, input string, wantPieces bool) ([]int32, []string, error) {
    if p == nil {
        return nil, nil, ErrModelInvalid
    }
    if err := ctx.Err(); err != nil {
        return nil, nil, err
    }

    runeBuf := p.runePool.Get().([]rune)
    runeBuf = runeBuf[:0]
    runes := p.normalizer.normalize(input, runeBuf)

    tokensTmp := p.tokensPool.Get().([]int32)
    tokensTmp = tokensTmp[:0]

    var piecesTmp []string
    if wantPieces {
        piecesTmp = p.piecesPool.Get().([]string)
        piecesTmp = piecesTmp[:0]
    }

    for pos := 0; pos < len(runes); {
        if err := ctx.Err(); err != nil {
            p.tokensPool.Put(tokensTmp[:0])
            if wantPieces {
                p.piecesPool.Put(piecesTmp[:0])
            }
            p.runePool.Put(runes[:0])
            return nil, nil, err
        }

        id, span := p.trie.longestMatch(runes, pos)
        if span == 0 {
            if !p.cfg.AllowFallback {
                p.tokensPool.Put(tokensTmp[:0])
                if wantPieces {
                    p.piecesPool.Put(piecesTmp[:0])
                }
                p.runePool.Put(runes[:0])
                return nil, nil, fmt.Errorf("sentencepiece: no match at position %d", pos)
            }
            tokensTmp = append(tokensTmp, p.unkID)
            if wantPieces {
                piecesTmp = append(piecesTmp, p.unkPiece)
            }
            pos++
            continue
        }

        tokensTmp = append(tokensTmp, id)
        if wantPieces {
            piecesTmp = append(piecesTmp, p.idToPiece[int(id)])
        }
        pos += span

        if p.cfg.TokenLimit > 0 && len(tokensTmp) > p.cfg.TokenLimit {
            p.tokensPool.Put(tokensTmp[:0])
            if wantPieces {
                p.piecesPool.Put(piecesTmp[:0])
            }
            p.runePool.Put(runes[:0])
            return nil, nil, ErrEncodeOverflow
        }
    }

    outTokens := make([]int32, len(tokensTmp))
    copy(outTokens, tokensTmp)
    p.tokensPool.Put(tokensTmp[:0])

    var outPieces []string
    if wantPieces {
        outPieces = make([]string, len(piecesTmp))
        copy(outPieces, piecesTmp)
        p.piecesPool.Put(piecesTmp[:0])
    }

    p.runePool.Put(runes[:0])
    return outTokens, outPieces, nil
}

