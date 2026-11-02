//go:build integration

package ofdrw

import (
	"bytes"
	"encoding/json"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"reflect"
	"strconv"
	"testing"
)

func TestCanvasTextAreaMatchesJava(t *testing.T) {
	root := filepath.Join(
		"..",
		"..",
		"..",
		"crates",
		"ofdrw",
		"tests",
		"fixtures",
	)
	data, err := os.ReadFile(filepath.Join(root, "canvas-text-java", "areas.json"))
	if err != nil {
		t.Fatal(err)
	}
	var reference struct {
		Cases []struct {
			Font             string       `json:"font"`
			Text             string       `json:"text"`
			ReadDirection    uint16       `json:"read_direction"`
			CharDirection    uint16       `json:"char_direction"`
			LetterSpacing    float64      `json:"letter_spacing"`
			EffectiveSpacing float64      `json:"effective_letter_spacing"`
			Width            float64      `json:"width"`
			Height           float64      `json:"height"`
			CharAreas        [][4]float64 `json:"char_areas"`
		} `json:"cases"`
	}
	if err := json.Unmarshal(data, &reference); err != nil {
		t.Fatal(err)
	}
	if len(reference.Cases) != 960 {
		t.Fatal("missing Java cases")
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	fonts := map[string]string{}
	for _, name := range []string{"dejavu-sans.ttf", "dejavu-serif.ttf", "kerning-simhei.ttf", "test-name-cff.otf"} {
		content, err := os.ReadFile(filepath.Join(root, name))
		if err != nil {
			t.Fatal(err)
		}
		id, err := doc.AddEmbeddedFont(name, content)
		if err != nil {
			t.Fatal(err)
		}
		fonts[name] = strconv.Itoa(id)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	for index, entry := range reference.Cases {
		t.Run(fmt.Sprintf("%s/%d", entry.Font, index), func(t *testing.T) {
			size := 4.
			font := CanvasFont{FontID: fonts[entry.Font], Size: &size, LetterSpacing: &entry.LetterSpacing,
				ReadDirection: &entry.ReadDirection, CharDirection: &entry.CharDirection}
			actual, err := doc.MeasureCanvasTextArea(0, entry.Text, font)
			if err != nil {
				t.Fatal(err)
			}
			tolerance := 0.
			if entry.Font == "test-name-cff.otf" {
				tolerance = 0.0001
			}
			close := func(a, b, bound float64) {
				t.Helper()
				if math.Abs(a-b) > bound {
					t.Fatalf(
						"area: %g != %g, tolerance %g",
						a,
						b,
						bound,
					)
				}
			}
			count := math.Max(float64(len(actual.CharAreas)), 1)
			close(actual.Width, entry.Width, tolerance*count)
			close(actual.Height, entry.Height, tolerance*count)
			if actual.LetterSpacing != entry.EffectiveSpacing {
				t.Fatal("effective tracking")
			}
			if len(actual.CharAreas) != len(entry.CharAreas) {
				t.Fatal("character count")
			}
			for i, box := range actual.CharAreas {
				for j, value := range box {
					close(value, entry.CharAreas[i][j], tolerance)
				}
			}
		})
	}
	after, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(before, after) {
		t.Fatal("measurement mutated document")
	}
}

func TestCanvasTextAreaValidationAndLifetime(t *testing.T) {
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
	empty, err := doc.MeasureCanvasTextArea(0, "", CanvasFont{})
	if err != nil {
		t.Fatal(err)
	}
	emptyDimensions := empty.Width == 0 && empty.Height == 0
	if !emptyDimensions || empty.CharAreas == nil {
		t.Fatal("empty area")
	}
	area, err := doc.MeasureCanvasTextArea(0, "A😀", CanvasFont{})
	if err != nil {
		t.Fatal(err)
	}
	if len(area.CharAreas) != 2 {
		t.Fatal("supplementary scalar split")
	}
	zero, nan, huge := 0., math.NaN(), 1e308
	for _, font := range []CanvasFont{{Size: &zero}, {LetterSpacing: &nan}, {LetterSpacing: &huge}, {FontID: "999999"}} {
		if _, err := doc.MeasureCanvasTextArea(0, "ABC", font); err == nil {
			t.Fatal("invalid font accepted")
		}
	}
	for _, page := range []int{-1, 1} {
		if _, err := doc.MeasureCanvasTextArea(page, "A", CanvasFont{}); err == nil {
			t.Fatal("invalid page accepted")
		}
	}
	after, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(before, after) {
		t.Fatal("invalid requests mutated document")
	}
	reopened, err := FromBytes(before)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	roundTrip, err := reopened.MeasureCanvasTextArea(0, "A😀", CanvasFont{})
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(area, roundTrip) {
		t.Fatal("area changed after reopening")
	}
	doc.Close()
	if _, err := doc.MeasureCanvasTextArea(0, "", CanvasFont{}); err == nil {
		t.Fatal("closed document accepted")
	}
}
