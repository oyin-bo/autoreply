package sentencepiece

import (
	"context"
	"errors"
	"fmt"
	"math"
	"os"
	"strconv"
	"sync"
	"unicode/utf8"

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
	// EnableByteFallback forces byte-piece emission when the trie has no match.
	EnableByteFallback bool
}

// Processor performs SentencePiece tokenisation using a loaded model.
type Processor struct {
	cfg         ProcessorConfig
	model       *spb.ModelProto
	trainer     *spb.TrainerSpec
	normalizer  *normalizer
	trie        *doubleArrayTrie
	pieceIndex  map[string]int32
	idToPiece   []string
	pieceScores []float32
	bytePieces  [256]int32

	tokensPool  sync.Pool
	piecesPool  sync.Pool
	runePool    sync.Pool
	scoresPool  sync.Pool
	backPosPool sync.Pool
	backTokPool sync.Pool
	matchPool   sync.Pool
	fbStartPool sync.Pool
	fbLenPool   sync.Pool
	fbIDsPool   sync.Pool

	unkID        int32
	unkPiece     string
	unkScore     float32
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

	bytePieces := [256]int32{}
	for i := range bytePieces {
		bytePieces[i] = -1
	}

	trie, err := buildTrie(pieces)
	if err != nil {
		return nil, err
	}

	pieceIndex := make(map[string]int32, len(pieces))
	idToPiece := make([]string, len(pieces))
	pieceScores := make([]float32, len(pieces))
	for i, p := range pieces {
		piece := p.GetPiece()
		pieceIndex[piece] = int32(i)
		idToPiece[i] = piece
		pieceScores[i] = p.GetScore()

		if p.GetType() == spb.ModelProto_SentencePiece_BYTE {
			if b, ok := parseBytePiece(piece); ok {
				bytePieces[b] = int32(i)
			}
		}
	}

	trainer := mp.GetTrainerSpec()
	if trainer == nil {
		trainer = &spb.TrainerSpec{}
	}

	cfg.EnableByteFallback = cfg.EnableByteFallback || trainer.GetByteFallback()

	proc := &Processor{
		cfg:          cfg,
		model:        mp,
		trainer:      trainer,
		normalizer:   newNormalizer(mp.GetNormalizerSpec(), trainer),
		trie:         trie,
		pieceIndex:   pieceIndex,
		idToPiece:    idToPiece,
		pieceScores:  pieceScores,
		bytePieces:   bytePieces,
		tokensPool:   sync.Pool{New: func() any { return make([]int32, 0, 64) }},
		piecesPool:   sync.Pool{New: func() any { return make([]string, 0, 64) }},
		runePool:     sync.Pool{New: func() any { return make([]rune, 0, 128) }},
		scoresPool:   sync.Pool{New: func() any { return make([]float32, 0, 128) }},
		backPosPool:  sync.Pool{New: func() any { return make([]int, 0, 128) }},
		backTokPool:  sync.Pool{New: func() any { return make([]int32, 0, 128) }},
		matchPool:    sync.Pool{New: func() any { return make([]trieMatch, 0, 16) }},
		fbStartPool:  sync.Pool{New: func() any { return make([]int, 0, 128) }},
		fbLenPool:    sync.Pool{New: func() any { return make([]int, 0, 128) }},
		fbIDsPool:    sync.Pool{New: func() any { return make([]int32, 0, 128) }},
		unkID:        trainer.GetUnkId(),
		unkPiece:     trainer.GetUnkPiece(),
		unkScore:     pieceScoreSafe(pieceScores, trainer.GetUnkId()),
		byteFallback: trainer.GetByteFallback(),
	}

	return proc, nil
}

const negInf float32 = -math.MaxFloat32

type trieMatch struct {
	id      int32
	span    int
	score   float32
	fbStart int
	fbLen   int
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
	var runes []rune
	if p.normalizer != nil {
		runes = p.normalizer.normalize(input, runeBuf)
	} else {
		runes = append(runeBuf[:0], []rune(input)...)
	}

	n := len(runes)

	scoresBuf := p.scoresPool.Get().([]float32)
	if cap(scoresBuf) < n+1 {
		scoresBuf = make([]float32, n+1)
	}
	scores := scoresBuf[:n+1]
	for i := range scores {
		scores[i] = negInf
	}
	scores[0] = 0

	backPosBuf := p.backPosPool.Get().([]int)
	if cap(backPosBuf) < n+1 {
		backPosBuf = make([]int, n+1)
	}
	backPos := backPosBuf[:n+1]
	for i := range backPos {
		backPos[i] = -1
	}

	backTokBuf := p.backTokPool.Get().([]int32)
	if cap(backTokBuf) < n+1 {
		backTokBuf = make([]int32, n+1)
	}
	backTok := backTokBuf[:n+1]
	for i := range backTok {
		backTok[i] = -1
	}

	matchesBuf := p.matchPool.Get().([]trieMatch)
	matches := matchesBuf[:0]

	fbStartBuf := p.fbStartPool.Get().([]int)
	if cap(fbStartBuf) < n+1 {
		fbStartBuf = make([]int, n+1)
	}
	backFbStart := fbStartBuf[:n+1]
	for i := range backFbStart {
		backFbStart[i] = -1
	}

	fbLenBuf := p.fbLenPool.Get().([]int)
	if cap(fbLenBuf) < n+1 {
		fbLenBuf = make([]int, n+1)
	}
	backFbLen := fbLenBuf[:n+1]

	fbIDsBuf := p.fbIDsPool.Get().([]int32)
	fbIDs := fbIDsBuf[:0]

	unkID := p.unkID
	unkScore := p.unkScore

	for pos := 0; pos < n; pos++ {
		if err := ctx.Err(); err != nil {
			p.releaseRuneBuffer(runes)
			p.releaseDPBuffers(scoresBuf, backPosBuf, backTokBuf, backFbStart, backFbLen, matchesBuf, fbIDs)
			return nil, nil, err
		}

		if scores[pos] == negInf {
			continue
		}

		matches = matches[:0]
		p.trie.matchesAt(runes, pos, func(id int32, span int) {
			matches = append(matches, trieMatch{id: id, span: span})
		})

		if len(matches) == 0 && p.cfg.EnableByteFallback && p.byteFallback {
			matches = p.appendByteFallbackMatch(runes, pos, matches, &fbIDs)
		}

		if len(matches) == 0 {
			if !p.cfg.AllowFallback {
				p.releaseRuneBuffer(runes)
				p.releaseDPBuffers(scoresBuf, backPosBuf, backTokBuf, backFbStart, backFbLen, matchesBuf, fbIDs)
				return nil, nil, fmt.Errorf("sentencepiece: no match at position %d", pos)
			}
			if unkID >= 0 {
				matches = append(matches, trieMatch{id: unkID, span: 1})
			}
		}

		if len(matches) == 0 {
			continue
		}

		for _, m := range matches {
			if m.span <= 0 {
				continue
			}
			next := pos + m.span
			if next > n {
				continue
			}
			newScore := scores[pos]
			if m.fbLen > 0 {
				newScore += m.score
			} else if int(m.id) < len(p.pieceScores) && m.id >= 0 {
				newScore += p.pieceScores[m.id]
			} else if m.id == unkID {
				newScore += unkScore
			}
			if newScore > scores[next] {
				scores[next] = newScore
				backPos[next] = pos
				backTok[next] = m.id
				if m.fbLen > 0 {
					backFbStart[next] = m.fbStart
					backFbLen[next] = m.fbLen
				} else {
					backFbStart[next] = -1
					backFbLen[next] = 0
				}
			}
		}
	}

	if scores[n] == negInf {
		if !p.cfg.AllowFallback || unkID < 0 {
			p.releaseRuneBuffer(runes)
			p.releaseDPBuffers(scoresBuf, backPosBuf, backTokBuf, backFbStart, backFbLen, matchesBuf, fbIDs)
			return nil, nil, fmt.Errorf("sentencepiece: unable to tokenize input")
		}
	}

	tokensTmp := p.tokensPool.Get().([]int32)
	tokensTmp = tokensTmp[:0]

	var piecesTmp []string
	if wantPieces {
		piecesTmp = p.piecesPool.Get().([]string)
		piecesTmp = piecesTmp[:0]
	}

	for pos := n; pos > 0; {
		prev := backPos[pos]
		span := pos - prev

		if fbCount := backFbLen[pos]; fbCount > 0 {
			start := backFbStart[pos]
			end := start + fbCount
			for i := end - 1; i >= start; i-- {
				id := fbIDs[i]
				tokensTmp = append(tokensTmp, id)
				if wantPieces {
					piece := ""
					if idx := int(id); idx >= 0 && idx < len(p.idToPiece) {
						piece = p.idToPiece[idx]
					}
					piecesTmp = append(piecesTmp, piece)
				}
			}
			pos = prev
			continue
		}

		id := backTok[pos]
		if id < 0 || prev < 0 || span <= 0 {
			if !p.cfg.AllowFallback || unkID < 0 {
				p.releaseRuneBuffer(runes)
				p.releaseDPBuffers(scoresBuf, backPosBuf, backTokBuf, backFbStart, backFbLen, matchesBuf, fbIDs)
				p.tokensPool.Put(tokensTmp[:0])
				if wantPieces {
					p.piecesPool.Put(piecesTmp[:0])
				}
				return nil, nil, fmt.Errorf("sentencepiece: invalid backpointer at position %d", pos)
			}
			id = unkID
			prev = pos - 1
		}

		tokensTmp = append(tokensTmp, id)
		if wantPieces {
			piecesTmp = append(piecesTmp, p.idToPiece[int(id)])
		}
		pos = prev
	}

	reverseInt32(tokensTmp)
	if wantPieces {
		reverseString(piecesTmp)
	}

	if p.cfg.TokenLimit > 0 && len(tokensTmp) > p.cfg.TokenLimit {
		p.releaseRuneBuffer(runes)
		p.releaseDPBuffers(scoresBuf, backPosBuf, backTokBuf, backFbStart, backFbLen, matchesBuf, fbIDs)
		p.tokensPool.Put(tokensTmp[:0])
		if wantPieces {
			p.piecesPool.Put(piecesTmp[:0])
		}
		return nil, nil, ErrEncodeOverflow
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

	p.releaseRuneBuffer(runes)
	p.releaseDPBuffers(scoresBuf, backPosBuf, backTokBuf, backFbStart, backFbLen, matchesBuf, fbIDs)
	return outTokens, outPieces, nil
}

func (p *Processor) releaseRuneBuffer(runes []rune) {
	p.runePool.Put(runes[:0])
}

func (p *Processor) releaseDPBuffers(scores []float32, backPos []int, backTok []int32, backFbStart []int, backFbLen []int, matches []trieMatch, fbIDs []int32) {
	p.scoresPool.Put(scores[:0])
	p.backPosPool.Put(backPos[:0])
	p.backTokPool.Put(backTok[:0])
	p.fbStartPool.Put(backFbStart[:0])
	p.fbLenPool.Put(backFbLen[:0])
	p.matchPool.Put(matches[:0])
	p.fbIDsPool.Put(fbIDs[:0])
}

func (p *Processor) appendByteFallbackMatch(runes []rune, pos int, matches []trieMatch, fbIDs *[]int32) []trieMatch {
	if pos >= len(runes) {
		return matches
	}
	var encoded [utf8.UTFMax]byte
	size := utf8.EncodeRune(encoded[:], runes[pos])
	start := len(*fbIDs)
	total := float32(0)
	for i := 0; i < size; i++ {
		id := p.bytePieces[encoded[i]]
		if id < 0 || int(id) >= len(p.pieceScores) {
			*fbIDs = (*fbIDs)[:start]
			return matches
		}
		*fbIDs = append(*fbIDs, id)
		total += p.pieceScores[id]
	}
	if len(*fbIDs) == start {
		return matches
	}
	return append(matches, trieMatch{
		id:      (*fbIDs)[start],
		span:    1,
		score:   total,
		fbStart: start,
		fbLen:   len(*fbIDs) - start,
	})
}

func reverseInt32(s []int32) {
	for i, j := 0, len(s)-1; i < j; i, j = i+1, j-1 {
		s[i], s[j] = s[j], s[i]
	}
}

func reverseString(s []string) {
	for i, j := 0, len(s)-1; i < j; i, j = i+1, j-1 {
		s[i], s[j] = s[j], s[i]
	}
}

func pieceScoreSafe(scores []float32, id int32) float32 {
	if id >= 0 && int(id) < len(scores) {
		return scores[id]
	}
	return 0
}

func parseBytePiece(piece string) (byte, bool) {
	if len(piece) != 6 || piece[0] != '<' || (piece[1] != '0' || (piece[2] != 'x' && piece[2] != 'X')) || piece[5] != '>' {
		return 0, false
	}
	value, err := strconv.ParseUint(piece[3:5], 16, 8)
	if err != nil {
		return 0, false
	}
	return byte(value), true
}
