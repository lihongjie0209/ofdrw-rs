//go:build integration

package ofdrw

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func TestPDFNormalAnnotationAppearance(t *testing.T) {
	for _, name := range []string{"matrix", "widget", "link", "image", "nonprint", "no-view"} {
		t.Run(name, func(t *testing.T) {
			source, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "annotation-ap-"+name+".pdf"))
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(source)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			annotations, err := doc.Annotations()
			if err != nil || len(annotations) != 1 || annotations[0].Remark == nil || *annotations[0].Remark != "custom appearance" {
				t.Fatalf("annotation: %+v, %v", annotations, err)
			}
			if name == "link" && (len(annotations[0].Actions) != 1 || annotations[0].Actions[0].Attributes["URI"] != "https://example.com/appearance") {
				t.Fatal("lost Link action")
			}
			png, err := doc.PagePNG(0, 144)
			if err != nil {
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
			after, err := reopened.PagePNG(0, 144)
			if err != nil || !bytes.Equal(png, after) {
				t.Fatalf("appearance changed: %v", err)
			}
			issues, err := reopened.ValidateReferences()
			if err != nil || len(issues) != 0 {
				t.Fatalf("references: %v, %v", issues, err)
			}
			pdf, err := reopened.ToPDF()
			if err != nil || !bytes.HasPrefix(pdf, []byte("%PDF-")) {
				t.Fatalf("export: %v", err)
			}
			converted, err := FromPDF(pdf)
			if err != nil {
				t.Fatal(err)
			}
			defer converted.Close()
			annotations, err = converted.Annotations()
			if err != nil || len(annotations) != 1 {
				t.Fatalf("exported annotation: %v, %v", annotations, err)
			}
			if name == "nonprint" && annotations[0].Attributes["Print"] != "false" {
				t.Fatal("lost print flag")
			}
			if name == "no-view" && (annotations[0].Attributes["Visible"] != "false" || annotations[0].Attributes["Print"] == "false") {
				t.Fatal("NoView incorrectly hides printing")
			}
		})
	}
}
