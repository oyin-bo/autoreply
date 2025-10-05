//go:build !experimental_sentencepiece

// Package sentencepiece provides a stub implementation when the experimental_sentencepiece
// build tag is not enabled. This ensures the package compiles without the full implementation.
//
// To enable the real SentencePiece tokenizer, build with:
//
//	go build -tags=experimental_sentencepiece
package sentencepiece

import (
	"context"
	"errors"
)

// ErrNotEnabled is returned when SentencePiece functionality is called but not enabled.
var ErrNotEnabled = errors.New("sentencepiece: not enabled - rebuild with -tags=experimental_sentencepiece")

// Option configures a Processor (stub).
type Option func(*ProcessorConfig)

// WithTokenLimit bounds the number of tokens returned by Encode (stub).
func WithTokenLimit(limit int) Option {
	return func(cfg *ProcessorConfig) {
		cfg.TokenLimit = limit
	}
}

// ProcessorConfig captures optional runtime behaviour tweaks (stub).
type ProcessorConfig struct {
	TokenLimit         int
	AllowFallback      bool
	EnableByteFallback bool
}

// Processor performs SentencePiece tokenisation (stub - always returns errors).
type Processor struct {
	cfg ProcessorConfig
}

// LoadProcessor returns an error indicating SentencePiece is not enabled.
func LoadProcessor(modelPath string, opts ...Option) (*Processor, error) {
	return nil, ErrNotEnabled
}

// NewProcessorFromModel returns an error indicating SentencePiece is not enabled.
func NewProcessorFromModel(data []byte, opts ...Option) (*Processor, error) {
	return nil, ErrNotEnabled
}

// Encode returns an error indicating SentencePiece is not enabled.
func (p *Processor) Encode(ctx context.Context, input string) ([]int32, error) {
	return nil, ErrNotEnabled
}

// EncodePieces returns an error indicating SentencePiece is not enabled.
func (p *Processor) EncodePieces(ctx context.Context, input string) ([]string, error) {
	return nil, ErrNotEnabled
}
