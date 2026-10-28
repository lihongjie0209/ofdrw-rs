//go:build integration

package ofdrw

import (
	"bytes"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"
)

func TestPageContentIgnoresExtensionNamespaceAcrossBinding(t *testing.T) {
	const entryPath = "Doc_0/Pages/Page_0/Content.xml"
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("Go real", 80, 50); err != nil {
		t.Fatal(err)
	}
	entry, err := doc.Entry(entryPath)
	if err != nil {
		t.Fatal(err)
	}
	page := string(entry)
	page = strings.Replace(page, "<ofd:Page xmlns:ofd=", `<ofd:Page xmlns:ext="urn:example:extension" xmlns:ofd=`, 1)
	page = strings.Replace(page, "<ofd:Content>",
		`<ext:Content><ext:Layer ID="fake"><ext:TextCode>fake</ext:TextCode></ext:Layer></ext:Content>`+
			`<ofd:Content><ext:Layer ID="fake"><ext:TextCode>fake</ext:TextCode></ext:Layer>`, 1)
	page = strings.Replace(page, "<ofd:TextObject ",
		`<ext:TextObject ID="fake" Boundary="0 0 1 1"><ext:TextCode>fake</ext:TextCode></ext:TextObject>`+
			"<ofd:TextObject ", 1)
	page = strings.Replace(page, "<ofd:TextCode ", "<ext:TextCode>fake</ext:TextCode><ofd:TextCode ", 1)
	if err := doc.SetEntry(entryPath, []byte(page)); err != nil {
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
	text, err := reopened.PageText(0)
	if err != nil || !slices.Equal(text, []string{"Go real"}) {
		t.Fatalf("page text: %v, %v", text, err)
	}
	layers, err := reopened.PageLayers(0)
	if err != nil || len(layers) != 1 || layers[0].ObjectCount != 1 {
		t.Fatalf("page layers: %+v, %v", layers, err)
	}
	objects, err := reopened.PageObjects(0)
	if err != nil || len(objects) != 1 {
		t.Fatalf("page objects: %+v, %v", objects, err)
	}
	textObjects, err := reopened.PageTextObjects(0)
	if err != nil || len(textObjects) != 1 || len(textObjects[0].Codes) != 1 {
		t.Fatalf("text objects: %+v, %v", textObjects, err)
	}
	matches, err := reopened.KeywordPositions([]string{"fake"}, nil)
	if err != nil || len(matches) != 0 {
		t.Fatalf("extension text matches: %+v, %v", matches, err)
	}
	svg, err := reopened.PageSVG(0)
	if err != nil || !strings.Contains(svg, "Go real") || strings.Contains(svg, "fake") {
		t.Fatalf("page SVG: %q, %v", svg, err)
	}
}

func TestTextCodeHexEscapesAcrossBinding(t *testing.T) {
	const path = "Doc_0/Pages/Page_0/Content.xml"
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("A B", 100, 100); err != nil {
		t.Fatal(err)
	}
	source, err := doc.Entry(path)
	if err != nil {
		t.Fatal(err)
	}
	before := []byte(">A B</ofd:TextCode>")
	if !bytes.Contains(source, before) {
		t.Fatal("created TextCode does not contain the expected text")
	}
	after := []byte(`>A\0020B</ofd:TextCode>`)
	if err := doc.SetEntry(path, bytes.Replace(source, before, after, 1)); err != nil {
		t.Fatal(err)
	}
	text, err := doc.PageText(0)
	if err != nil || !slices.Equal(text, []string{"A B"}) {
		t.Fatalf("decoded page text: %v, %v", text, err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || len(objects[0].Codes) != 1 || objects[0].Codes[0].Text != "A B" {
		t.Fatalf("decoded TextCode: %+v, %v", objects, err)
	}
	matches, err := doc.KeywordPositions([]string{"A B"}, nil)
	if err != nil || len(matches) != 1 {
		t.Fatalf("decoded keyword: %+v, %v", matches, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, ">A B</text>") {
		t.Fatalf("decoded SVG: %q, %v", svg, err)
	}

	literal, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer literal.Close()
	if err := literal.AddTextPage(`\0041`, 100, 100); err != nil {
		t.Fatal(err)
	}
	text, err = literal.PageText(0)
	if err != nil || !slices.Equal(text, []string{`\0041`}) {
		t.Fatalf("literal backslash round trip: %v, %v", text, err)
	}

	control, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer control.Close()
	if err := control.AddTextPage("A\x01B", 100, 100); err != nil {
		t.Fatal(err)
	}
	text, err = control.PageText(0)
	if err != nil || !slices.Equal(text, []string{"A\x01B"}) {
		t.Fatalf("escaped control extraction: %v, %v", text, err)
	}
	svg, err = control.PageSVG(0)
	if err != nil || !strings.Contains(svg, "A\uFFFDB") {
		t.Fatalf("escaped control SVG: %q, %v", svg, err)
	}

	carriageReturn, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer carriageReturn.Close()
	if err := carriageReturn.AddTextPage("A\rB", 100, 100); err != nil {
		t.Fatal(err)
	}
	encoded, err := carriageReturn.Entry(path)
	if err != nil || !bytes.Contains(encoded, []byte(`A\000DB`)) {
		t.Fatalf("encoded carriage return: %q, %v", encoded, err)
	}
	text, err = carriageReturn.PageText(0)
	if err != nil || !slices.Equal(text, []string{"A\rB"}) {
		t.Fatalf("carriage return round trip: %v, %v", text, err)
	}
}

func TestTextWritersPreserveNulAcrossBinding(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("A\x00B", 100, 100); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("initial text object: %+v, %v", objects, err)
	}
	id := *objects[0].ID
	if err := doc.SetPageTextCode(0, id, 0, "C\x00D"); err != nil {
		t.Fatal(err)
	}
	if err := doc.AppendPageTextCode(0, id, "E\x00F", TextCodeLayout{}); err != nil {
		t.Fatal(err)
	}
	text, err := doc.PageText(0)
	if err != nil || !slices.Equal(text, []string{"C\x00D", "E\x00F"}) {
		t.Fatalf("edited text: %v, %v", text, err)
	}
	if err := doc.AppendTextPage(0, "G\x00H", 100, 100); err != nil {
		t.Fatal(err)
	}
	index, err := doc.InsertTextPage(0, 1, "I\x00J", 100, 100)
	if err != nil || index != 1 {
		t.Fatalf("inserted text page: %d, %v", index, err)
	}
	for _, tc := range []struct {
		name  string
		index int
		want  string
	}{
		{name: "inserted", index: 1, want: "I\x00J"},
		{name: "appended", index: 2, want: "G\x00H"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			text, err := doc.PageText(tc.index)
			if err != nil || !slices.Equal(text, []string{tc.want}) {
				t.Fatalf("page text: %v, %v", text, err)
			}
		})
	}

	fontData, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-sans.ttf"))
	if err != nil {
		t.Fatal(err)
	}
	withFont, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer withFont.Close()
	fontID, err := withFont.AddEmbeddedFont("DejaVu Sans", fontData)
	if err != nil {
		t.Fatal(err)
	}
	if err := withFont.AddTextPageWithFont("K\x00L", fontID, 100, 100); err != nil {
		t.Fatal(err)
	}
	if err := withFont.AppendTextPageWithFont(0, "M\x00N", fontID, 100, 100); err != nil {
		t.Fatal(err)
	}
	text, err = withFont.PageText(0)
	if err != nil || !slices.Equal(text, []string{"K\x00L"}) {
		t.Fatalf("font page text: %v, %v", text, err)
	}
	text, err = withFont.PageText(1)
	if err != nil || !slices.Equal(text, []string{"M\x00N"}) {
		t.Fatalf("appended font page text: %v, %v", text, err)
	}
}
