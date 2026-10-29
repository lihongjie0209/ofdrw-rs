//go:build integration

package ofdrw

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestImportsAffineDirectShadingsFromPDF(t *testing.T) {
	tests := []struct{ name string }{
		{name: "axial-shear"}, {name: "radial-shear"}, {name: "axial-alpha"},
		{name: "radial-clip"}, {name: "axial-form"}, {name: "radial-multiply"}, {name: "radial-rotate"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			data, err := os.ReadFile(filepath.Join(
				"..",
				"..",
				"..",
				"crates/ofdrw/tests/fixtures",
				"affine-shading-"+tt.name+".pdf",
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
			if err != nil || !strings.Contains(svg, "<pattern") {
				t.Fatalf("pattern SVG missing: %v", err)
			}
			units, err := doc.PageCompositeUnits(0)
			if err != nil || len(units) == 0 {
				t.Fatalf("pattern units = %v, error = %v", units, err)
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
