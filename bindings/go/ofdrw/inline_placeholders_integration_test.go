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

func TestInlinePlaceholdersMatchJavaWithoutFontResources(t *testing.T) {
	fixtureDir := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	data, err := os.ReadFile(filepath.Join(fixtureDir, "inline-placeholders-java.json"))
	if err != nil {
		t.Fatal(err)
	}
	var fixture struct {
		Cases []struct {
			Name         string            `json:"name"`
			ContentWidth float64           `json:"content_width"`
			Style        FlowTextStyle     `json:"style"`
			Spans        []FlowSpan        `json:"spans"`
			Lines        []FlowLineMetrics `json:"lines"`
		} `json:"cases"`
	}
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatal(err)
	}
	for _, variant := range []struct {
		name  string
		shape bool
	}{
		{name: "plain", shape: false}, {name: "shaped", shape: true},
	} {
		t.Run(variant.name, func(t *testing.T) {
			for _, entry := range fixture.Cases {
				t.Run(entry.Name, func(t *testing.T) {
					doc, err := New()
					if err != nil {
						t.Fatal(err)
					}
					defer doc.Close()
					before, err := doc.ToBytes()
					if err != nil {
						t.Fatal(err)
					}
					style := entry.Style
					style.Shape = variant.shape
					metrics, err := doc.MeasureFlowedSpans(
						0,
						entry.Spans,
						entry.ContentWidth,
						style,
					)
					if err != nil {
						t.Fatal(err)
					}
					if !reflect.DeepEqual(metrics.Lines, entry.Lines) {
						t.Fatalf("lines = %+v; want %+v", metrics.Lines, entry.Lines)
					}
					after, err := doc.ToBytes()
					if err != nil {
						t.Fatal(err)
					}
					if !bytes.Equal(before, after) {
						t.Fatal("measurement changed document")
					}
					style.PageSize = [2]float64{entry.ContentWidth + 4, 50}
					style.Margins = [4]float64{2, 2, 2, 2}
					if _, err := doc.AddFlowedSpans(0, entry.Spans, style); err != nil {
						t.Fatal(err)
					}
					count, err := doc.PageCount()
					if err != nil || count != 1 {
						t.Fatalf("pages = %d: %v", count, err)
					}
					objects, err := doc.PageObjects(0)
					if err != nil || len(objects) != 0 {
						t.Fatalf("objects = %v: %v", objects, err)
					}
					text, err := doc.PageText(0)
					if err != nil || len(text) != 0 {
						t.Fatalf("text = %v: %v", text, err)
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
					image, err := reopened.PagePNG(0, 30)
					if err != nil || len(image) == 0 {
						t.Fatalf("render: %v", err)
					}
				})
			}
		})
	}
}

func TestInlinePlaceholderMixedWritersAndAtomicErrors(t *testing.T) {
	for _, variant := range []struct {
		name  string
		shape bool
	}{
		{name: "plain", shape: false}, {name: "shaped", shape: true},
	} {
		t.Run(variant.name, func(t *testing.T) {
			doc, err := New()
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			style := DefaultFlowTextStyle()
			style.FontSize, style.LineHeight, style.Shape = 2, 3, variant.shape
			style.PageSize, style.Margins = [2]float64{30, 20}, [4]float64{2, 2, 2, 2}
			spans := []FlowSpan{NewPlaceholderSpan(3, 5), NewPlaceholderSpan(2, 2)}
			root := FlowContainer{Layout: LayoutBox{Size: [2]float64{10, 0}}, Children: []FlowContainerChild{
				{Type: "paragraph", Spans: spans, Style: &style},
			}}
			if err := doc.AddBlankPage(30, 20); err != nil {
				t.Fatal(err)
			}
			fixed, err := doc.AddFixedContainers(0, []FixedContainer{{Position: [2]float64{2, 2}, Container: root}})
			if err != nil {
				t.Fatal(err)
			}
			if len(fixed) != 1 || fixed[0].ContentBox != [4]float64{2, 2, 10, 6} {
				t.Fatalf("fixed = %+v", fixed)
			}
			boxes, err := doc.AddTextBoxes(0, []TextBox{{Layout: root.Layout, Spans: spans, Style: style}},
				BoxLayoutStyle{PageSize: style.PageSize, Margins: style.Margins})
			if err != nil {
				t.Fatal(err)
			}
			if len(boxes.Placements) != 1 || boxes.Placements[0].ContentBox != [4]float64{2, 2, 10, 6} {
				t.Fatalf("boxes = %+v", boxes)
			}
			before, err := doc.ToBytes()
			if err != nil {
				t.Fatal(err)
			}
			for _, width := range []float64{-1, 27, math.NaN(), math.Inf(1)} {
				bad := NewPlaceholderSpan(width, 2)
				if _, err := doc.MeasureFlowedSpans(
					0,
					[]FlowSpan{bad},
					26,
					style,
				); err == nil {
					t.Fatalf("accepted width %v", width)
				}
				if _, err := doc.AddFlowedSpans(0, []FlowSpan{spans[0], bad}, style); err == nil {
					t.Fatalf("paint accepted width %v", width)
				}
				after, err := doc.ToBytes()
				if err != nil {
					t.Fatal(err)
				}
				if !bytes.Equal(before, after) {
					t.Fatal("invalid input changed document")
				}
			}
			bad := NewPlaceholderSpan(1, 2)
			bad.Text = "hidden"
			if _, err := doc.AddFlowedSpans(0, []FlowSpan{bad}, style); err == nil {
				t.Fatal("accepted text in a placeholder")
			}
		})
	}
}
