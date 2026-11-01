//go:build integration

package ofdrw

import (
	"bytes"
	"os"
	"path/filepath"
	"slices"
	"strconv"
	"testing"
)

func TestFlowPageContinuation(t *testing.T) {
	font, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-serif.ttf"))
	if err != nil {
		t.Fatal(err)
	}
	for _, test := range []struct {
		name           string
		styled, shaped bool
	}{
		{name: "plain"}, {name: "styled", styled: true},
		{name: "shaped", shaped: true}, {name: "shaped_spans", styled: true, shaped: true},
	} {
		t.Run(test.name, func(t *testing.T) {
			doc, err := New()
			if err != nil {
				t.Fatal(err)
			}
			t.Cleanup(doc.Close)
			if err := doc.AddTextPage("seed", 20, 20); err != nil {
				t.Fatal(err)
			}
			fontID, err := doc.AddEmbeddedFont("DejaVu Serif", font)
			if err != nil {
				t.Fatal(err)
			}
			style := FlowTextStyle{PageSize: [2]float64{20, 20}, Margins: [4]float64{2, 2, 2, 2},
				FontSize: 2, LineHeight: 3, FontID: strconv.Itoa(fontID), Shape: test.shaped,
				Start: &FlowStart{PageIndex: 0, Y: 10}}
			write := func() ([]int, error) {
				if test.styled {
					return doc.AddFlowedSpans(0, []FlowSpan{{Text: "A\nB\nC\nD"}}, style)
				}
				return doc.AddFlowedText(0, "A\nB\nC\nD", style)
			}
			pages, err := write()
			if err != nil || !slices.Equal(pages, []int{0, 1}) {
				t.Fatalf("pages: %v, %v", pages, err)
			}
			for page, want := range [][]string{{"seed", "A", "B"}, {"C", "D"}} {
				text, err := doc.PageText(page)
				if err != nil || !slices.Equal(text, want) {
					t.Fatalf("page %d: %v, %v", page, text, err)
				}
			}
			objects, err := doc.PageTextObjects(0)
			if err != nil || len(objects) != 3 || objects[1].Boundary[1] != 10 {
				t.Fatalf("start: %v, %v", objects, err)
			}
			objects, err = doc.PageTextObjects(1)
			if err != nil || len(objects) != 2 || objects[0].Boundary[1] != 2 {
				t.Fatalf("continuation: %v, %v", objects, err)
			}
			before, err := doc.ToBytes()
			if err != nil {
				t.Fatal(err)
			}
			style.Start.Y = 19
			if _, err := write(); err == nil {
				t.Fatal("expected invalid start error")
			}
			after, err := doc.ToBytes()
			if err != nil || !bytes.Equal(before, after) {
				t.Fatalf("invalid request changed document: %v", err)
			}
			reopened, err := FromBytes(before)
			if err != nil {
				t.Fatal(err)
			}
			t.Cleanup(reopened.Close)
			issues, err := reopened.ValidateReferences()
			if err != nil || len(issues) != 0 {
				t.Fatalf("references: %v, %v", issues, err)
			}
			text, err := reopened.PageText(0)
			if err != nil || !slices.Equal(text, []string{"seed", "A", "B"}) {
				t.Fatalf("round trip: %v, %v", text, err)
			}
		})
	}
}
