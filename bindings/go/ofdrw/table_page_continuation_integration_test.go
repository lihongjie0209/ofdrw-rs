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

func TestTablePageContinuation(t *testing.T) {
	font, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-serif.ttf"))
	if err != nil {
		t.Fatal(err)
	}
	for _, test := range []struct {
		name        string
		merged      bool
		y           float64
		pages       []int
		first, last []string
	}{
		{name: "dense", y: 10, pages: []int{0, 1}, first: []string{"seed", "H", "A"}, last: []string{"H", "B"}},
		{name: "dense_header_moves", y: 12, pages: []int{1}, first: []string{"seed"}, last: []string{"H", "A", "B"}},
		{name: "merged", merged: true, y: 4, pages: []int{0, 1}, first: []string{"seed", "H", "M"}, last: []string{"H", "B"}},
		{name: "merged_header_moves", merged: true, y: 8, pages: []int{1}, first: []string{"seed"}, last: []string{"H", "M", "B"}},
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
			style := TableStyle{PageSize: [2]float64{20, 20}, Margins: [4]float64{2, 2, 2, 2}, ColumnWidths: []float64{16},
				RowHeight: 4, FontSize: 2, LineHeight: 3, Padding: 0.5, BorderWidth: 0.2, FontID: strconv.Itoa(fontID),
				RepeatHeader: true, Start: &FlowStart{PageIndex: 0, Y: test.y}}
			write := func() ([]int, error) {
				if test.merged {
					return doc.AddTableCells(0, 4, []TableCell{
						{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1, Text: "H"},
						{Row: 1, Column: 0, RowSpan: 2, ColumnSpan: 1, Text: "M"},
						{Row: 3, Column: 0, RowSpan: 1, ColumnSpan: 1, Text: "B"}}, style)
				}
				return doc.AddTable(0, [][]string{{"H"}, {"A"}, {"B"}}, style)
			}
			pages, err := write()
			if err != nil || !slices.Equal(pages, test.pages) {
				t.Fatalf("pages: %v, %v", pages, err)
			}
			for page, want := range [][]string{test.first, test.last} {
				text, err := doc.PageText(page)
				if err != nil || !slices.Equal(text, want) {
					t.Fatalf("page %d: %v, %v", page, text, err)
				}
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
				t.Fatalf("invalid request changed source: %v", err)
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
			text, err := reopened.PageText(1)
			if err != nil || !slices.Equal(text, test.last) {
				t.Fatalf("round trip: %v, %v", text, err)
			}
		})
	}
}
