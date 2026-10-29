//go:build integration

package ofdrw

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func TestImportsGouraudMeshesFromPDF(t *testing.T) {
	tests := []struct {
		name string
		kind string
	}{
		{"mesh-free-rgb", "Gouraud"},
		{"mesh-lattice-rgb", "LaGouraud"},
		{"mesh-packed-gray", "Gouraud"},
		{"mesh-wide-rgb", "Gouraud"},
		{"mesh-lattice-clipped", "LaGouraud"},
		{"mesh-cmyk", "Gouraud"},
		{"mesh-calgray", "Gouraud"},
		{"mesh-function-array", "Gouraud"},
		{"mesh-sampled-function", "Gouraud"},
		{"mesh-stitching-function", "Gouraud"},
		{"mesh-calculator-function", "Gouraud"},
		{"patch-coons-flat", "Gouraud"},
		{"patch-coons-curved", "Gouraud"},
		{"patch-tensor-curved", "Gouraud"},
		{"patch-tensor-folded", "Gouraud"},
		{"patch-coons-reuse-1", "Gouraud"},
		{"patch-coons-reuse-2", "Gouraud"},
		{"patch-coons-reuse-3", "Gouraud"},
		{"patch-tensor-reuse", "Gouraud"},
		{"patch-coons-function", "Gouraud"},
		{"patch-tensor-clipped", "Gouraud"},
		{"patch-coons-wide", "Gouraud"},
		{"patch-coons-packed", "Gouraud"},
		{"patch-coons-calgray", "Gouraud"},
		{"patch-tensor-cmyk", "Gouraud"},
		{"mesh-function", "Gouraud"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", tt.name+".pdf"))
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			paints, err := doc.PageObjectPaints(0)
			if err != nil || len(paints) != 1 || paints[0].Fill == nil || paints[0].Fill.Kind != tt.kind {
				t.Fatalf("mesh paints = %+v, error = %v", paints, err)
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
				t.Fatalf("mesh raster changed after round trip: %v", err)
			}
			issues, err := reopened.ValidateReferences()
			if err != nil || len(issues) != 0 {
				t.Fatalf("mesh references = %v, error = %v", issues, err)
			}
		})
	}
}

func TestImportsFunctionBasedShadingsFromPDF(t *testing.T) {
	tests := []struct{ name string }{
		{"plane-calculator-rgb"},
		{"plane-calculator-step"},
		{"plane-component-array"},
		{"plane-sampled-rgb"},
		{"plane-sampled-packed"},
		{"plane-sampled-cubic"},
		{"plane-sampled-cubic-small"},
		{"plane-sampled-reversed"},
		{"plane-cmyk"},
		{"plane-calgray"},
		{"plane-transformed"},
		{"plane-bbox-background"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", tt.name+".pdf"))
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			images, err := doc.PageImages(0)
			if err != nil || len(images) != 1 {
				t.Fatalf("function shading images = %v, error = %v", images, err)
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
				t.Fatalf("function shading raster changed after round trip: %v", err)
			}
			issues, err := reopened.ValidateReferences()
			if err != nil || len(issues) != 0 {
				t.Fatalf("function shading references = %v, error = %v", issues, err)
			}
		})
	}
}

func TestImportsSeparationColorsFromPDF(t *testing.T) {
	tests := []struct{ name string }{
		{"separation-paths"},
		{"separation-cmyk"},
		{"separation-sampled"},
		{"separation-stitching"},
		{"separation-calculator"},
		{"separation-image"},
		{"separation-image16-mask"},
		{"separation-jpeg"},
		{"separation-indexed"},
		{"separation-all"},
		{"separation-none"},
		{"separation-none-clipping"},
		{"separation-none-independent"},
		{"separation-none-text-clip"},
		{"separation-shading"},
		{"separation-mesh"},
		{"separation-patch"},
		{"separation-plane"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", tt.name+".pdf"))
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			if tt.name == "separation-none" {
				text, err := doc.PageText(0)
				if err != nil || len(text) != 1 || text[0] != "none" {
					t.Fatalf("None text = %v, error = %v", text, err)
				}
				images, err := doc.PageImages(0)
				if err != nil || len(images) != 0 {
					t.Fatalf("None images = %v, error = %v", images, err)
				}
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
				t.Fatalf("Separation raster changed after round trip: %v", err)
			}
			issues, err := reopened.ValidateReferences()
			if err != nil || len(issues) != 0 {
				t.Fatalf("Separation references = %v, error = %v", issues, err)
			}
		})
	}
}

func TestImportsDeviceNColorsFromPDF(t *testing.T) {
	tests := []struct{ name string }{
		{"devicen-jpx-two"},
		{"devicen-jpx-five"},
		{"devicen-jpx-thirty-two"},
		{"devicen-jpx-five-mask"},
		{"devicen-jpx-five-sixteen-mask"},
		{"devicen-jpx-five-soft-mask"},
		{"devicen-jpx-five-no-bpc"},
		{"devicen-jpx-four"},
		{"devicen-jpx-three-alpha"},
		{"devicen-jpx-five-container"},
		{"devicen-paths"},
		{"devicen-sampled"},
		{"devicen-cubic"},
		{"devicen-cmyk"},
		{"devicen-three-input"},
		{"devicen-image"},
		{"devicen-image-five"},
		{"devicen-image-thirty-two"},
		{"devicen-image16-mask"},
		{"devicen-indexed"},
		{"devicen-none"},
		{"devicen-mixed-none"},
		{"devicen-none-clipping"},
		{"devicen-nchannel"},
		{"devicen-shading"},
		{"devicen-mesh"},
		{"devicen-patch"},
		{"devicen-plane"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", tt.name+".pdf"))
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			if tt.name == "devicen-none" {
				text, err := doc.PageText(0)
				if err != nil || len(text) != 1 || text[0] != "none" {
					t.Fatalf("None text = %v, error = %v", text, err)
				}
				images, err := doc.PageImages(0)
				if err != nil || len(images) != 0 {
					t.Fatalf("None images = %v, error = %v", images, err)
				}
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
				t.Fatalf("DeviceN raster changed after round trip: %v", err)
			}
			issues, err := reopened.ValidateReferences()
			if err != nil || len(issues) != 0 {
				t.Fatalf("DeviceN references = %v, error = %v", issues, err)
			}
		})
	}
}

func TestImportsImageRenderingIntentsFromPDF(t *testing.T) {
	tests := []struct{ name string }{
		{"image-intent-xobject"},
		{"image-intent-form"},
		{"image-intent-inline"},
		{"image-intent-inline-embedded-ei"},
		{"image-intent-inline-trailing-cr"},
		{"image-intent-inline-flate"},
		{"image-intent-form-inline"},
		{"image-intent-masked"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", tt.name+".pdf"))
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			colors := make([][]byte, 5)
			for index := range colors {
				colors[index], err = doc.PageImageData(0, index)
				if err != nil {
					t.Fatal(err)
				}
			}
			if !bytes.Equal(colors[0], colors[2]) || bytes.Equal(colors[0], colors[1]) || bytes.Equal(colors[0], colors[3]) {
				t.Fatal("image intent inheritance or saved state is incorrect")
			}
			if tt.name != "image-intent-masked" && !bytes.Equal(colors[1], colors[4]) {
				t.Fatal("image Intent did not override the graphics state")
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
				t.Fatalf("image intent raster changed after round trip: %v", err)
			}
			issues, err := reopened.ValidateReferences()
			if err != nil || len(issues) != 0 {
				t.Fatalf("image intent references = %v, error = %v", issues, err)
			}
		})
	}
}
