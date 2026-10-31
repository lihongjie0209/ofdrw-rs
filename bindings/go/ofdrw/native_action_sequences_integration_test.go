//go:build integration

package ofdrw

import (
	"reflect"
	"sort"
	"strconv"
	"testing"
)

func TestNativeClickActionSequences(t *testing.T) {
	for _, overlap := range []bool{false, true} {
		t.Run(strconv.FormatBool(overlap), func(t *testing.T) {
			document, err := New()
			if err != nil {
				t.Fatal(err)
			}
			defer document.Close()
			if err := document.AddBlankPage(30, 20); err != nil {
				t.Fatal(err)
			}
			id, err := document.AddPathObject(0, "M 0 0 L 10 0 L 10 5 C", [4]float64{2, 3, 10, 5}, PathStyle{Stroke: true})
			if err != nil {
				t.Fatal(err)
			}
			expected := []string{"https://example.org/first", "https://example.org/second"}
			for _, uri := range expected {
				if err := document.AddPageObjectURIAction(0, strconv.Itoa(id), uri); err != nil {
					t.Fatal(err)
				}
			}
			if overlap {
				for index, rect := range [][4]float64{{2, 3, 6, 4}, {5, 3, 6, 4}} {
					if err := document.SetPageObjectActionRegions(0, strconv.Itoa(id), index, [][4]float64{rect}); err != nil {
						t.Fatal(err)
					}
				}
			}
			data, err := document.ToBytes()
			if err != nil {
				t.Fatal(err)
			}
			reopened, err := FromBytes(data)
			if err != nil {
				t.Fatal(err)
			}
			defer reopened.Close()
			pdf, err := reopened.ToPDF()
			if err != nil {
				t.Fatal(err)
			}
			imported, err := FromPDF(pdf)
			if err != nil {
				t.Fatal(err)
			}
			defer imported.Close()
			annotations, err := imported.Annotations()
			if err != nil {
				t.Fatal(err)
			}
			counts := make([]int, 0, len(annotations))
			for _, annotation := range annotations {
				counts = append(counts, len(annotation.Actions))
				targets := make([]string, 0, len(annotation.Actions))
				for _, action := range annotation.Actions {
					targets = append(targets, action.Attributes["URI"])
				}
				if len(targets) == 2 {
					if !reflect.DeepEqual(targets, expected) {
						t.Fatalf("sequence: %v", targets)
					}
				} else if len(targets) != 1 || (targets[0] != expected[0] && targets[0] != expected[1]) {
					t.Fatalf("target: %v", targets)
				}
			}
			sort.Ints(counts)
			want := []int{2}
			if overlap {
				want = []int{1, 1, 2}
			}
			if !reflect.DeepEqual(counts, want) {
				t.Fatalf("action counts: %v, want %v", counts, want)
			}
		})
	}
}
