//go:build integration

package ofdrw

import (
	"reflect"
	"sort"
	"strconv"
	"testing"
)

func TestAnnotationClickActionSequences(t *testing.T) {
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
			expected := []string{"https://example.org/first", "https://example.org/second"}
			if _, err := document.AddURILinkAnnotation(0, expected[0], [4]float64{2, 3, 10, 5}); err != nil {
				t.Fatal(err)
			}
			annotations, err := document.Annotations()
			if err != nil || len(annotations) != 1 {
				t.Fatalf("annotations: %v, %v", annotations, err)
			}
			annotationID := annotations[0].ID
			objects, err := document.AnnotationObjects(0, annotationID)
			if err != nil || len(objects) == 0 {
				t.Fatalf("objects: %v, %v", objects, err)
			}
			if objects[0].ID == nil {
				t.Fatal("missing annotation object id")
			}
			objectID := *objects[0].ID
			if err := document.AddAnnotationObjectAction(0, annotationID, objectID, DocumentActionSpec{Kind: "uri", URI: "https://example.org/discard"}); err != nil {
				t.Fatal(err)
			}
			if err := document.RemoveAnnotationObjectAction(0, annotationID, objectID, 1); err != nil {
				t.Fatal(err)
			}
			if err := document.AddAnnotationObjectAction(0, annotationID, objectID, DocumentActionSpec{Kind: "uri", URI: expected[1]}); err != nil {
				t.Fatal(err)
			}
			if err := document.SetAnnotationObjectActionRegions(0, annotationID, objectID, 0, [][4]float64{{1, 1, 2, 2}}); err != nil {
				t.Fatal(err)
			}
			if err := document.SetAnnotationObjectActionRegions(0, annotationID, objectID, 0, nil); err != nil {
				t.Fatal(err)
			}
			actions, err := document.AnnotationObjectActions(0, annotationID, objectID)
			if err != nil || len(actions) != 2 || actions[0].Region != nil {
				t.Fatalf("actions: %v, %v", actions, err)
			}
			if overlap {
				if err := document.SetAnnotationObjectActionRegions(0, annotationID, objectID, 0, [][4]float64{{2, 3, 6, 4}}); err != nil {
					t.Fatal(err)
				}
				if err := document.SetAnnotationObjectActionRegions(0, annotationID, objectID, 1, [][4]float64{{5, 3, 6, 4}}); err != nil {
					t.Fatal(err)
				}
			}
			if err := document.AddAnnotationObjectAction(0, annotationID, objectID, DocumentActionSpec{Kind: "uri", Event: "PO", URI: "x"}); err == nil {
				t.Fatal("accepted PO action")
			}
			if err := document.RemoveAnnotationObjectAction(0, annotationID, objectID, 99); err == nil {
				t.Fatal("accepted missing action")
			}
			if err := document.SetAnnotationObjectActionRegions(0, annotationID, objectID, 0, [][4]float64{{0, 0, -1, 2}}); err == nil {
				t.Fatal("accepted invalid region")
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
			annotations, err = imported.Annotations()
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
