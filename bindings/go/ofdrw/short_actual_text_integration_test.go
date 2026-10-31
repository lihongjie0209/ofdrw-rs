//go:build integration

package ofdrw

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPDFShortActualText(t *testing.T) {
	for _, name := range []string{"truetype", "affine", "reflected", "cff", "cff-matrix", "cid-cff-matrix"} {
		t.Run(name, func(t *testing.T) {
			source, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "actual-text-short-"+name+".pdf"))
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(source)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			text, err := doc.PageText(0)
			if err != nil || len(text) != 1 || text[0] != "X" {
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
			defer reopened.Close()
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
			defer exported.Close()
			text, err = exported.PageText(0)
			if err != nil || strings.Join(text, "") != "X" {
				t.Fatalf("PDF replacement: %v, %v", text, err)
			}
		})
	}
}
