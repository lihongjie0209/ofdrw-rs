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

func containerTypography(container *FlowContainer, font string, shape bool) {
	for i := range container.Children {
		child := &container.Children[i]
		switch child.Type {
		case "paragraph":
			child.Style.FontID, child.Style.Shape = font, shape
		case "table":
			child.TableStyle.FontID = font
		case "container":
			containerTypography(child.Container, font, shape)
		}
	}
}

func TestFlowContainersMatchReferenceAndContinue(t *testing.T) {
	for _, input := range []struct {
		name    string
		shape   bool
		dynamic bool
		repeat  bool
	}{
		{"plain", false, false, false},
		{"shaped", true, false, false},
		{"plain dynamic", false, true, false},
		{"shaped dynamic", true, true, false},
		{"repeated headers", false, false, true},
	} {
		t.Run(input.name, func(t *testing.T) {
			fixturePath := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "flow-containers.json")
			data, err := os.ReadFile(fixturePath)
			if err != nil {
				t.Fatal(err)
			}
			var fixture struct {
				Request struct {
					Containers []FlowContainer `json:"containers"`
					BoxLayoutStyle
				} `json:"request"`
				Expected       BoxLayoutResult `json:"expected"`
				Text           [][]string      `json:"text"`
				HeaderExpected BoxLayoutResult `json:"header_expected"`
				HeaderText     [][]string      `json:"header_text"`
				ImageBoundary  []float64       `json:"image_boundary"`
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
			containerTypography(&fixture.Request.Containers[0], strconv.Itoa(id), input.shape)
			if input.dynamic {
				fixture.Request.Containers[0].Children[1].Container.Layout.Clear = "none"
			}
			if input.repeat {
				fixture.Request.Containers[0].Children[3].TableStyle.RepeatHeader = true
			}
			expected := fixture.Expected
			textReference := fixture.Text
			if input.repeat {
				expected, textReference = fixture.HeaderExpected, fixture.HeaderText
			}
			last := expected.Pages[len(expected.Pages)-1]
			before, err := json.Marshal(fixture.Request.Containers)
			if err != nil {
				t.Fatal(err)
			}
			result, err := doc.AddFlowContainers(0, fixture.Request.Containers, fixture.Request.BoxLayoutStyle)
			if err != nil || !reflect.DeepEqual(result, expected) {
				t.Fatalf("layout: %+v, %v", result, err)
			}
			after, err := json.Marshal(fixture.Request.Containers)
			if err != nil || !bytes.Equal(before, after) {
				t.Fatalf("input changed: %v", err)
			}
			for page, expected := range textReference {
				text, err := doc.PageText(page)
				if err != nil || !reflect.DeepEqual(text, expected) {
					t.Fatalf("page %d: %v, %v", page, text, err)
				}
			}
			images, err := doc.PageImages(1)
			if err != nil || len(images) != 1 || !reflect.DeepEqual(images[0].Boundary, fixture.ImageBoundary) {
				t.Fatalf("images: %+v, %v", images, err)
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
			text, err := reopened.PageText(2)
			if err != nil || !reflect.DeepEqual(text, textReference[2]) {
				t.Fatalf("reopened text: %v, %v", text, err)
			}
			extra := FlowContainer{
				Layout: LayoutBox{Size: [2]float64{4, 0}},
				Children: []FlowContainerChild{{
					Type: "paragraph", Spans: []FlowSpan{{Text: "Q"}},
					Style: fixture.Request.Containers[0].Children[0].Style,
				}},
			}
			geometry := fixture.Request.BoxLayoutStyle
			geometry.Start = result.Next
			continued, err := doc.AddFlowContainers(0, []FlowContainer{extra}, geometry)
			if err != nil || !reflect.DeepEqual(continued.Pages, []int{last}) {
				t.Fatalf("continuation: %+v, %v", continued, err)
			}
		})
	}
}

func TestFlowContainersValidationAndDefaults(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	for _, input := range []struct {
		name  string
		child FlowContainerChild
	}{
		{"unknown", FlowContainerChild{Type: "unknown"}},
		{"negative spacer", FlowContainerChild{Type: "spacer", Height: -1}},
		{"nonfinite spacer", FlowContainerChild{Type: "spacer", Height: math.NaN()}},
		{"empty paragraph", FlowContainerChild{Type: "paragraph"}},
	} {
		t.Run(input.name, func(t *testing.T) {
			root := FlowContainer{Layout: LayoutBox{Size: [2]float64{12, 0}}, Children: []FlowContainerChild{input.child}}
			if _, err := doc.AddFlowContainers(0, []FlowContainer{root}, BoxLayoutStyle{}); err == nil {
				t.Fatal("expected error")
			}
			after, err := doc.ToBytes()
			if err != nil || !bytes.Equal(before, after) {
				t.Fatalf("source changed: %v", err)
			}
		})
	}
	cycle := FlowContainer{Layout: LayoutBox{Size: [2]float64{12, 0}}}
	cycle.Children = []FlowContainerChild{{Type: "container", Container: &cycle}}
	if _, err := doc.AddFlowContainers(0, []FlowContainer{cycle}, BoxLayoutStyle{}); err == nil {
		t.Fatal("cycle accepted")
	}
	if _, err := doc.AddFlowContainers(-1, nil, BoxLayoutStyle{}); err == nil {
		t.Fatal("negative index accepted")
	}
	if _, err := doc.AddFlowContainers(0, nil, BoxLayoutStyle{}); err != nil {
		t.Fatal(err)
	}
	zero := FlowContainer{Layout: LayoutBox{Size: [2]float64{4, 0}}, Children: []FlowContainerChild{{Type: "spacer"}}}
	if _, err := doc.AddFlowContainers(0, []FlowContainer{zero}, BoxLayoutStyle{}); err != nil {
		t.Fatal(err)
	}
	doc.Close()
	if _, err := doc.AddFlowContainers(0, nil, BoxLayoutStyle{}); err == nil {
		t.Fatal("closed document accepted")
	}
}
