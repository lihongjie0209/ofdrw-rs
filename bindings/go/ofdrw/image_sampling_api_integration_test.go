//go:build integration

package ofdrw

import (
	"bytes"
	"path/filepath"
	"strings"
	"testing"
)

func TestImageInterpolationAPI(t *testing.T) {
	doc, err := Open(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "image-mask.ofd"))
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	baseline, err := doc.PageSVG(0)
	if err != nil {
		t.Fatal(err)
	}
	for _, tc := range []struct {
		name  string
		value bool
		hint  string
	}{
		{"nearest", false, "optimizeSpeed"}, {"smooth", true, "optimizeQuality"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			if err := doc.SetPageImageInterpolate(0, "5", &tc.value); err != nil {
				t.Fatal(err)
			}
			svg, err := doc.PageSVG(0)
			if err != nil || !strings.Contains(svg, `image-rendering="`+tc.hint+`"`) {
				t.Fatalf("sampling: %v", err)
			}
		})
	}
	if err := doc.SetPageImageInterpolate(0, "5", nil); err != nil {
		t.Fatal(err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || svg != baseline {
		t.Fatalf("reset: %v", err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	nearest := false
	for _, tc := range []struct {
		name string
		page int
		id   string
	}{
		{"negative page", -1, "5"}, {"missing object", 0, "missing"}, {"NUL ID", 0, "5\x00"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			if err := doc.SetPageImageInterpolate(tc.page, tc.id, &nearest); err == nil {
				t.Fatal("expected validation error")
			}
		})
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed edit changed bytes: %v", err)
	}
	if err := doc.SetPageImageInterpolate(0, "5", &nearest); err != nil {
		t.Fatal(err)
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
	svg, err = reopened.PageSVG(0)
	if err != nil || !strings.Contains(svg, `image-rendering="optimizeSpeed"`) {
		t.Fatalf("round trip: %v", err)
	}
	doc.Close()
	if err := doc.SetPageImageInterpolate(0, "5", &nearest); err == nil {
		t.Fatal("expected closed document error")
	}
}
