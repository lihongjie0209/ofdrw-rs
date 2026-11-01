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

func mixedFlowFixture(t *testing.T) ([]FlowBlock, FlowDocumentStyle, FlowLayoutResult) {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "flow-blocks.json"))
	if err != nil {
		t.Fatal(err)
	}
	var fixture struct {
		Request struct {
			Blocks []FlowBlock `json:"blocks"`
			FlowDocumentStyle
		} `json:"request"`
		Expected FlowLayoutResult `json:"expected"`
	}
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatal(err)
	}
	return fixture.Request.Blocks, fixture.Request.FlowDocumentStyle, fixture.Expected
}

func TestMixedFlowComposesBlocksAndReturnsReusableCursor(t *testing.T) {
	for _, input := range []struct {
		name  string
		shape bool
	}{{"plain", false}, {"shaped", true}} {
		t.Run(input.name, func(t *testing.T) {
			blocks, style, expected := mixedFlowFixture(t)
			doc, err := New()
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			font, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-serif.ttf"))
			if err != nil {
				t.Fatal(err)
			}
			id, err := doc.AddEmbeddedFont("DejaVu Serif", font)
			if err != nil {
				t.Fatal(err)
			}
			style.FontID = strconv.Itoa(id)
			style.Shape = input.shape
			result, err := doc.AddFlowBlocks(0, blocks, style)
			if err != nil {
				t.Fatal(err)
			}
			if !reflect.DeepEqual(result, expected) {
				t.Fatalf("layout: %+v, want %+v", result, expected)
			}
			images, err := doc.PageImages(0)
			if err != nil || len(images) != 1 || !reflect.DeepEqual(images[0].Boundary, []float64{7, 6, 6, 4}) {
				t.Fatalf("images: %+v, %v", images, err)
			}
			style.Start = result.Next
			text := "C"
			continued, err := doc.AddFlowBlocks(0, []FlowBlock{{Type: "paragraph", Text: &text}}, style)
			if err != nil {
				t.Fatal(err)
			}
			if !reflect.DeepEqual(continued, FlowLayoutResult{Pages: []int{1}, Next: &FlowStart{PageIndex: 1, Y: 13}}) {
				t.Fatalf("continuation: %+v", continued)
			}
			data, err := doc.ToBytes()
			if err != nil {
				t.Fatal(err)
			}
			reopened, err := FromBytes(data)
			if err != nil {
				t.Fatal(err)
			}
			defer reopened.Close()
			texts, err := reopened.PageText(1)
			if err != nil || !reflect.DeepEqual(texts, []string{"R2", "B", "C"}) {
				t.Fatalf("text: %v, %v", texts, err)
			}
			refs, err := reopened.ValidateReferences()
			if err != nil || len(refs) != 0 {
				t.Fatalf("references: %v, %v", refs, err)
			}
		})
	}
}

func TestMixedFlowRejectsInvalidContentAtomically(t *testing.T) {
	blocks, style, _ := mixedFlowFixture(t)
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	zero := 0.
	for _, input := range []struct {
		name  string
		block FlowBlock
	}{
		{"invalid paragraph size", FlowBlock{Type: "paragraph", Spans: []FlowSpan{{Text: "x"}}, Style: &FlowParagraphStyle{FontSize: &zero}}},
		{"invalid image", FlowBlock{Type: "image", Data: []byte("bad"), Format: "PNG", Width: 2, Height: 2}},
		{"unbounded spacer", FlowBlock{Type: "spacer", Height: 1e308}},
		{"nonfinite spacer", FlowBlock{Type: "spacer", Height: math.NaN()}},
		{"unknown block", FlowBlock{Type: "missing"}},
	} {
		t.Run(input.name, func(t *testing.T) {
			inputBlocks := append(append([]FlowBlock(nil), blocks...), input.block)
			if _, err := doc.AddFlowBlocks(0, inputBlocks, style); err == nil {
				t.Fatal("accepted invalid content")
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
	if _, err := doc.AddFlowBlocks(-1, blocks, style); err == nil {
		t.Fatal("accepted negative document index")
	}
	result, err := doc.AddFlowBlocks(0, nil, FlowDocumentStyle{})
	if err != nil || len(result.Pages) != 0 || result.Next != nil {
		t.Fatalf("empty layout: %+v, %v", result, err)
	}
	doc.Close()
	if _, err := doc.AddFlowBlocks(0, blocks, style); err == nil {
		t.Fatal("accepted closed document")
	}
}

func TestIntrinsicImageSizingAndFitBounds(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "image-layout.json"))
	if err != nil {
		t.Fatal(err)
	}
	var fixture struct {
		Request struct {
			Blocks []FlowBlock `json:"blocks"`
			FlowDocumentStyle
		} `json:"request"`
		Expected FlowLayoutResult `json:"expected"`
		Boundary []float64        `json:"boundary"`
	}
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	result, err := doc.AddFlowBlocks(0, fixture.Request.Blocks, fixture.Request.FlowDocumentStyle)
	if err != nil || !reflect.DeepEqual(result, fixture.Expected) {
		t.Fatalf("layout: %+v, %v", result, err)
	}
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 || !reflect.DeepEqual(images[0].Boundary, fixture.Boundary) {
		t.Fatalf("images: %+v, %v", images, err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	for _, input := range []struct {
		name   string
		width  float64
		bounds [2]float64
	}{
		{"conflicting width", 2, [2]float64{4, 4}},
		{"zero bound", 0, [2]float64{0, 4}},
		{"nonfinite bound", 0, [2]float64{math.Inf(1), 4}},
	} {
		t.Run(input.name, func(t *testing.T) {
			block := fixture.Request.Blocks[0]
			block.Width = input.width
			block.FitWithin = &input.bounds
			if _, err := doc.AddFlowBlocks(0, []FlowBlock{block}, fixture.Request.FlowDocumentStyle); err == nil {
				t.Fatal("expected error")
			}
			after, err := doc.ToBytes()
			if err != nil || !bytes.Equal(before, after) {
				t.Fatalf("document changed: %v", err)
			}
		})
	}
}
