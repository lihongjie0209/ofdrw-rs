//go:build integration

package ofdrw

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPDFEmptyActualText(t *testing.T) {
	for _, name := range []string{
		"truetype",
		"affine",
		"reflected",
		"cff",
		"cff-matrix",
		"cid-cff-matrix",
		"truetype-stroke",
		"cff-stroke",
		"truetype-dashed-stroke",
		"cff-dashed-stroke",
		"cff-opentype",
		"truetype-matrix-stroke",
		"cff-matrix-stroke",
		"cff-pattern",
		"cff-pattern-matrix-stroke",
	} {
		t.Run(name, func(t *testing.T) {
			source, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "actual-text-empty-"+name+".pdf"))
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(source)
			if err != nil {
				t.Fatal(err)
			}
			t.Cleanup(doc.Close)
			text, err := doc.PageText(0)
			if err != nil || len(text) != 0 {
				t.Fatalf("replacement: %v, %v", text, err)
			}
			png, err := doc.PagePNG(0, 144)
			if err != nil {
				t.Fatal(err)
			}
			data, err := doc.ToBytes()
			if err != nil {
				t.Fatal(err)
			}
			reopened, err := FromBytes(data)
			if err != nil {
				t.Fatal(err)
			}
			t.Cleanup(reopened.Close)
			after, err := reopened.PagePNG(0, 144)
			if err != nil || !bytes.Equal(png, after) {
				t.Fatalf("OFD raster: %v", err)
			}
			issues, err := reopened.ValidateReferences()
			if err != nil || len(issues) != 0 {
				t.Fatalf("references: %v, %v", issues, err)
			}
			pdf, err := reopened.ToPDF()
			if err != nil {
				t.Fatal(err)
			}
			exported, err := FromPDF(pdf)
			if err != nil {
				t.Fatal(err)
			}
			t.Cleanup(exported.Close)
			text, err = exported.PageText(0)
			if err != nil || strings.Join(text, "") != "" {
				t.Fatalf("PDF replacement: %v, %v", text, err)
			}
		})
	}
}
