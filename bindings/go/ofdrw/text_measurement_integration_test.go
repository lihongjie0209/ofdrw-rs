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

func TestTextMeasurementReferenceAndAutomaticBoxHeight(t *testing.T) {
	for _, input := range []struct {
		name  string
		shape bool
	}{{"plain", false}, {"shaped", true}} {
		t.Run(input.name, func(t *testing.T) {
			path := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
			data, err := os.ReadFile(filepath.Join(path, "text-measurement.json"))
			if err != nil {
				t.Fatal(err)
			}
			var fixture struct {
				Request struct {
					Spans        []FlowSpan `json:"spans"`
					ContentWidth float64    `json:"content_width"`
					FlowTextStyle
				} `json:"request"`
				Expected FlowTextMetrics `json:"expected"`
			}
			if err := json.Unmarshal(data, &fixture); err != nil {
				t.Fatal(err)
			}
			doc, err := New()
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			font, err := os.ReadFile(filepath.Join(path, "dejavu-serif.ttf"))
			if err != nil {
				t.Fatal(err)
			}
			id, err := doc.AddEmbeddedFont("DejaVu Serif", font)
			if err != nil {
				t.Fatal(err)
			}
			style := fixture.Request.FlowTextStyle
			style.FontID = strconv.Itoa(id)
			style.Shape = input.shape
			before, err := doc.ToBytes()
			if err != nil {
				t.Fatal(err)
			}
			metrics, err := doc.MeasureFlowedSpans(0, fixture.Request.Spans, fixture.Request.ContentWidth, style)
			if err != nil {
				t.Fatal(err)
			}
			if !reflect.DeepEqual(metrics, fixture.Expected) {
				t.Fatalf("metrics: %+v, want %+v", metrics, fixture.Expected)
			}
			after, err := doc.ToBytes()
			if err != nil || !bytes.Equal(before, after) {
				t.Fatalf("measurement changed source: %v", err)
			}
			ignored := style
			ignored.PageSize = [2]float64{}
			ignored.Margins = [4]float64{-1, -1, -1, -1}
			ignored.Start = &FlowStart{PageIndex: 999, Y: -1}
			measured, err := doc.MeasureFlowedSpans(0, fixture.Request.Spans, fixture.Request.ContentWidth, ignored)
			if err != nil || !reflect.DeepEqual(measured, metrics) {
				t.Fatalf("geometry affected metrics: %+v, %v", measured, err)
			}
			boxes, err := doc.AddBoxLayout(0, []BoxLayoutItem{{Type: "box", LayoutBox: &LayoutBox{
				Size: [2]float64{20, metrics.Height}, Padding: [4]float64{1, 1, 1, 1}, Border: [4]float64{0.2, 0.2, 0.2, 0.2}, Integrity: true,
			}}}, BoxLayoutStyle{PageSize: [2]float64{30, 80}, Margins: [4]float64{2, 2, 2, 2}})
			if err != nil {
				t.Fatal(err)
			}
			placed := boxes.Placements[0]
			x, y, w, h := placed.ContentBox[0], placed.ContentBox[1], placed.ContentBox[2], placed.ContentBox[3]
			if h != metrics.Height {
				t.Fatalf("box height: %v", h)
			}
			style.PageSize = [2]float64{30, 80}
			style.Margins = [4]float64{y, 30 - x - w, 80 - y - h, x}
			style.Start = &FlowStart{PageIndex: placed.PageIndex, Y: y}
			pages, err := doc.AddFlowedSpans(0, fixture.Request.Spans, style)
			if err != nil || !reflect.DeepEqual(pages, []int{0}) {
				t.Fatalf("rendered pages: %v, %v", pages, err)
			}
			objects, err := doc.PageTextObjects(0)
			if err != nil {
				t.Fatal(err)
			}
			index := 0
			for _, line := range metrics.Lines {
				if line.Text == "" {
					continue
				}
				if index >= len(objects) {
					t.Fatal("missing rendered line")
				}
				boundary := objects[index].Boundary
				if math.Abs(boundary[0]-x-line.X) > 1e-8 || math.Abs(boundary[1]-y-line.Y) > 1e-8 {
					t.Fatalf("line position: %v, %+v", boundary, line)
				}
				index++
			}
			if index != len(objects) {
				t.Fatal("unexpected rendered lines")
			}
		})
	}
}

func TestTextMeasurementErrorsAndDefaultsPreserveTheDocument(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	metrics, err := doc.MeasureFlowedSpans(0, []FlowSpan{{Text: "A\n\n"}}, 20, FlowTextStyle{})
	if err != nil || metrics.Height != 6 || len(metrics.Lines) != 1 {
		t.Fatalf("defaults: %+v, %v", metrics, err)
	}
	for _, input := range []struct {
		name  string
		spans []FlowSpan
		width float64
	}{
		{"zero width", []FlowSpan{{Text: "A"}}, 0}, {"negative width", []FlowSpan{{Text: "A"}}, -1},
		{"NaN width", []FlowSpan{{Text: "A"}}, math.NaN()}, {"empty spans", nil, 20},
		{"missing font", []FlowSpan{{Text: "A", FontID: "missing"}}, 20}, {"only linebreaks", []FlowSpan{{Text: "\n"}}, 20},
	} {
		t.Run(input.name, func(t *testing.T) {
			if _, err := doc.MeasureFlowedSpans(0, input.spans, input.width, FlowTextStyle{}); err == nil {
				t.Fatal("accepted invalid measurement")
			}
		})
	}
	if _, err := doc.MeasureFlowedSpans(-1, []FlowSpan{{Text: "A"}}, 20, FlowTextStyle{}); err == nil {
		t.Fatal("accepted negative document index")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("measurement changed source: %v", err)
	}
	doc.Close()
	if _, err := doc.MeasureFlowedSpans(0, []FlowSpan{{Text: "A"}}, 20, FlowTextStyle{}); err == nil {
		t.Fatal("accepted closed document")
	}
}
