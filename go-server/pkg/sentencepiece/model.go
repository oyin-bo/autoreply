package sentencepiece

import (
	"fmt"

	"google.golang.org/protobuf/proto"

	spb "github.com/oyin-bo/autoreply/go-server/pkg/sentencepiece/proto"
)

// parseModel decodes a serialized SentencePiece model payload into a ModelProto.
func parseModel(data []byte) (*spb.ModelProto, error) {
	if len(data) == 0 {
		return nil, fmt.Errorf("sentencepiece: empty model payload")
	}

	model := &spb.ModelProto{}
	if err := proto.Unmarshal(data, model); err != nil {
		return nil, fmt.Errorf("sentencepiece: decode model: %w", err)
	}

	if len(model.GetPieces()) == 0 {
		return nil, fmt.Errorf("sentencepiece: model contains no sentence pieces")
	}

	return model, nil
}
