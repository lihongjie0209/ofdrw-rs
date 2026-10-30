//go:build integration

package ofdrw

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestImportsGraphicsSoftMasksFromPDF(t *testing.T) {
	tests := []struct{ name string }{
		{name: "alpha"}, {name: "luminosity-bc"}, {name: "captured-ctm"},
		{name: "text"}, {name: "text-fill-stroke-alpha"},
		{name: "group"}, {name: "nested-masks"}, {name: "pattern-cell-mask"}, {name: "fill-stroke-alpha"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			data, err := os.ReadFile(filepath.Join(
				"..",
				"..",
				"..",
				"crates/ofdrw/tests/fixtures",
				"soft-mask-"+tt.name+".pdf",
			))
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			count, err := doc.PageCount()
			if err != nil || count != 1 {
				t.Fatalf("page count = %d, error = %v", count, err)
			}
			svg, err := doc.PageSVG(0)
			if err != nil || !strings.Contains(svg, "<image") {
				t.Fatalf("pattern SVG missing: %v", err)
			}
			png, err := doc.PagePNG(0, 72)
			if err != nil {
				t.Fatal(err)
			}
			data, err = doc.ToBytes()
			if err != nil {
				t.Fatal(err)
			}
			reopened, err := FromBytes(data)
			if err != nil {
				t.Fatal(err)
			}
			defer reopened.Close()
			if tt.name == "text" || tt.name == "text-fill-stroke-alpha" {
				for _, document := range []*Document{doc, reopened} {
					text, err := document.PageText(0)
					if err != nil || strings.Join(text, "") != "A" {
						t.Fatalf("masked text = %v, error = %v", text, err)
					}
					objects, err := document.PageTextObjects(0)
					if err != nil || len(objects) != 1 {
						t.Fatalf("masked text objects = %v, error = %v", objects, err)
					}
				}
			}
			actual, err := reopened.PagePNG(0, 72)
			if err != nil || !bytes.Equal(actual, png) {
				t.Fatalf("pattern raster changed after round trip: %v", err)
			}
			issues, err := reopened.ValidateReferences()
			if err != nil || len(issues) != 0 {
				t.Fatalf("pattern references = %v, error = %v", issues, err)
			}
		})
	}
}
