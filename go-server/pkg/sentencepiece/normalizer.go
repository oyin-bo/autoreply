//go:build experimental_sentencepiece

package sentencepiece

import (
	"bufio"
	"errors"
	"strconv"
	"strings"
	"unicode"
	"unicode/utf8"

	"golang.org/x/text/unicode/norm"

	spb "github.com/oyin-bo/autoreply/go-server/pkg/sentencepiece/proto"
)

const (
	spaceRune             = ' '
	escapedWhitespaceRune = '\u2581'
)

type normalizer struct {
	addDummyPrefix         bool
	removeExtraWhitespaces bool
	escapeWhitespaces      bool
	treatWhitespaceSuffix  bool
	rules                  *normalizationTrie
	scratch                []rune
}

func newNormalizer(spec *spb.NormalizerSpec, trainer *spb.TrainerSpec) *normalizer {
	if spec == nil {
		spec = &spb.NormalizerSpec{}
	}
	if trainer == nil {
		trainer = &spb.TrainerSpec{}
	}

	addDummy := spec.GetAddDummyPrefix()
	suffix := trainer.GetTreatWhitespaceAsSuffix()

	return &normalizer{
		addDummyPrefix:         addDummy && !suffix,
		removeExtraWhitespaces: spec.GetRemoveExtraWhitespaces(),
		escapeWhitespaces:      spec.GetEscapeWhitespaces(),
		treatWhitespaceSuffix:  addDummy && suffix,
		rules:                  buildNormalizationTrie(spec),
	}
}

func (n *normalizer) normalize(input string, buf []rune) []rune {
	if n == nil {
		return buf[:0]
	}
	buf = buf[:0]

	n.scratch = n.scratch[:0]
	for idx := 0; idx < len(input); {
		if n.rules != nil {
			if replacement, consumed, ok := n.rules.longestMatch(input[idx:]); ok {
				n.scratch = append(n.scratch, replacement...)
				idx += consumed
				continue
			}
		}

		r, size := utf8.DecodeRuneInString(input[idx:])
		if r == utf8.RuneError && size == 1 {
			r = unicode.ReplacementChar
			size = 1
		}
		if size <= 0 {
			size = 1
		}
		n.scratch = append(n.scratch, r)
		idx += size
	}

	normalized := norm.NFKC.String(string(n.scratch))

	if n.addDummyPrefix {
		buf = append(buf, n.spaceOutput())
	}

	var (
		prevWasSpace bool
		seenContent  bool
	)

	for _, r := range normalized {
		if unicode.IsControl(r) && !unicode.IsSpace(r) {
			continue
		}

		if unicode.IsSpace(r) {
			if n.removeExtraWhitespaces {
				if !seenContent {
					continue
				}
				if prevWasSpace {
					continue
				}
				prevWasSpace = true
			} else {
				prevWasSpace = false
			}
			buf = appendSpace(buf, n.escapeWhitespaces)
			continue
		}

		seenContent = true
		prevWasSpace = false
		buf = append(buf, r)
	}

	if n.removeExtraWhitespaces {
		trimStart := 0
		if n.addDummyPrefix {
			trimStart = 1
		}
		for len(buf) > trimStart && isSpaceOutput(buf[len(buf)-1]) {
			buf = buf[:len(buf)-1]
		}
	}

	if n.treatWhitespaceSuffix {
		buf = append(buf, n.spaceOutput())
	}

	return buf
}

func (n *normalizer) spaceOutput() rune {
	if n.escapeWhitespaces {
		return escapedWhitespaceRune
	}
	return spaceRune
}

func appendSpace(buf []rune, escape bool) []rune {
	if escape {
		return append(buf, escapedWhitespaceRune)
	}
	return append(buf, spaceRune)
}

type normalizationTrie struct {
	children    map[rune]*normalizationTrie
	replacement []rune
}

func buildNormalizationTrie(spec *spb.NormalizerSpec) *normalizationTrie {
	if spec == nil {
		return nil
	}

	trie := &normalizationTrie{children: make(map[rune]*normalizationTrie)}

	if tsv := spec.GetNormalizationRuleTsv(); tsv != "" {
		scanner := bufio.NewScanner(strings.NewReader(tsv))
		for scanner.Scan() {
			line := strings.TrimSpace(scanner.Text())
			if line == "" || strings.HasPrefix(line, "#") {
				continue
			}
			parts := strings.SplitN(line, "\t", 2)
			if len(parts) != 2 {
				continue
			}
			src := parseHexSequence(parts[0])
			dst := parseHexSequence(parts[1])
			if len(src) == 0 {
				continue
			}
			trie.insert(src, dst)
		}
	}

	if len(trie.children) == 0 {
		return nil
	}
	return trie
}

func (t *normalizationTrie) insert(pattern []rune, replacement []rune) {
	node := t
	for _, r := range pattern {
		child, ok := node.children[r]
		if !ok {
			child = &normalizationTrie{children: make(map[rune]*normalizationTrie)}
			node.children[r] = child
		}
		node = child
	}
	node.replacement = append([]rune(nil), replacement...)
}

func (t *normalizationTrie) longestMatch(input string) ([]rune, int, bool) {
	node := t
	var (
		best         []rune
		bestConsumed int
	)

	for consumed := 0; consumed < len(input); {
		r, size := utf8.DecodeRuneInString(input[consumed:])
		if size <= 0 {
			break
		}
		child, ok := node.children[r]
		if !ok {
			break
		}
		consumed += size
		node = child
		if node.replacement != nil {
			best = node.replacement
			bestConsumed = consumed
		}
	}

	if bestConsumed == 0 {
		return nil, 0, false
	}
	return best, bestConsumed, true
}

func parseHexSequence(field string) []rune {
	tokens := strings.Fields(field)
	result := make([]rune, 0, len(tokens))
	for _, token := range tokens {
		if strings.HasPrefix(token, "#") {
			break
		}
		r, err := parseHexToken(token)
		if err != nil {
			continue
		}
		result = append(result, r)
	}
	return result
}

func parseHexToken(token string) (rune, error) {
	if token == "" {
		return 0, errInvalidHex
	}
	value, err := strconv.ParseUint(token, 16, 32)
	if err != nil {
		return 0, err
	}
	if value > utf8.MaxRune {
		return 0, errInvalidHex
	}
	return rune(value), nil
}

var errInvalidHex = errors.New("sentencepiece: invalid hex token")

func isSpaceOutput(r rune) bool {
	return r == spaceRune || r == escapedWhitespaceRune
}
