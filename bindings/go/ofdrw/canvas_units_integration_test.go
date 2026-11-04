//go:build integration

package ofdrw

import (
	"encoding/json"
	"math"
	"os"
	"path/filepath"
	"strconv"
	"testing"
)

func TestCanvasUnitsJavaReferences(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "canvas-units-java.json"))
	if err != nil {
		t.Fatal(err)
	}
	var reference struct {
		Cases []struct {
			Op           string `json:"op"`
			PPMBits      string `json:"ppm_bits"`
			Input        int32  `json:"input"`
			InputBits    string `json:"input_bits"`
			Expected     int32  `json:"expected"`
			ExpectedBits string `json:"expected_bits"`
		} `json:"cases"`
	}
	if err := json.Unmarshal(data, &reference); err != nil {
		t.Fatal(err)
	}
	if len(reference.Cases) != 329 {
		t.Fatalf("case count: %d", len(reference.Cases))
	}
	fromBits := func(value string) float64 {
		t.Helper()
		bits, err := strconv.ParseUint(value, 16, 64)
		if err != nil {
			t.Fatal(err)
		}
		return math.Float64frombits(bits)
	}
	for i, entry := range reference.Cases {
		t.Run(strconv.Itoa(i), func(t *testing.T) {
			ppm := fromBits(entry.PPMBits)
			if entry.Op == "mm" {
				actual, err := CanvasPixelsToMM(entry.Input, ppm)
				if err != nil {
					t.Fatal(err)
				}
				if math.Float64bits(actual) != math.Float64bits(fromBits(entry.ExpectedBits)) {
					t.Fatalf("unexpected mm bits: %x", math.Float64bits(actual))
				}
			} else {
				actual, err := CanvasMMToPixels(fromBits(entry.InputBits), ppm)
				if err != nil {
					t.Fatal(err)
				}
				if actual != entry.Expected {
					t.Fatalf("got %d, expected %d", actual, entry.Expected)
				}
			}
		})
	}
}

func TestCanvasUnitsInvalidDensity(t *testing.T) {
	for i, ppm := range []float64{0, -1, math.NaN(), math.Inf(1), math.Inf(-1)} {
		t.Run(strconv.Itoa(i), func(t *testing.T) {
			if _, err := CanvasPixelsToMM(4, ppm); err == nil {
				t.Fatal("accepted invalid density")
			}
			if _, err := CanvasMMToPixels(4, ppm); err == nil {
				t.Fatal("accepted invalid density")
			}
		})
	}
}
