//go:build integration

package ofdrw

import (
	"bytes"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestImportsUnicodeSequencesWithOriginalPDFGlyphs(t *testing.T) {
	for _, tc := range []struct {
		name   string
		text   []string
		glyphs []uint32
	}{
		{"type0-ligature-glyph.pdf", []string{"fiA", "fi"}, []uint32{3315, 36}},
		{"type1-opentype-cff.pdf", []string{"fiA", "fi"}, []uint32{2, 1}},
		{"type1-raw-cff.pdf", []string{"fiA", "fi"}, []uint32{2, 1}},
		{"type0-opentype-cid-cff.pdf", []string{"fiA", "fi"}, []uint32{2, 1}},
		{"type0-raw-cid-cff.pdf", []string{"fiA", "fi"}, []uint32{2, 1}},
		{"type0-name-cff.pdf", []string{"fiA", "fi"}, []uint32{2, 1}},
		{"type0-vertical-cid-cff.pdf", []string{"fiA", "fi"}, []uint32{2, 1}},
		{"type1-internal-cff.pdf", []string{"ﬁA", "ﬁ"}, []uint32{2, 1}},
		{"truetype-glyph-name-sequence.pdf", []string{"fiA", "fi"}, []uint32{3315, 36}},
		{"type0-vertical-default.pdf", []string{"fiA", "fi"}, []uint32{3315, 36}},
		{"type0-vertical-metrics.pdf", []string{"fiA", "fi"}, []uint32{3315, 36}},
		{"type0-vertical-transformed.pdf", []string{"fiA", "fi"}, []uint32{3315, 36}},
		{"truetype-ligature-glyph.pdf", []string{"fiA", "fi"}, []uint32{3315, 36}},
		{"type0-no-unicode-cmap-glyph.pdf", []string{"fiA", "fi"}, []uint32{3315, 36}},
		{"type0-variation-glyph.pdf", []string{"0\ufe00", "0\ufe00"}, []uint32{19}},
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
			if err != nil || !reflect.DeepEqual(text, tc.text) {
				t.Fatalf("text = %v, error = %v", text, err)
			}
			objects, err := document.PageTextObjects(0)
			if err != nil || len(objects) == 0 || len(objects[0].CGTransforms) != 1 {
				t.Fatalf("mapped text = %v, error = %v", objects, err)
			}
			if !reflect.DeepEqual(objects[0].CGTransforms[0].Glyphs, tc.glyphs) {
				t.Fatalf("glyphs = %v, want %v", objects[0].CGTransforms[0].Glyphs, tc.glyphs)
			}
			png, err := document.PagePNG(0, 72)
			if err != nil {
				t.Fatal(err)
			}
			target := filepath.Join(t.TempDir(), "mapped.ofd")
			if err := document.Save(target); err != nil {
				t.Fatal(err)
			}
			reopened, err := Open(target)
			if err != nil {
				t.Fatal(err)
			}
			defer reopened.Close()
			restored, err := reopened.PageTextObjects(0)
			if err != nil || !reflect.DeepEqual(restored, objects) {
				t.Fatalf("restored text = %v, error = %v", restored, err)
			}
			restoredPNG, err := reopened.PagePNG(0, 72)
			if err != nil || !bytes.Equal(restoredPNG, png) {
				t.Fatalf("restored raster changed, error = %v", err)
			}
		})
	}
}
