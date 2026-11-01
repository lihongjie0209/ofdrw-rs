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
