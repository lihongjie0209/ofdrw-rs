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

func TestCanvasTextMatchesJavaMetricsAndPixels(t *testing.T) {
	root := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "canvas-text-java")
	data, err := os.ReadFile(filepath.Join(root, "requests.json"))
	if err != nil {
		t.Fatal(err)
	}
	var requests struct {
		Boundary [4]float64 `json:"boundary"`
		Text     string     `json:"text"`
		Cases    []struct {
			Font    CanvasFont        `json:"font"`
			Point   [2]float64        `json:"point"`
			Metrics CanvasTextMetrics `json:"metrics"`
		} `json:"cases"`
	}
	if err := json.Unmarshal(data, &requests); err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	java, err := Open(filepath.Join(root, "directions.ofd"))
	if err != nil {
		t.Fatal(err)
	}
	defer java.Close()
	if err := doc.AddBlankPage(90, 90); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageArea(0, PageArea{
		PhysicalBox: [4]float64{0, 0, 90, 90}, ApplicationBox: &[4]float64{0, 0, 90, 90},
	}); err != nil {
		t.Fatal(err)
	}
	fontData, err := os.ReadFile(filepath.Join(root, "..", "dejavu-sans.ttf"))
	if err != nil {
		t.Fatal(err)
	}
	fontID, err := doc.AddEmbeddedFont("CanvasTest", fontData)
	if err != nil {
		t.Fatal(err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	alpha := 0.5
	commands := []CanvasCommand{
		{Type: "translate", Offset: &[2]float64{1, 1}}, {Type: "set_global_alpha", Alpha: &alpha},
	}
	for _, entry := range requests.Cases {
		font := entry.Font
		font.FontID = strconv.Itoa(fontID)
		metrics, err := doc.MeasureCanvasText(0, requests.Text, font)
		if err != nil {
			t.Fatal(err)
		}
		if !reflect.DeepEqual(metrics, entry.Metrics) {
			t.Fatalf("metrics: %+v, expected %+v", metrics, entry.Metrics)
		}
		point := entry.Point
		commands = append(commands, CanvasCommand{Type: "set_font", Font: &font},
			CanvasCommand{Type: "fill_text", Text: &requests.Text, Point: &point})
	}
	controlData, err := os.ReadFile(filepath.Join(root, "controls.json"))
	if err != nil {
		t.Fatal(err)
	}
	var controls struct {
		Cases []struct {
			Code    int               `json:"code"`
			Metrics CanvasTextMetrics `json:"metrics"`
		} `json:"cases"`
	}
	if err := json.Unmarshal(controlData, &controls); err != nil {
		t.Fatal(err)
	}
	for _, entry := range controls.Cases {
		text := "A" + string(rune(entry.Code)) + "B"
		size := 4.0
		actual, err := doc.MeasureCanvasText(0, text, CanvasFont{FontID: strconv.Itoa(fontID), Size: &size})
		if err != nil {
			t.Fatal(err)
		}
		if !reflect.DeepEqual(actual, entry.Metrics) {
			t.Fatalf("control U+%04X: %+v", entry.Code, actual)
		}
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatal("measurement mutated document")
	}
	snapshot, err := json.Marshal(commands)
	if err != nil {
		t.Fatal(err)
	}
	ids, err := doc.DrawCanvas(0, requests.Boundary, commands)
	if err != nil {
		t.Fatal(err)
	}
	if len(ids) != 16 {
		t.Fatalf("IDs: %v", ids)
	}
	encoded, err := json.Marshal(commands)
	if err != nil || !bytes.Equal(snapshot, encoded) {
		t.Fatal("commands mutated")
	}
	for _, dpi := range []float64{25.4, 101.6} {
		actual, err := doc.PagePNG(0, dpi)
		if err != nil {
			t.Fatal(err)
		}
		expected, err := java.PagePNG(0, dpi)
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Equal(actual, expected) {
			t.Fatalf("pixels differ at %v DPI", dpi)
		}
	}
	encoded, err = doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	reopened, err := FromBytes(encoded)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	issues, err := reopened.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
}

func TestCanvasTextDefaultsErrorsAndEmptyTextDoNotMutateDocument(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	metrics, err := doc.MeasureCanvasText(0, "", CanvasFont{})
	if err != nil || metrics.Width != 0 || len(metrics.Offsets) != 0 {
		t.Fatalf("empty metrics: %+v, %v", metrics, err)
	}
	empty := ""
	if ids, err := doc.DrawCanvas(0, [4]float64{0, 0, 40, 30}, []CanvasCommand{
		{Type: "fill_text", Text: &empty, Point: &[2]float64{}},
	}); err != nil || len(ids) != 0 {
		t.Fatalf("empty paint: %v, %v", ids, err)
	}
	zero := 0.0
	nan := math.NaN()
	invalidWeight := uint16(450)
	invalidDirection := uint16(1)
	text := "text"
	for _, font := range []CanvasFont{
		{Size: &zero}, {LetterSpacing: &nan}, {Weight: &invalidWeight},
		{ReadDirection: &invalidDirection}, {Align: "bad"}, {FontID: "99999"},
	} {
		if _, err := doc.MeasureCanvasText(0, text, font); err == nil {
			t.Fatal("invalid font accepted")
		}
		if _, err := doc.DrawCanvas(0, [4]float64{0, 0, 40, 30}, []CanvasCommand{
			{Type: "fill_text", Text: &text, Point: &[2]float64{1, 5}},
			{Type: "set_font", Font: &font}, {Type: "fill_text", Text: &text, Point: &[2]float64{1, 10}},
		}); err == nil {
			t.Fatal("invalid font batch accepted")
		}
		after, err := doc.ToBytes()
		if err != nil || !bytes.Equal(before, after) {
			t.Fatal("invalid font mutated document")
		}
	}
	if _, err := doc.MeasureCanvasText(-1, text, CanvasFont{}); err == nil {
		t.Fatal("negative page accepted")
	}
	doc.Close()
	if _, err := doc.MeasureCanvasText(0, text, CanvasFont{}); err == nil {
		t.Fatal("closed document accepted")
	}
}
