//go:build integration

package ofdrw

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestCanvasPatternsMatchJavaWriter(t *testing.T) {
	root := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	metadata, err := os.ReadFile(filepath.Join(root, "canvas-pattern-java", "reference.json"))
	if err != nil {
		t.Fatal(err)
	}
	source, err := os.ReadFile(filepath.Join(root, "canvas-images-java", "source.png"))
	if err != nil {
		t.Fatal(err)
	}
	var reference struct {
		Cases []struct {
			Name       string      `json:"name"`
			Repetition string      `json:"repetition"`
			PPM        float64     `json:"ppm"`
			Size       *[2]float64 `json:"size"`
			Transform  *[6]float64 `json:"transform"`
			CanvasCTM  *[6]float64 `json:"canvas_ctm"`
			Stroke     bool        `json:"stroke"`
			Alpha      *float64    `json:"alpha"`
			Expected   struct {
				Width    float64    `json:"width"`
				Height   float64    `json:"height"`
				ImageCTM [6]float64 `json:"image_ctm"`
				Boundary [4]float64 `json:"image_boundary"`
			} `json:"expected"`
		} `json:"cases"`
	}
	if err := json.Unmarshal(metadata, &reference); err != nil {
		t.Fatal(err)
	}
	if len(reference.Cases) != 8 {
		t.Fatalf("case count %d", len(reference.Cases))
	}
	for _, entry := range reference.Cases {
		t.Run(entry.Name, func(t *testing.T) {
			doc, err := New()
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			java, err := Open(filepath.Join(root, "canvas-pattern-java", entry.Name+".ofd"))
			if err != nil {
				t.Fatal(err)
			}
			defer java.Close()
			if err := doc.AddBlankPage(40, 30); err != nil {
				t.Fatal(err)
			}
			area := PageArea{
				PhysicalBox:    [4]float64{0, 0, 40, 30},
				ApplicationBox: &[4]float64{0, 0, 40, 30},
			}
			if err := doc.SetPageArea(0, area); err != nil {
				t.Fatal(err)
			}
			pattern, err := doc.CreateCanvasPattern(0, source, "png", CanvasPatternOptions{
				Repetition: entry.Repetition, PixelsPerMM: &entry.PPM, ImageSize: entry.Size, CTM: entry.Transform,
			})
			if err != nil {
				t.Fatal(err)
			}
			if pattern.Width != entry.Expected.Width || pattern.Height != entry.Expected.Height || len(pattern.Cells) != 1 ||
				pattern.Cells[0].CTM == nil || *pattern.Cells[0].CTM != entry.Expected.ImageCTM ||
				pattern.Cells[0].Boundary != entry.Expected.Boundary {
				t.Fatalf("unexpected pattern %+v", pattern)
			}
			paths, err := doc.PagePaths(0)
			if err != nil {
				t.Fatal(err)
			}
			if len(paths) != 0 {
				t.Fatal("pattern creation painted page objects")
			}
			saved, err := json.Marshal(pattern)
			if err != nil {
				t.Fatal(err)
			}
			style := DrawParamStyle{FillPattern: &pattern}
			if entry.Stroke {
				width := 1.2
				style = DrawParamStyle{StrokePattern: &pattern, LineWidth: &width}
			}
			commands := []CanvasCommand{{Type: "set_draw_param", Style: &style}}
			if entry.CanvasCTM != nil {
				commands = append(commands, CanvasCommand{Type: "set_transform", Matrix: entry.CanvasCTM})
			}
			if entry.Alpha != nil {
				commands = append(commands, CanvasCommand{Type: "set_global_alpha", Alpha: entry.Alpha})
			}
			if entry.Stroke {
				commands = append(commands,
					CanvasCommand{Type: "rect", Rect: &[4]float64{2, 2, 18, 12}},
					CanvasCommand{Type: "stroke"},
				)
			} else {
				commands = append(commands, CanvasCommand{Type: "fill_rect", Rect: &[4]float64{2, 2, 18, 12}})
			}
			if _, err := doc.DrawCanvas(0, [4]float64{4, 3, 24, 20}, commands); err != nil {
				t.Fatal(err)
			}
			after, err := json.Marshal(pattern)
			if err != nil {
				t.Fatal(err)
			}
			if !bytes.Equal(after, saved) {
				t.Fatal("pattern mutated during drawing")
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
			for _, dpi := range []float64{25.4, 101.6} {
				actual, err := doc.PagePNG(0, dpi)
				if err != nil {
					t.Fatal(err)
				}
				expected, err := java.PagePNG(0, dpi)
				if err != nil {
					t.Fatal(err)
				}
				restored, err := reopened.PagePNG(0, dpi)
				if err != nil {
					t.Fatal(err)
				}
				if !bytes.Equal(actual, expected) || !bytes.Equal(actual, restored) {
					t.Fatalf("pixels differ at %v DPI", dpi)
				}
			}
			issues, err := reopened.ValidateReferences()
			if err != nil {
				t.Fatal(err)
			}
			if len(issues) != 0 {
				t.Fatalf("invalid references %+v", issues)
			}
		})
	}
}

func TestCanvasPatternFailuresPreserveResources(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	source, err := os.ReadFile(filepath.Join(
		"..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "canvas-images-java", "source.png",
	))
	if err != nil {
		t.Fatal(err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	zero := 0.
	tests := []struct {
		name    string
		options CanvasPatternOptions
	}{
		{name: "repetition", options: CanvasPatternOptions{Repetition: "repeat-x"}},
		{name: "density", options: CanvasPatternOptions{PixelsPerMM: &zero}},
		{name: "size", options: CanvasPatternOptions{ImageSize: &[2]float64{0, 4}}},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if _, err := doc.CreateCanvasPattern(0, source, "png", tt.options); err == nil {
				t.Fatal("accepted invalid options")
			}
			after, err := doc.ToBytes()
			if err != nil {
				t.Fatal(err)
			}
			if !bytes.Equal(after, before) {
				t.Fatal("failed creation changed resource bytes")
			}
		})
	}
	doc.Close()
	if _, err := doc.CreateCanvasPattern(0, source, "png", CanvasPatternOptions{}); err == nil {
		t.Fatal("accepted closed document")
	}
}
