//go:build integration

package ofdrw

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestContentRangesPreserveFragmentsAndIndependentColumnOffsets(t *testing.T) {
	dir := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	data, err := os.ReadFile(filepath.Join(dir, "box-content-ranges.json"))
	if err != nil {
		t.Fatal(err)
	}
	var fixture struct {
		Cases []struct {
			Name    string `json:"name"`
			Request struct {
				Items         []BoxLayoutItem    `json:"items"`
				ContentRanges []BoxContentRanges `json:"content_ranges"`
				BoxLayoutStyle
			} `json:"request"`
			Expected BoxLayoutResult `json:"expected"`
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
			original, err := json.Marshal(entry.Request)
			if err != nil {
				t.Fatal(err)
			}
			result, err := doc.AddBoxLayoutWithContentRanges(
				0,
				entry.Request.Items,
				entry.Request.ContentRanges,
				entry.Request.BoxLayoutStyle,
			)
			if err != nil {
				t.Fatal(err)
			}
			if !reflect.DeepEqual(result, entry.Expected) {
				t.Fatalf("result = %+v; want %+v", result, entry.Expected)
			}
			after, err := json.Marshal(entry.Request)
			if err != nil {
				t.Fatal(err)
			}
			if !bytes.Equal(original, after) {
				t.Fatal("source request changed")
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
			image, err := reopened.PagePNG(1, 30)
			if err != nil || len(image) == 0 {
				t.Fatalf("render: %v", err)
			}
			for _, policies := range [][]BoxContentRanges{
				{{SourceIndex: -1, Ranges: [][2]float64{}}},
				{{SourceIndex: 0, Ranges: [][2]float64{{0, 11}}}},
				{{SourceIndex: 0, Ranges: [][2]float64{{3, 2}}}},
				{{SourceIndex: 0, Ranges: [][2]float64{{0, 4}, {3, 5}}}},
				{{SourceIndex: 0}, {SourceIndex: 0}},
			} {
				if _, err := doc.AddBoxLayoutWithContentRanges(
					0,
					entry.Request.Items,
					policies,
					entry.Request.BoxLayoutStyle,
				); err == nil {
					t.Fatalf("accepted policies %+v", policies)
				}
				after, err := doc.ToBytes()
				if err != nil {
					t.Fatal(err)
				}
				if !bytes.Equal(encoded, after) {
					t.Fatal("invalid policy changed document")
				}
			}
			if _, err := doc.AddBoxLayoutWithContentRanges(
				-1,
				entry.Request.Items,
				entry.Request.ContentRanges,
				entry.Request.BoxLayoutStyle,
			); err == nil {
				t.Fatal("accepted negative document")
			}
		})
	}
}

func TestMeasuredLinesBecomeContentRangesAndNilRangesRemainEmptyWithoutMutatingInput(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := DefaultFlowTextStyle()
	style.FontSize, style.LineHeight = 2, 3
	metrics, err := doc.MeasureFlowedSpans(
		0,
		[]FlowSpan{{Text: "A\nB\nC\nD"}},
		8,
		style,
	)
	if err != nil {
		t.Fatal(err)
	}
	policy := BoxContentRangesFromText(0, metrics)
	if !reflect.DeepEqual(policy.Ranges, [][2]float64{{0, 3}, {3, 6}, {6, 9}, {9, 12}}) {
		t.Fatalf("ranges = %v", policy.Ranges)
	}
	items := []BoxLayoutItem{{Type: "box", LayoutBox: &LayoutBox{Size: [2]float64{8, metrics.Height}}}}
	pageStyle := BoxLayoutStyle{PageSize: [2]float64{20, 14}, Margins: [4]float64{2, 2, 2, 2}}
	result, err := doc.AddBoxLayoutWithContentRanges(
		0,
		items,
		[]BoxContentRanges{policy},
		pageStyle,
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(result.Placements) != 2 || result.Placements[0].ContentBox[3] != 9 || result.Placements[1].ContentBox[3] != 3 {
		t.Fatalf("placements = %+v", result.Placements)
	}
	empty := []BoxContentRanges{{SourceIndex: 0}}
	if _, err := doc.AddBoxLayoutWithContentRanges(
		0,
		items,
		empty,
		pageStyle,
	); err != nil {
		t.Fatal(err)
	}
	if empty[0].Ranges != nil {
		t.Fatal("normalization changed source ranges")
	}
}
