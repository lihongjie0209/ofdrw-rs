//go:build integration

package ofdrw

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestParseCanvasColorJavaReferences(t *testing.T) {
	path := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "canvas-color-java.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var reference struct {
		Cases []struct {
			Input    string
			Expected []uint8
		}
	}
	if err := json.Unmarshal(data, &reference); err != nil {
		t.Fatal(err)
	}
	if len(reference.Cases) != 461 {
		t.Fatalf("case count: %d", len(reference.Cases))
	}
	for _, entry := range reference.Cases {
		t.Run(entry.Input, func(t *testing.T) {
			paint, err := ParseCanvasColor(entry.Input)
			if err != nil {
				t.Fatal(err)
			}
			if entry.Expected == nil {
				if paint != nil {
					t.Fatalf("invalid color returned %+v", paint)
				}
				return
			}
			alpha := uint8(255)
			if len(entry.Expected) == 4 {
				alpha = entry.Expected[3]
			}
			expected := [3]uint8{entry.Expected[0], entry.Expected[1], entry.Expected[2]}
			if paint == nil || paint.Color != expected || paint.Alpha == nil || *paint.Alpha != alpha {
				t.Fatalf("paint: %+v, expected %v alpha %d", paint, expected, alpha)
			}
		})
	}
}

func TestParseCanvasColorRejectsNULAndInvalidUTF8(t *testing.T) {
	for _, input := range []string{"red\x00blue", string([]byte{255})} {
		if _, err := ParseCanvasColor(input); err == nil {
			t.Fatalf("accepted %q", input)
		}
	}
}
