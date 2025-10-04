package sentencepiece

import (
	"fmt"
	"sort"

	spb "github.com/oyin-bo/autoreply/go-server/pkg/sentencepiece/proto"
)

type doubleArrayTrie struct {
	base  []int32
	check []int32
	value []int32

	codes map[rune]int32
	root  int32
}

func buildTrie(pieces []*spb.ModelProto_SentencePiece) (*doubleArrayTrie, error) {
	if len(pieces) == 0 {
		return nil, fmt.Errorf("sentencepiece: build trie: no pieces provided")
	}

	b := newTrieBuilder()
	root := b.newNode()
	root.index = b.rootIndex

	b.ensure(root.index)
	b.check[root.index] = 0

	if err := b.insertPieces(root, pieces); err != nil {
		return nil, err
	}

	if root.terminal {
		b.value[root.index] = root.id
	}

	if err := b.assign(root); err != nil {
		return nil, err
	}

	b.trim()

	return &doubleArrayTrie{
		base:  b.base,
		check: b.check,
		value: b.value,
		codes: b.codes,
		root:  b.rootIndex,
	}, nil
}

func (t *doubleArrayTrie) longestMatch(runes []rune, pos int) (int32, int) {
	if t == nil || pos >= len(runes) {
		return -1, 0
	}

	var (
		bestID   int32 = -1
		bestSpan int
	)

	t.matchesAt(runes, pos, func(id int32, span int) {
		if span > bestSpan {
			bestID = id
			bestSpan = span
		}
	})

	if bestID < 0 {
		return -1, 0
	}

	return bestID, bestSpan
}

func (t *doubleArrayTrie) matchesAt(runes []rune, pos int, fn func(id int32, span int)) {
	if t == nil || fn == nil || pos >= len(runes) {
		return
	}

	index := t.root
	if int(index) >= len(t.base) {
		return
	}
	base := t.base[index]

	for i := pos; i < len(runes); i++ {
		code, ok := t.codes[runes[i]]
		if !ok {
			break
		}

		next := base + code
		if next <= 0 || int(next) >= len(t.check) || t.check[next] != index {
			break
		}

		if val := t.value[next]; val >= 0 {
			fn(val, i-pos+1)
		}

		index = next
		if int(index) >= len(t.base) {
			break
		}
		base = t.base[index]
	}
}

type builderNode struct {
	children map[int32]*builderNode
	index    int32
	id       int32
	terminal bool
}

type trieBuilder struct {
	codes     map[rune]int32
	nextCode  int32
	base      []int32
	check     []int32
	value     []int32
	rootIndex int32
}

func newTrieBuilder() *trieBuilder {
	b := &trieBuilder{
		codes:     make(map[rune]int32),
		nextCode:  1,
		base:      make([]int32, 2),
		check:     make([]int32, 2),
		value:     make([]int32, 2),
		rootIndex: 1,
	}

	for i := range b.check {
		b.check[i] = -1
		b.value[i] = -1
	}

	return b
}

func (b *trieBuilder) newNode() *builderNode {
	return &builderNode{
		children: make(map[int32]*builderNode),
		id:       -1,
	}
}

func (b *trieBuilder) insertPieces(root *builderNode, pieces []*spb.ModelProto_SentencePiece) error {
	for idx, pieceProto := range pieces {
		if pieceProto == nil {
			continue
		}

		piece := pieceProto.GetPiece()
		node := root

		for _, r := range piece {
			code := b.codeForRune(r)
			child, ok := node.children[code]
			if !ok {
				child = b.newNode()
				node.children[code] = child
			}
			node = child
		}

		if node.terminal {
			return fmt.Errorf("sentencepiece: duplicate piece %q", piece)
		}

		node.terminal = true
		node.id = int32(idx)
	}

	return nil
}

func (b *trieBuilder) assign(node *builderNode) error {
	if len(node.children) == 0 {
		return nil
	}

	codes := make([]int32, 0, len(node.children))
	for code := range node.children {
		codes = append(codes, code)
	}
	sort.Slice(codes, func(i, j int) bool { return codes[i] < codes[j] })

	base := b.findBase(codes)
	b.base[node.index] = base

	for _, code := range codes {
		child := node.children[code]
		childIndex := base + code
		b.ensure(childIndex)
		b.check[childIndex] = node.index
		child.index = childIndex
		if child.terminal {
			b.value[childIndex] = child.id
		}
	}

	for _, code := range codes {
		child := node.children[code]
		if err := b.assign(child); err != nil {
			return err
		}
	}

	return nil
}

func (b *trieBuilder) findBase(codes []int32) int32 {
	if len(codes) == 0 {
		return 0
	}

	base := int32(1)
	for {
		conflict := false
		for _, code := range codes {
			idx := base + code
			b.ensure(idx)
			if b.check[idx] != -1 {
				conflict = true
				break
			}
		}

		if !conflict {
			return base
		}

		base++
	}
}

func (b *trieBuilder) ensure(idx int32) {
	if int(idx) < len(b.base) {
		return
	}

	oldLen := len(b.base)
	newLen := oldLen
	if newLen == 0 {
		newLen = 2
	}
	for int(idx) >= newLen {
		newLen *= 2
	}

	base := make([]int32, newLen)
	copy(base, b.base)
	check := make([]int32, newLen)
	copy(check, b.check)
	value := make([]int32, newLen)
	copy(value, b.value)

	for i := oldLen; i < newLen; i++ {
		check[i] = -1
		value[i] = -1
	}

	b.base = base
	b.check = check
	b.value = value
}

func (b *trieBuilder) trim() {
	last := len(b.base) - 1
	for last > int(b.rootIndex) && b.check[last] == -1 && b.value[last] == -1 && b.base[last] == 0 {
		last--
	}

	b.base = b.base[:last+1]
	b.check = b.check[:last+1]
	b.value = b.value[:last+1]
}

func (b *trieBuilder) codeForRune(r rune) int32 {
	if code, ok := b.codes[r]; ok {
		return code
	}

	code := b.nextCode
	b.codes[r] = code
	b.nextCode++
	return code
}
