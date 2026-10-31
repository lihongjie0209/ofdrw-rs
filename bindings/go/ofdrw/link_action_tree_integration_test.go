//go:build integration

package ofdrw

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestPDFLinkActionTree(t *testing.T) {
	source, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "link-action-tree.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	check := func(document *Document) {
		t.Helper()
		annotations, err := document.Annotations()
		if err != nil || len(annotations) != 1 {
			t.Fatalf("annotations: %v, %v", annotations, err)
		}
		actions := annotations[0].Actions
		if len(actions) != 6 {
			t.Fatalf("actions: %v", actions)
		}
		for index, kind := range []string{"URI", "Goto", "URI", "URI", "URI", "URI"} {
			if actions[index].Kind != kind {
				t.Fatalf("action %d: %v", index, actions[index])
			}
			if !reflect.DeepEqual(actions[index].Region, actions[0].Region) {
				t.Fatalf("region %d changed", index)
			}
		}
		for index, suffix := range map[int]string{0: "docs/start", 2: "docs/shared", 3: "last", 4: "docs/tail", 5: "docs/shared"} {
			if actions[index].Attributes["URI"] != "https://example.org/"+suffix {
				t.Fatalf("URI %d: %v", index, actions[index])
			}
		}
		pages, err := document.Pages()
		if err != nil || len(pages) != 2 {
			t.Fatalf("pages: %v, %v", pages, err)
		}
		if actions[1].Destination == nil || actions[1].Destination.PageID != pages[1].ID {
			t.Fatalf("destination: %v", actions[1])
		}
		if actions[0].Region == nil {
			t.Fatal("missing trigger region")
		}
	}
	document, err := FromPDF(source)
	if err != nil {
		t.Fatal(err)
	}
	defer document.Close()
	check(document)
	data, err := document.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	reopened, err := FromBytes(data)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	check(reopened)
	pdf, err := reopened.ToPDF()
	if err != nil {
		t.Fatal(err)
	}
	exported, err := FromPDF(pdf)
	if err != nil {
		t.Fatal(err)
	}
	defer exported.Close()
	check(exported)
}
