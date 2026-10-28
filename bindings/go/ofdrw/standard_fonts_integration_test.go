//go:build integration

package ofdrw

import (
	"math"
	"os"
	"path/filepath"
	"strconv"
	"testing"
)

func TestImportsStandardFontsWithoutWidths(t *testing.T) {
	for _, tc := range []struct {
		name    string
		text    string
		advance float64
	}{
		{"standard14-tounicode.pdf", "中文", 6.804},
		{"standard14-symbol.pdf", "ΑΒ", 8.664},
		{"standard14-dingbats.pdf", "✁✂", 11.688},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", tc.name))
			if err != nil {
				t.Fatal(err)
			}
			document, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer document.Close()
			text, err := document.PageText(0)
			if err != nil || len(text) != 1 || text[0] != tc.text {
				t.Fatalf("text = %v, error = %v", text, err)
			}
			objects, err := document.PageTextObjects(0)
			if err != nil || len(objects) != 1 || len(objects[0].Codes) != 1 || objects[0].Codes[0].DeltaX == nil {
				t.Fatalf("text positions = %v, error = %v", objects, err)
			}
			delta, err := strconv.ParseFloat(*objects[0].Codes[0].DeltaX, 64)
			if err != nil || math.Abs(delta-tc.advance*25.4/72) > 1e-6 {
				t.Fatalf("advance = %v, error = %v", delta, err)
			}
			target := filepath.Join(t.TempDir(), "standard.ofd")
			if err := document.Save(target); err != nil {
				t.Fatal(err)
			}
			reopened, err := Open(target)
			if err != nil {
				t.Fatal(err)
			}
			defer reopened.Close()
			text, err = reopened.PageText(0)
			if err != nil || len(text) != 1 || text[0] != tc.text {
				t.Fatalf("reopened text = %v, error = %v", text, err)
			}
		})
	}
}
