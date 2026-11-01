//go:build integration

package ofdrw

import (
	"bytes"
	"math"
	"testing"
)

func TestAnnotationActionValidationPreservesDocument(t *testing.T) {
	operations := []struct {
		name string
		call func(*Document, int, string, string) error
	}{
		{"read", func(d *Document, p int, a, o string) error { _, err := d.AnnotationObjectActions(p, a, o); return err }},
		{"add", func(d *Document, p int, a, o string) error {
			return d.AddAnnotationObjectAction(p, a, o, DocumentActionSpec{Kind: "uri", URI: "https://example.org"})
		}},
		{"remove", func(d *Document, p int, a, o string) error { return d.RemoveAnnotationObjectAction(p, a, o, 0) }},
		{"regions", func(d *Document, p int, a, o string) error {
			return d.SetAnnotationObjectActionRegions(p, a, o, 0, nil)
		}},
	}
	for _, operation := range operations {
		t.Run(operation.name, func(t *testing.T) {
			doc, err := New()
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			if err := doc.AddBlankPage(30, 20); err != nil {
				t.Fatal(err)
			}
			before, err := doc.ToBytes()
			if err != nil {
				t.Fatal(err)
			}
			for _, input := range []struct {
				name               string
				page               int
				annotation, object string
			}{
				{"negative page", -1, "missing", "missing"},
				{"annotation contains NUL", 0, "bad\x00id", "missing"},
				{"object contains NUL", 0, "missing", "bad\x00id"},
				{"missing annotation", 0, "missing", "missing"},
			} {
				t.Run(input.name, func(t *testing.T) {
					if err := operation.call(doc, input.page, input.annotation, input.object); err == nil {
						t.Fatal("accepted invalid target")
					}
					after, err := doc.ToBytes()
					if err != nil {
						t.Fatal(err)
					}
					if !bytes.Equal(before, after) {
						t.Fatal("failed operation changed document")
					}
				})
			}
			doc.Close()
			if err := operation.call(doc, 0, "missing", "missing"); err == nil {
				t.Fatal("accepted closed document")
			}
		})
	}
}

func TestAnnotationActionRejectsInvalidIndexAndNonfiniteRegions(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	for _, input := range []struct {
		name string
		call func() error
	}{
		{"remove negative index", func() error { return doc.RemoveAnnotationObjectAction(0, "a", "o", -1) }},
		{"regions negative index", func() error { return doc.SetAnnotationObjectActionRegions(0, "a", "o", -1, nil) }},
		{"NaN region", func() error {
			return doc.SetAnnotationObjectActionRegions(0, "a", "o", 0, [][4]float64{{0, 0, math.NaN(), 1}})
		}},
		{"infinite region", func() error {
			return doc.SetAnnotationObjectActionRegions(0, "a", "o", 0, [][4]float64{{0, 0, 1, math.Inf(1)}})
		}},
	} {
		t.Run(input.name, func(t *testing.T) {
			if err := input.call(); err == nil {
				t.Fatal("accepted invalid action input")
			}
		})
	}
}
