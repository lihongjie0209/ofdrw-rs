//go:build integration

package ofdrw

import (
	"os"
	"path/filepath"
	"testing"
)

func TestPDFCatalogURIBase(t *testing.T) {
	source, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "uri-base-links.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	expected := []string{
		"https://example.org/资料/页.html?字=值#段",
		"https://example.org/docs/manual.pdf?view=full",
		"https://example.org/docs/manual.pdf?lang=en#section",
		"https://other.example/a",
		"mailto:user@example.org",
		"https://example.org/docs/manual.pdf?lang=en",
	}
	check := func(document *Document) {
		t.Helper()
		annotations, err := document.Annotations()
		if err != nil || len(annotations) != len(expected) {
			t.Fatalf("annotations: %v, %v", annotations, err)
		}
		for index, annotation := range annotations {
			if len(annotation.Actions) != 1 || annotation.Actions[0].Attributes["URI"] != expected[index] {
				t.Fatalf("URI %d: %v", index, annotation.Actions)
			}
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
