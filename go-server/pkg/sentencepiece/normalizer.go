package sentencepiece

import (
	spb "github.com/oyin-bo/autoreply/go-server/pkg/sentencepiece/proto"
)

type normalizer struct {
	spec    *spb.NormalizerSpec
	trainer *spb.TrainerSpec
}

func newNormalizer(spec *spb.NormalizerSpec, trainer *spb.TrainerSpec) *normalizer {
	if spec == nil {
		spec = &spb.NormalizerSpec{}
	}
	if trainer == nil {
		trainer = &spb.TrainerSpec{}
	}
	return &normalizer{
		spec:    spec,
		trainer: trainer,
	}
}

func (n *normalizer) normalize(input string, buf []rune) []rune {
	buf = buf[:0]
	for _, r := range input {
		buf = append(buf, r)
	}
	return buf
}
