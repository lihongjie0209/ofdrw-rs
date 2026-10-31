//go:build integration

package ofdrw

import (
	"fmt"
	"reflect"
	"sort"
	"strconv"
	"strings"
	"testing"
)

func TestAnnotationClickActionSequences(t *testing.T) {
	region := func(x0, x1 int) string {
		return fmt.Sprintf(`<ofd:Region><ofd:Area Start="%d 3"><ofd:Line Point1="%d 3"/><ofd:Line Point1="%d 7"/><ofd:Line Point1="%d 7"/><ofd:Close/></ofd:Area></ofd:Region>`, x0, x1, x1, x0)
	}
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
			path := annotations[0].SourcePath
			xml, err := document.Entry(path)
			if err != nil {
				t.Fatal(err)
			}
			text := string(xml)
			if overlap {
				text = strings.Replace(text, `<ofd:Action Event="CLICK">`, `<ofd:Action Event="CLICK">`+region(2, 8), 1)
			}
			if !strings.Contains(text, "</ofd:Actions>") {
				t.Fatal("missing native action list")
			}
			area := ""
			if overlap {
				area = region(5, 11)
			}
			text = strings.Replace(text, "</ofd:Actions>", `<ofd:Action Event="CLICK">`+area+`<ofd:URI URI="`+expected[1]+`"/></ofd:Action></ofd:Actions>`, 1)
			if err := document.SetEntry(path, []byte(text)); err != nil {
				t.Fatal(err)
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
