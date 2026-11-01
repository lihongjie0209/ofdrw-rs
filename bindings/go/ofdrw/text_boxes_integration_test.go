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
	"testing"
)

func TestTextBoxesMatchReferenceDrawAndContinue(t *testing.T) {
	for _, input := range []struct {
		name  string
		shape bool
	}{{"plain", false}, {"shaped", true}} {
		t.Run(input.name, func(t *testing.T) {
			data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "text-boxes.json"))
			if err != nil {
				t.Fatal(err)
			}
			var fixture struct {
				Request struct {
					Boxes []TextBox `json:"boxes"`
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
			font, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-sans.ttf"))
			if err != nil {
				t.Fatal(err)
			}
			id, err := doc.AddEmbeddedFont("DejaVu Sans", font)
			if err != nil {
				t.Fatal(err)
			}
			fixture.Request.Boxes[0].Style.FontID = strconv.Itoa(id)
			fixture.Request.Boxes[0].Style.Shape = input.shape
			before, err := json.Marshal(fixture.Request.Boxes)
			if err != nil {
				t.Fatal(err)
			}
			result, err := doc.AddTextBoxes(0, fixture.Request.Boxes, fixture.Request.BoxLayoutStyle)
			if err != nil || !reflect.DeepEqual(result, fixture.Expected) {
				t.Fatalf("layout: %+v, %v", result, err)
			}
			after, err := json.Marshal(fixture.Request.Boxes)
			if err != nil || !bytes.Equal(before, after) {
				t.Fatalf("input changed: %v", err)
			}
			text, err := doc.PageText(1)
			if err != nil || !reflect.DeepEqual(text, []string{"D"}) {
				t.Fatalf("text: %v, %v", text, err)
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
			text, err = reopened.PageText(0)
			if err != nil || !reflect.DeepEqual(text, []string{"A", "B", "C"}) {
				t.Fatalf("reopened: %v, %v", text, err)
			}
			box := fixture.Request.Boxes[0]
			box.Spans = []FlowSpan{{Text: "E"}}
			geometry := fixture.Request.BoxLayoutStyle
			geometry.Start = result.Next
			continued, err := doc.AddTextBoxes(0, []TextBox{box}, geometry)
			if err != nil || !reflect.DeepEqual(continued.Pages, []int{1}) {
				t.Fatalf("continuation: %+v, %v", continued, err)
			}
		})
	}
}

func TestTextBoxesValidationIsAtomic(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	box := TextBox{Layout: LayoutBox{Size: [2]float64{8, 0}}, Spans: []FlowSpan{{Text: "A"}}}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	for _, input := range []struct {
		name  string
		width float64
	}{{"zero", 0}, {"negative", -1}, {"nan", math.NaN()}} {
		t.Run(input.name, func(t *testing.T) {
			bad := box
			bad.Layout.Size[0] = input.width
			if _, err := doc.AddTextBoxes(0, []TextBox{box, bad}, BoxLayoutStyle{}); err == nil {
				t.Fatal("expected error")
			}
			after, err := doc.ToBytes()
			if err != nil || !bytes.Equal(before, after) {
				t.Fatalf("source changed: %v", err)
			}
		})
	}
	if _, err := doc.AddTextBoxes(-1, []TextBox{box}, BoxLayoutStyle{}); err == nil {
		t.Fatal("negative index accepted")
	}
	if _, err := doc.AddTextBoxes(0, nil, BoxLayoutStyle{}); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddTextBoxes(0, []TextBox{box}, BoxLayoutStyle{}); err != nil {
		t.Fatal(err)
	}
	if box.Style != (FlowTextStyle{}) {
		t.Fatal("caller style changed")
	}
	doc.Close()
	if _, err := doc.AddTextBoxes(0, []TextBox{box}, BoxLayoutStyle{}); err == nil {
		t.Fatal("closed document accepted")
	}
}
