//go:build integration

package ofdrw

import (
	"bytes"
	"encoding/json"
	"math"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestCanvasMatchesJavaReferenceGeometryAndPixels(t *testing.T) {
	root := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "canvas-java")
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
	data, err = os.ReadFile(filepath.Join(root, "reference.json"))
	if err != nil {
		t.Fatal(err)
	}
	var references struct {
		Cases []struct {
			Name            string `json:"name"`
			RenderReference string `json:"render_reference"`
			Objects         []struct {
				LineWidth float64 `json:"line_width"`
			} `json:"objects"`
		} `json:"cases"`
	}
	if err := json.Unmarshal(data, &references); err != nil {
		t.Fatal(err)
	}
	for index, entry := range requests.Cases {
		t.Run(entry.Name, func(t *testing.T) {
			reference := references.Cases[index]
			if reference.Name != entry.Name {
				t.Fatal("reference order mismatch")
			}
			file := reference.RenderReference
			if file == "" {
				file = entry.Name + ".ofd"
			}
			java, err := Open(filepath.Join(root, file))
			if err != nil {
				t.Fatal(err)
			}
			defer java.Close()
			doc, err := New()
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
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
			after, err := json.Marshal(entry.Commands)
			if err != nil || !bytes.Equal(before, after) {
				t.Fatal("caller commands mutated")
			}
			if len(ids) != len(reference.Objects) {
				t.Fatalf("object count %d, want %d", len(ids), len(reference.Objects))
			}
			paths, err := doc.PagePaths(0)
			if err != nil {
				t.Fatal(err)
			}
			styles, err := doc.PageObjectStrokeStyles(0)
			if err != nil {
				t.Fatal(err)
			}
			for index, id := range ids {
				if paths[index].ID == nil || *paths[index].ID != id {
					t.Fatal("ID order mismatch")
				}
				if styles[index].Style.LineWidth != reference.Objects[index].LineWidth {
					t.Fatal("reference width mismatch")
				}
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
					t.Fatalf("%v DPI pixels differ", dpi)
				}
			}
			issues, err := doc.ValidateReferences()
			if err != nil || len(issues) != 0 {
				t.Fatalf("references: %+v, %v", issues, err)
			}
		})
	}
}

func TestCanvasEmptyZeroOpacityClearAndInvalidBatches(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	box := [4]float64{0, 0, 40, 30}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	ids, err := doc.DrawCanvas(0, box, nil)
	if err != nil || len(ids) != 0 {
		t.Fatalf("empty batch: %+v, %v", ids, err)
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatal("empty batch mutated document")
	}
	negative := -1.0
	badAlpha := 1.1
	nonfinite := math.NaN()
	for _, bad := range []CanvasCommand{
		{Type: "unknown"}, {Type: "stroke", EvenOdd: true}, {Type: "move_to"},
		{Type: "set_line_width", Width: &negative}, {Type: "set_global_alpha", Alpha: &badAlpha},
		{Type: "set_global_alpha", Alpha: &nonfinite},
	} {
		commands := []CanvasCommand{{Type: "rect", Rect: &[4]float64{1, 1, 4, 4}}, {Type: "stroke"}, bad}
		if _, err := doc.DrawCanvas(0, box, commands); err == nil {
			t.Fatalf("accepted invalid command %+v", bad)
		}
		after, err := doc.ToBytes()
		if err != nil || !bytes.Equal(before, after) {
			t.Fatal("failed batch mutated document")
		}
	}
	if _, err := doc.DrawCanvas(-1, box, nil); err == nil {
		t.Fatal("accepted negative page")
	}
	if _, err := doc.DrawCanvas(9, box, nil); err == nil {
		t.Fatal("accepted missing page")
	}
	zero := 0.0
	commands := []CanvasCommand{
		{Type: "set_draw_param", Style: &DrawParamStyle{LineWidth: &zero, Cap: "round"}},
		{Type: "rect", Rect: &[4]float64{2, 2, 12, 8}},
		{Type: "set_global_alpha", Alpha: &zero}, {Type: "stroke"},
		{Type: "set_global_alpha"}, {Type: "stroke"},
	}
	ids, err = doc.DrawCanvas(0, box, commands)
	if err != nil || len(ids) != 2 {
		t.Fatalf("zero state batch: %+v, %v", ids, err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual([]uint8{objects[0].Alpha, objects[1].Alpha}, []uint8{0, 255}) {
		t.Fatal("opacity not preserved")
	}
	styles, err := doc.PageObjectStrokeStyles(0)
	if err != nil || styles[0].Style.LineWidth != 0 || styles[1].Style.LineWidth != 0 {
		t.Fatal("zero width not preserved")
	}
	doc.Close()
	if _, err := doc.DrawCanvas(0, box, nil); err == nil {
		t.Fatal("accepted closed document")
	}
}
