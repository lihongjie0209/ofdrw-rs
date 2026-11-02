package ofdrw

import (
	"bytes"
	"errors"
	"image/png"
	"math"
	"strconv"
	"testing"
)

func assertHairlineCoverage(t *testing.T, doc *Document, expected float64) {
	t.Helper()
	for _, dpi := range []float64{25.4, 101.6, 300} {
		data, err := doc.PagePNG(0, dpi)
		if err != nil {
			t.Fatal(err)
		}
		raster, err := png.Decode(bytes.NewReader(data))
		if err != nil {
			t.Fatal(err)
		}
		x := raster.Bounds().Dx() / 2
		weight := 0.0
		for y := 0; y < raster.Bounds().Dy(); y++ {
			red, _, _, _ := raster.At(x, y).RGBA()
			weight += 1 - float64(red)/65535
		}
		if math.Abs(weight-expected) > 0.08 {
			t.Fatalf(
				"%v DPI: coverage %v, want %v device pixels",
				dpi,
				weight,
				expected,
			)
		}
	}
}

func TestPathWritersPreserveExplicitDeviceHairlines(t *testing.T) {
	box := [4]float64{0, 0, 20, 20}
	commands := "M 2 5 L 18 5"
	style := PathStyle{Stroke: true, Hairline: true}
	writers := map[string]func(*Document) error{
		"page": func(doc *Document) error {
			id, err := doc.AddPathObject(
				0,
				commands,
				box,
				style,
			)
			if err != nil {
				return err
			}
			ctm := [6]float64{1, 0, 0, 3, 0, 0}
			return doc.SetPageObjectCTM(0, strconv.Itoa(id), &ctm)
		},
		"composite": func(doc *Document) error {
			_, err := doc.AddCompositePathObject(
				0,
				commands,
				box,
				style,
			)
			return err
		},
		"annotation": func(doc *Document) error {
			_, err := doc.AddPathAnnotation(
				0,
				commands,
				box,
				style,
			)
			return err
		},
		"multiple paths": func(doc *Document) error {
			paths := []AnnotationPath{{Commands: commands, Boundary: box, PathStyle: style}}
			_, err := doc.AddMultiPathAnnotation(0, box, paths)
			if paths[0].LineWidth != 0 || !paths[0].Hairline {
				return errors.New("mutated path input")
			}
			return err
		},
		"mixed annotation": func(doc *Document) error {
			objects := []AnnotationGraphic{{Kind: "path", Commands: commands, Boundary: box, PathStyle: style}}
			_, err := doc.AddMixedAnnotation(0, box, objects)
			if objects[0].LineWidth != 0 || !objects[0].Hairline {
				return errors.New("mutated graphic input")
			}
			return err
		},
		"appended annotation": func(doc *Document) error {
			id, err := doc.AddPathAnnotation(
				0,
				commands,
				box,
				PathStyle{Stroke: false},
			)
			if err != nil {
				return err
			}
			objects := []AnnotationGraphic{{Kind: "path", Commands: commands, Boundary: box, PathStyle: style}}
			_, err = doc.AppendAnnotationObjects(0, strconv.Itoa(id), objects)
			if objects[0].LineWidth != 0 || !objects[0].Hairline {
				return errors.New("mutated appended input")
			}
			return err
		},
	}
	for name, write := range writers {
		t.Run(name, func(t *testing.T) {
			doc, err := New()
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			if err := doc.AddBlankPage(20, 20); err != nil {
				t.Fatal(err)
			}
			if err := write(doc); err != nil {
				t.Fatal(err)
			}
			assertHairlineCoverage(t, doc, 1)
		})
	}
}

func TestHairlineOptionRetainsDefaultAndExplicitPositiveWidths(t *testing.T) {
	for _, tc := range []struct {
		name  string
		style PathStyle
		width float64
	}{
		{name: "default", style: PathStyle{Stroke: true}, width: 0.4},
		{name: "hairline", style: PathStyle{Stroke: true, Hairline: true}, width: 0},
		{name: "positive", style: PathStyle{Stroke: true, Hairline: true, LineWidth: 2}, width: 2},
	} {
		t.Run(tc.name, func(t *testing.T) {
			doc, err := New()
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			if err := doc.AddBlankPage(20, 20); err != nil {
				t.Fatal(err)
			}
			_, err = doc.AddPathObject(
				0,
				"M 2 5 L 18 5",
				[4]float64{0, 0, 20, 20},
				tc.style,
			)
			if err != nil {
				t.Fatal(err)
			}
			styles, err := doc.PageObjectStrokeStyles(0)
			if err != nil {
				t.Fatal(err)
			}
			if len(styles) != 1 || styles[0].Style.LineWidth != tc.width {
				t.Fatalf("stroke width: %+v, want %v", styles, tc.width)
			}
		})
	}
}
