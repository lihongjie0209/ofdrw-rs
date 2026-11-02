//go:build integration

package ofdrw

import (
	"bytes"
	"encoding/json"
	"math"
	"os"
	"path/filepath"
	"strconv"
	"testing"
)

func TestCanvasFontStyleJavaCases(t *testing.T) {
	root := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	data, err := os.ReadFile(filepath.Join(root, "canvas-font-style-java.json"))
	if err != nil {
		t.Fatal(err)
	}
	var reference struct {
		Cases []struct {
			Input       string  `json:"input"`
			PixelsPerMM float64 `json:"pixels_per_mm"`
			BaseWeight  uint16  `json:"base_weight"`
			BaseItalic  bool    `json:"base_italic"`
			Expected    struct {
				Family string  `json:"family"`
				Size   float64 `json:"size"`
				Weight uint16  `json:"weight"`
				Italic bool    `json:"italic"`
			} `json:"expected"`
		} `json:"cases"`
	}
	if err := json.Unmarshal(data, &reference); err != nil {
		t.Fatal(err)
	}
	if len(reference.Cases) != 986 {
		t.Fatalf("case count: %d", len(reference.Cases))
	}
	fontData, err := os.ReadFile(filepath.Join(root, "dejavu-sans.ttf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	ids := map[string]string{}
	for _, name := range []string{"CanvasTest", "宋体"} {
		id, err := doc.AddEmbeddedFont(name, fontData)
		if err != nil {
			t.Fatal(err)
		}
		ids[name] = strconv.Itoa(id)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	for i, entry := range reference.Cases {
		t.Run(strconv.Itoa(i), func(t *testing.T) {
			spacing := 0.25
			read, char := uint16(90), uint16(270)
			base := CanvasFont{Weight: &entry.BaseWeight, Italic: entry.BaseItalic,
				LetterSpacing: &spacing, ReadDirection: &read, CharDirection: &char, Align: "end"}
			font, err := doc.ResolveCanvasFontStyle(0, entry.Input, base, entry.PixelsPerMM)
			if err != nil {
				t.Fatal(err)
			}
			if font.Size == nil || math.Abs(*font.Size-entry.Expected.Size) >= 1e-12 ||
				font.Weight == nil || *font.Weight != entry.Expected.Weight ||
				font.Italic != entry.Expected.Italic || font.FontID != ids[entry.Expected.Family] ||
				font.LetterSpacing == nil || *font.LetterSpacing != spacing ||
				font.ReadDirection == nil || *font.ReadDirection != read ||
				font.CharDirection == nil || *font.CharDirection != char || font.Align != "end" {
				t.Fatalf("unexpected font for %q: %+v", entry.Input, font)
			}
		})
	}
	after, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(before, after) {
		t.Fatal("resolving existing fonts modified resources")
	}
	for _, style := range []string{"0mm New", "4mm New\x00", "bad"} {
		if _, err := doc.ResolveCanvasFontStyle(0, style, CanvasFont{}, 3.78); err == nil {
			t.Fatalf("accepted invalid style %q", style)
		}
	}
	if _, err := doc.ResolveCanvasFontStyle(0, "4mm New", CanvasFont{}, 0); err == nil {
		t.Fatal("accepted zero pixels per mm")
	}
	after, err = doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(before, after) {
		t.Fatal("invalid style modified resources")
	}
	doc.Close()
	if _, err := doc.ResolveCanvasFontStyle(0, "4mm New", CanvasFont{}, 3.78); err == nil {
		t.Fatal("accepted closed document")
	}
}
