//go:build integration

package ofdrw

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestCanvasImagesMatchJavaReferences(t *testing.T) {
	root := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "canvas-images-java")
	data, err := os.ReadFile(filepath.Join(root, "requests.json"))
	if err != nil {
		t.Fatal(err)
	}
	var requests struct {
		Boundary [4]float64 `json:"boundary"`
		Cases    []struct {
			Name     string          `json:"name"`
			Commands []CanvasCommand `json:"commands"`
		} `json:"cases"`
	}
	if err := json.Unmarshal(data, &requests); err != nil {
		t.Fatal(err)
	}
	for _, entry := range requests.Cases {
		t.Run(entry.Name, func(t *testing.T) {
			doc, err := New()
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			java, err := Open(filepath.Join(root, entry.Name+".ofd"))
			if err != nil {
				t.Fatal(err)
			}
			defer java.Close()
			if err := doc.AddBlankPage(40, 30); err != nil {
				t.Fatal(err)
			}
			if err := doc.SetPageArea(0, PageArea{
				PhysicalBox:    [4]float64{0, 0, 40, 30},
				ApplicationBox: &[4]float64{0, 0, 40, 30},
			}); err != nil {
				t.Fatal(err)
			}
			before, err := json.Marshal(entry.Commands)
			if err != nil {
				t.Fatal(err)
			}
			ids, err := doc.DrawCanvas(0, requests.Boundary, entry.Commands)
			if err != nil {
				t.Fatal(err)
			}
			if len(ids) != 1 {
				t.Fatalf("IDs: %v", ids)
			}
			after, err := json.Marshal(entry.Commands)
			if err != nil || !bytes.Equal(before, after) {
				t.Fatal("caller commands mutated")
			}
			images, err := doc.PageImages(0)
			if err != nil {
				t.Fatal(err)
			}
			if len(images) != 1 || images[0].ID == nil || *images[0].ID != ids[0] {
				t.Fatal("image ID mismatch")
			}
			for _, dpi := range []float64{25.4, 101.6} {
				actual, err := doc.PagePNG(0, dpi)
				if err != nil {
					t.Fatal(err)
				}
				expected, err := java.PagePNG(0, dpi)
				if err != nil {
					t.Fatal(err)
				}
				if !bytes.Equal(actual, expected) {
					t.Fatalf("pixels differ at %v DPI", dpi)
				}
			}
			encoded, err := doc.ToBytes()
			if err != nil {
				t.Fatal(err)
			}
			reopened, err := FromBytes(encoded)
			if err != nil {
				t.Fatal(err)
			}
			defer reopened.Close()
			issues, err := reopened.ValidateReferences()
			if err != nil || len(issues) != 0 {
				t.Fatalf("references: %v, %v", issues, err)
			}
		})
	}
}

func TestCanvasImageInvalidBytesAndZeroPixelScaleRollBack(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	root := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "canvas-images-java")
	data, err := os.ReadFile(filepath.Join(root, "source.png"))
	if err != nil {
		t.Fatal(err)
	}
	valid := CanvasCommand{Type: "draw_image", Data: data, Format: "png", Rect: &[4]float64{1, 1, 6, 4}}
	zero := 0.0
	for _, invalid := range []CanvasCommand{
		{Type: "draw_image", Data: []byte("bad"), Format: "png", Rect: &[4]float64{1, 1, 6, 4}},
		{Type: "draw_image_at", Data: data, Format: "png", Point: &[2]float64{}, PixelsPerMM: &zero},
	} {
		if _, err := doc.DrawCanvas(0, [4]float64{0, 0, 40, 30}, []CanvasCommand{valid, invalid}); err == nil {
			t.Fatal("invalid image accepted")
		}
		after, err := doc.ToBytes()
		if err != nil || !bytes.Equal(before, after) {
			t.Fatal("invalid image batch changed the document")
		}
	}
}
