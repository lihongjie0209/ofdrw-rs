//go:build integration

package ofdrw

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"strconv"
	"testing"
)

func TestFixedContainersMatchJavaGeometry(t *testing.T) {
	fixtureDir := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	data, err := os.ReadFile(filepath.Join(fixtureDir, "fixed-containers-java.json"))
	if err != nil {
		t.Fatal(err)
	}
	var fixture struct {
		Cases []struct {
			Name       string         `json:"name"`
			Input      FixedContainer `json:"input"`
			Allocation [4]float64     `json:"allocation"`
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
			if err := doc.AddBlankPage(20, 15); err != nil {
				t.Fatal(err)
			}
			source := []FixedContainer{entry.Input}
			before, err := json.Marshal(source)
			if err != nil {
				t.Fatal(err)
			}
			result, err := doc.AddFixedContainers(0, source)
			if err != nil {
				t.Fatal(err)
			}
			if len(result) != 1 || result[0].Allocation != entry.Allocation {
				t.Fatalf("placements = %+v, want allocation %v", result, entry.Allocation)
			}
			after, err := json.Marshal(source)
			if err != nil {
				t.Fatal(err)
			}
			if !bytes.Equal(before, after) {
				t.Fatal("input changed")
			}
			count, err := doc.PageCount()
			if err != nil || count != 1 {
				t.Fatalf("page count = %d: %v", count, err)
			}
			image, err := doc.PagePNG(0, 30)
			if err != nil || len(image) == 0 {
				t.Fatalf("render: %v", err)
			}
		})
	}
}

func TestFixedMixedContainersOverflowAndRollback(t *testing.T) {
	for _, entry := range []struct {
		name  string
		shape bool
	}{
		{name: "plain", shape: false}, {name: "shaped", shape: true},
	} {
		t.Run(entry.name, func(t *testing.T) {
			fixtureDir := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
			data, err := os.ReadFile(filepath.Join(fixtureDir, "flow-containers.json"))
			if err != nil {
				t.Fatal(err)
			}
			var fixture struct {
				Request struct {
					Containers []FlowContainer `json:"containers"`
				} `json:"request"`
			}
			if err := json.Unmarshal(data, &fixture); err != nil {
				t.Fatal(err)
			}
			doc, err := New()
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			if err := doc.AddBlankPage(20, 15); err != nil {
				t.Fatal(err)
			}
			font, err := os.ReadFile(filepath.Join(fixtureDir, "dejavu-sans.ttf"))
			if err != nil {
				t.Fatal(err)
			}
			id, err := doc.AddEmbeddedFont("DejaVu Sans", font)
			if err != nil {
				t.Fatal(err)
			}
			root := fixture.Request.Containers[0]
			root.Layout.Size[1] = 2
			containerTypography(&root, strconv.Itoa(id), entry.shape)
			source := []FixedContainer{{Position: [2]float64{-1, 2}, Container: root}}
			result, err := doc.AddFixedContainers(0, source)
			if err != nil {
				t.Fatal(err)
			}
			if len(result) != 1 || result[0].ContentBox != [4]float64{-0.25, 2.75, 12, 2} {
				t.Fatalf("placements = %+v", result)
			}
			text, err := doc.PageText(0)
			if err != nil {
				t.Fatal(err)
			}
			if !reflect.DeepEqual(text, []string{"A", "B", "C", "R1", "R2", "R3", "Z"}) {
				t.Fatalf("text = %v", text)
			}
			encoded, err := doc.ToBytes()
			if err != nil {
				t.Fatal(err)
			}
			invalid := FixedContainer{Position: [2]float64{0, 0}, Container: FlowContainer{
				Layout:   LayoutBox{Size: [2]float64{4, 3}},
				Children: []FlowContainerChild{{Type: "image", Data: []byte("bad"), Format: "PNG", Width: 1, Height: 1}},
			}}
			if _, err := doc.AddFixedContainers(0, []FixedContainer{source[0], invalid}); err == nil {
				t.Fatal("accepted invalid later image")
			}
			after, err := doc.ToBytes()
			if err != nil {
				t.Fatal(err)
			}
			if !bytes.Equal(encoded, after) {
				t.Fatal("failed call changed document")
			}
			if _, err := doc.AddFixedContainers(-1, source); err == nil {
				t.Fatal("accepted negative page")
			}
			if _, err := doc.AddFixedContainers(99, source); err == nil {
				t.Fatal("accepted missing page")
			}
			empty, err := doc.AddFixedContainers(0, nil)
			if err != nil || empty == nil || len(empty) != 0 {
				t.Fatalf("empty result = %v: %v", empty, err)
			}
			count, err := doc.PageCount()
			if err != nil || count != 1 {
				t.Fatalf("page count = %d: %v", count, err)
			}
			reopened, err := FromBytes(encoded)
			if err != nil {
				t.Fatal(err)
			}
			defer reopened.Close()
			reopenedText, err := reopened.PageText(0)
			if err != nil || !reflect.DeepEqual(reopenedText, text) {
				t.Fatalf("roundtrip text = %v: %v", reopenedText, err)
			}
			image, err := reopened.PagePNG(0, 30)
			if err != nil || len(image) == 0 {
				t.Fatalf("render: %v", err)
			}
			doc.Close()
			if _, err := doc.AddFixedContainers(0, source); err == nil {
				t.Fatal("accepted closed document")
			}
		})
	}
}
