//go:build integration

package ofdrw

import (
	"bytes"
	"encoding/json"
	"math"
	"os"
	"path/filepath"
	"reflect"
	"strconv"
	"strings"
	"testing"
)

func TestBoxLayoutFragmentsAndContentRectangles(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "box-layout.json"))
	if err != nil {
		t.Fatal(err)
	}
	var fixture struct {
		Request struct {
			Items []BoxLayoutItem `json:"items"`
			BoxLayoutStyle
		} `json:"request"`
		Expected BoxLayoutResult `json:"expected"`
	}
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	result, err := doc.AddBoxLayout(0, fixture.Request.Items, fixture.Request.BoxLayoutStyle)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(result, fixture.Expected) {
		t.Fatalf("layout: %+v, want %+v", result, fixture.Expected)
	}
	style := fixture.Request.BoxLayoutStyle
	style.Start = result.Next
	continued, err := doc.AddBoxLayout(0, []BoxLayoutItem{{Type: "box", LayoutBox: &LayoutBox{
		Size: [2]float64{5, 4}, Margin: [4]float64{0.5, 0.5, 0.5, 0.5}, Border: [4]float64{0.25, 0.25, 0.25, 0.25},
		Padding: [4]float64{0.5, 0.5, 0.5, 0.5}, Offset: [2]float64{1, 0}, BackgroundColor: &[3]uint8{230, 240, 250},
	}}}, style)
	if err != nil {
		t.Fatal(err)
	}
	if len(continued.Placements) != 1 {
		t.Fatalf("placements: %+v", continued)
	}
	placed := continued.Placements[0]
	if placed.Allocation != [4]float64{2, 8, 7.5, 6.5} || placed.ContentBox != [4]float64{4.25, 9.25, 5, 4} {
		t.Fatalf("placed: %+v", placed)
	}
	if _, err := doc.AddTextObject(placed.PageIndex, "Hi", placed.ContentBox, 2, ""); err != nil {
		t.Fatal(err)
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
	text, err := reopened.PageText(1)
	if err != nil || !reflect.DeepEqual(text, []string{"Hi"}) {
		t.Fatalf("text: %v, %v", text, err)
	}
	refs, err := reopened.ValidateReferences()
	if err != nil || len(refs) != 0 {
		t.Fatalf("references: %v, %v", refs, err)
	}
}

func TestBoxLayoutRejectsInvalidInputsAtomically(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	valid := BoxLayoutItem{Type: "box", LayoutBox: &LayoutBox{Size: [2]float64{4, 4}, BackgroundColor: &[3]uint8{255, 0, 0}}}
	for _, input := range []struct {
		name string
		item BoxLayoutItem
	}{
		{"negative size", BoxLayoutItem{Type: "box", LayoutBox: &LayoutBox{Size: [2]float64{-1, 3}}}},
		{"unbounded height", BoxLayoutItem{Type: "box", LayoutBox: &LayoutBox{Size: [2]float64{1, 1e308}}}},
		{"unsupported float", BoxLayoutItem{Type: "box", LayoutBox: &LayoutBox{Size: [2]float64{1, 1}, Float: "justify"}}},
		{"nonfinite size", BoxLayoutItem{Type: "box", LayoutBox: &LayoutBox{Size: [2]float64{math.NaN(), 1}}}},
		{"missing box", BoxLayoutItem{Type: "box"}},
		{"break with box attributes", BoxLayoutItem{Type: "line_break", LayoutBox: &LayoutBox{Size: [2]float64{1, 1}}}},
	} {
		t.Run(input.name, func(t *testing.T) {
			if _, err := doc.AddBoxLayout(0, []BoxLayoutItem{valid, input.item}, BoxLayoutStyle{}); err == nil {
				t.Fatal("accepted invalid box layout")
			}
			after, err := doc.ToBytes()
			if err != nil {
				t.Fatal(err)
			}
			if !bytes.Equal(before, after) {
				t.Fatal("failed layout changed document")
			}
		})
	}
	if _, err := doc.AddBoxLayout(-1, nil, BoxLayoutStyle{}); err == nil {
		t.Fatal("accepted negative document index")
	}
	empty, err := doc.AddBoxLayout(0, nil, BoxLayoutStyle{})
	if err != nil || len(empty.Pages) != 0 || len(empty.Placements) != 0 || empty.Next != nil {
		t.Fatalf("empty layout: %+v, %v", empty, err)
	}
	result, err := doc.AddBoxLayout(0, []BoxLayoutItem{{Type: "page_break"}, {Type: "page_break"}}, DefaultBoxLayoutStyle())
	if err != nil || !reflect.DeepEqual(result.Pages, []int{0, 1}) || result.Next == nil || result.Next.Y != 25.4 {
		t.Fatalf("breaks: %+v, %v", result, err)
	}
	doc.Close()
	if _, err := doc.AddBoxLayout(0, nil, BoxLayoutStyle{}); err == nil {
		t.Fatal("accepted closed document")
	}
}

func TestDashedBoxBordersMatchJavaReference(t *testing.T) {
	fixturePath := filepath.Join(
		"..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "box-border-dashes-java.json",
	)
	data, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatal(err)
	}
	var fixture struct {
		Cases []struct {
			Name   string     `json:"name"`
			Dash   []float64  `json:"dash"`
			Border [4]float64 `json:"border"`
			Paths  []struct {
				Boundary []float64 `json:"boundary"`
				Pattern  []float64 `json:"pattern"`
			} `json:"paths"`
		} `json:"cases"`
	}
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatal(err)
	}
	for _, entry := range fixture.Cases {
		t.Run(entry.Name, func(t *testing.T) {
			doc, err := New()
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			source := LayoutBox{Size: [2]float64{10, 5}, Border: entry.Border, BorderDash: entry.Dash}
			style := DefaultBoxLayoutStyle()
			style.PageSize, style.Margins = [2]float64{30, 20}, [4]float64{2, 2, 2, 2}
			items := []BoxLayoutItem{{Type: "box", LayoutBox: &source}}
			if _, err := doc.AddBoxLayout(0, items, style); err != nil {
				t.Fatal(err)
			}
			paths, err := doc.PagePaths(0)
			if err != nil || len(paths) != len(entry.Paths) {
				t.Fatalf("paths: %v, %v", paths, err)
			}
			for i, expected := range entry.Paths {
				if !reflect.DeepEqual(paths[i].Boundary, expected.Boundary) {
					t.Fatalf("boundary: %v", paths[i].Boundary)
				}
			}
			pages, err := doc.Pages()
			if err != nil {
				t.Fatal(err)
			}
			xml, err := doc.Entry(pages[0].Path)
			if err != nil {
				t.Fatal(err)
			}
			var pattern []string
			for _, value := range entry.Paths[0].Pattern {
				pattern = append(pattern, strconv.FormatFloat(
					value, 'g', -1, 64,
				))
			}
			if strings.Count(string(xml), `DashPattern="`+strings.Join(pattern, " ")+`"`) != len(entry.Paths) {
				t.Fatal("incorrect dash pattern")
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
			actual, err := reopened.PagePaths(0)
			if err != nil || !reflect.DeepEqual(actual, paths) {
				t.Fatalf("roundtrip: %v", err)
			}
			for _, dash := range [][]float64{{0}, {-1}, {1, 0}, {math.MaxFloat64}, {math.NaN()}} {
				source.BorderDash = dash
				if _, err := doc.AddBoxLayout(0, items, style); err == nil {
					t.Fatal("invalid dash accepted")
				}
				after, err := doc.ToBytes()
				if err != nil || !bytes.Equal(encoded, after) {
					t.Fatalf("mutation: %v", err)
				}
			}
		})
	}
}
