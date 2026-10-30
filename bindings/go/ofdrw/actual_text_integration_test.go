//go:build integration

package ofdrw

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestActualTextNativePDFRoundTrips(t *testing.T) {
	source, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "actual-text-cff.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	for range 3 {
		document, err := FromPDF(source)
		if err != nil {
			t.Fatal(err)
		}
		t.Cleanup(func() { document.Close() })
		text, err := document.PageText(0)
		if err != nil || strings.Join(text, "") != "中文" {
			t.Fatalf("ActualText: %q, %v", text, err)
		}
		issues, err := document.ValidateReferences()
		if err != nil || len(issues) != 0 {
			t.Fatalf("references: %+v, %v", issues, err)
		}
		data, err := document.ToBytes()
		if err != nil {
			t.Fatal(err)
		}
		reopened, err := FromBytes(data)
		if err != nil {
			t.Fatal(err)
		}
		t.Cleanup(func() { reopened.Close() })
		text, err = reopened.PageText(0)
		if err != nil || strings.Join(text, "") != "中文" {
			t.Fatalf("reopened ActualText: %q, %v", text, err)
		}
		source, err = reopened.ToPDF()
		if err != nil {
			t.Fatal(err)
		}
	}
}
