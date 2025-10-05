// Package bluesky provides MST (Merkle Search Tree) parsing using indigo library
package bluesky

import (
	"bytes"
	"context"
	"fmt"

	"github.com/bluesky-social/indigo/repo"
	"github.com/ipfs/go-cid"
)

// ExtractCIDToRKeyMapping extracts CID -> rkey mappings from CAR file using indigo's repo parser
func ExtractCIDToRKeyMapping(carData []byte, collection string) (map[string]string, error) {
	// Use indigo's repo library to read the repository from CAR
	ctx := context.Background()
	reader := bytes.NewReader(carData)

	r, err := repo.ReadRepoFromCar(ctx, reader)
	if err != nil {
		return nil, fmt.Errorf("failed to read repo from CAR: %w", err)
	}

	// Extract all records from the specified collection
	cidToRKey := make(map[string]string)

	// Iterate through all records in the collection
	if err := r.ForEach(ctx, collection, func(k string, v cid.Cid) error {
		// k is the rkey, v is the CID of the record
		cidToRKey[v.String()] = k
		return nil
	}); err != nil {
		return nil, fmt.Errorf("failed to iterate records: %w", err)
	}

	return cidToRKey, nil
}
