//go:build integration

package ofdrw

import (
	"bytes"
	"os"
	"path/filepath"
	"slices"
	"strconv"
	"testing"
)

func TestArchiveViolationsAcrossBinding(t *testing.T) {
	sample := filepath.Join("..", "..", "..", "..", "ofdrw-archive", "src", "test", "resources", "test-ofda.ofd")
	archived, err := Open(sample)
	if err != nil {
		t.Fatal(err)
	}
	defer archived.Close()
	issues, err := archived.ArchiveViolations()
	if err != nil || len(issues) != 1 || issues[0].Rule != "FONT_SUBSET" || issues[0].Severity != "ERROR" {
		t.Fatalf("Java OFD-A sample: %+v, %v", issues, err)
	}

	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("archive", 100, 100); err != nil {
		t.Fatal(err)
	}
	issues, err = doc.ArchiveViolations()
	if err != nil || len(issues) != 2 || issues[0].Rule != "DOC_TYPE" || issues[0].Severity != "ERROR" || issues[1].Rule != "FONT_SUBSET" {
		t.Fatalf("ordinary OFD: %+v, %v", issues, err)
	}
	source, err := doc.Entry("OFD.xml")
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetEntry("OFD.xml", bytes.Replace(source, []byte(`DocType="OFD"`), []byte(`DocType="OFD-A"`), 1)); err != nil {
		t.Fatal(err)
	}
	issues, err = doc.ArchiveViolations()
	if err != nil || len(issues) != 1 || issues[0].Rule != "FONT_SUBSET" {
		t.Fatalf("changed DocType: %+v, %v", issues, err)
	}
	if _, err := doc.AppendDocumentBody(); err != nil {
		t.Fatal(err)
	}
	issues, err = doc.ArchiveViolations()
	if err != nil || len(issues) != 3 || issues[0].Rule != "FONT_SUBSET" || issues[1].Rule != "FONT_SUBSET" || issues[2].Rule != "SINGLE_DOC" {
		t.Fatalf("multiple bodies: %+v, %v", issues, err)
	}
}

func TestPrepareArchiveAcrossBinding(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("archive", 100, 100); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddAttachment("report.pdf", []byte("%PDF-example")); err != nil {
		t.Fatal(err)
	}
	originalAttachments, err := doc.Attachments()
	if err != nil || len(originalAttachments) != 1 {
		t.Fatalf("original attachment: %+v, %v", originalAttachments, err)
	}
	prepared, err := doc.PrepareArchive()
	if err != nil {
		t.Fatal(err)
	}
	defer prepared.Close()
	issues, err := prepared.ArchiveViolations()
	if err != nil || len(issues) != 1 || issues[0].Rule != "FONT_SUBSET" {
		t.Fatalf("prepared archive: %+v, %v", issues, err)
	}
	attachments, err := prepared.Attachments()
	if err != nil || len(attachments) != 1 || attachments[0].Format == nil || *attachments[0].Format != "XML" {
		t.Fatalf("prepared attachment: %+v, %v", attachments, err)
	}
	summary, err := prepared.AttachmentData(0)
	if err != nil || !bytes.Contains(summary, []byte("<SHA256>")) {
		t.Fatalf("prepared attachment summary: %s, %v", summary, err)
	}
	entries, err := prepared.Entries()
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		if entry == originalAttachments[0].Path {
			t.Fatal("obsolete attachment payload remains in prepared archive")
		}
	}
	sourceXML, err := doc.Entry("OFD.xml")
	if err != nil {
		t.Fatal(err)
	}
	preparedXML, err := prepared.Entry("OFD.xml")
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(sourceXML, []byte(`DocType="OFD"`)) || !bytes.Contains(preparedXML, []byte(`DocType="OFD-A"`)) {
		t.Fatal("preparation changed source or failed to set OFD-A DocType")
	}
}

func TestPrepareArchiveMovesSingleGlyphHorizontalCTMAcrossBinding(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("A", 100, 100); err != nil {
		t.Fatal(err)
	}
	const page = "Doc_0/Pages/Page_0/Content.xml"
	data, err := doc.Entry(page)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetEntry(page, bytes.Replace(data, []byte(` Size="5"`), []byte(` Size="5" CTM="2 0 0 1 3 4"`), 1)); err != nil {
		t.Fatal(err)
	}
	before, err := doc.PagePNG(0, 72)
	if err != nil {
		t.Fatal(err)
	}
	prepared, err := doc.PrepareArchive()
	if err != nil {
		t.Fatal(err)
	}
	defer prepared.Close()
	updated, err := prepared.Entry(page)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(updated, []byte(`HScale="2"`)) || !bytes.Contains(updated, []byte(`CTM="1 0 0 1 3 4"`)) {
		t.Fatalf("horizontal text scale was not normalized: %s", updated)
	}
	after, err := prepared.PagePNG(0, 72)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("page rendering changed after archival preparation: %v", err)
	}
}

func TestPrepareArchiveRetainsShapedLigatureAcrossBinding(t *testing.T) {
	font, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-serif.ttf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	fontID, err := doc.AddEmbeddedFont("DejaVu Serif", font)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBlankPage(50, 30); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddShapedTextObject(0, "AfiB", [4]float64{5, 5, 30, 10}, 6, strconv.Itoa(fontID)); err != nil {
		t.Fatal(err)
	}
	const page = "Doc_0/Pages/Page_0/Content.xml"
	data, err := doc.Entry(page)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetEntry(page, bytes.Replace(data, []byte(` Size="6"`), []byte(` Size="6" CTM="2 0 0 1 3 4"`), 1)); err != nil {
		t.Fatal(err)
	}
	before, err := doc.PagePNG(0, 144)
	if err != nil {
		t.Fatal(err)
	}
	prepared, err := doc.PrepareArchive()
	if err != nil {
		t.Fatal(err)
	}
	defer prepared.Close()
	updated, err := prepared.Entry(page)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(updated, []byte(`HScale="2"`)) || !bytes.Contains(updated, []byte("CGTransform")) {
		t.Fatalf("shaped text was not normalized: %s", updated)
	}
	after, err := prepared.PagePNG(0, 144)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("shaped text rendering changed: %v", err)
	}
}

func TestPrepareArchiveFlattensPlainDeepPageBlocksAcrossBinding(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("nested", 100, 100); err != nil {
		t.Fatal(err)
	}
	const path = "Doc_0/Pages/Page_0/Content.xml"
	page, err := doc.Entry(path)
	if err != nil {
		t.Fatal(err)
	}
	layer := bytes.Index(page, []byte("<ofd:Layer"))
	if layer < 0 {
		t.Fatal("page has no layer")
	}
	openingEnd := layer + bytes.IndexByte(page[layer:], '>') + 1
	page = slices.Concat(page[:openingEnd], bytes.Repeat([]byte("<ofd:PageBlock>"), 4), page[openingEnd:])
	page = bytes.Replace(page, []byte("</ofd:Layer>"), slices.Concat(bytes.Repeat([]byte("</ofd:PageBlock>"), 4), []byte("</ofd:Layer>")), 1)
	if err := doc.SetEntry(path, page); err != nil {
		t.Fatal(err)
	}
	before, err := doc.PageSVG(0)
	if err != nil {
		t.Fatal(err)
	}
	issues, err := doc.ArchiveViolations()
	if err != nil {
		t.Fatal(err)
	}
	deep := false
	for _, issue := range issues {
		deep = deep || issue.Rule == "PAGEBLOCK_DEPTH"
	}
	if !deep {
		t.Fatalf("source should report deep blocks: %+v", issues)
	}
	prepared, err := doc.PrepareArchive()
	if err != nil {
		t.Fatal(err)
	}
	defer prepared.Close()
	preparedPage, err := prepared.Entry(path)
	if err != nil || bytes.Count(preparedPage, []byte("<ofd:PageBlock>")) != 3 {
		t.Fatalf("prepared page: %s, %v", preparedPage, err)
	}
	after, err := prepared.PageSVG(0)
	if err != nil || after != before {
		t.Fatalf("page SVG changed: %v", err)
	}
	issues, err = prepared.ArchiveViolations()
	if err != nil {
		t.Fatal(err)
	}
	for _, issue := range issues {
		if issue.Rule == "PAGEBLOCK_DEPTH" {
			t.Fatalf("deep blocks remain: %+v", issues)
		}
	}
	sourcePage, err := doc.Entry(path)
	if err != nil || !bytes.Equal(sourcePage, page) {
		t.Fatalf("source page changed: %v", err)
	}
}

func TestPrepareArchiveFlattensDeepTemplateBlocksAcrossBinding(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("template", 100, 100); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.PromotePageToTemplate(0, nil, "Background"); err != nil {
		t.Fatal(err)
	}
	sources, err := doc.PageSources(0)
	if err != nil || len(sources) < 2 {
		t.Fatalf("page sources: %v, %v", sources, err)
	}
	path := sources[0].Path
	template, err := doc.Entry(path)
	if err != nil {
		t.Fatal(err)
	}
	layer := bytes.Index(template, []byte("<ofd:Layer"))
	if layer < 0 {
		t.Fatal("template has no layer")
	}
	openingEnd := layer + bytes.IndexByte(template[layer:], '>') + 1
	template = slices.Concat(template[:openingEnd], bytes.Repeat([]byte("<ofd:PageBlock>"), 4), template[openingEnd:])
	template = bytes.Replace(template, []byte("</ofd:Layer>"), slices.Concat(bytes.Repeat([]byte("</ofd:PageBlock>"), 4), []byte("</ofd:Layer>")), 1)
	if err := doc.SetEntry(path, template); err != nil {
		t.Fatal(err)
	}
	issues, err := doc.ArchiveViolations()
	if err != nil {
		t.Fatal(err)
	}
	if !slices.ContainsFunc(issues, func(issue ArchiveViolation) bool {
		return issue.Rule == "PAGEBLOCK_DEPTH" && issue.Path != nil && *issue.Path == path
	}) {
		t.Fatalf("template depth was not reported: %+v", issues)
	}
	prepared, err := doc.PrepareArchive()
	if err != nil {
		t.Fatal(err)
	}
	defer prepared.Close()
	updated, err := prepared.Entry(path)
	if err != nil || bytes.Count(updated, []byte("<ofd:PageBlock>")) != 3 {
		t.Fatalf("prepared template: %s, %v", updated, err)
	}
}

func TestPrepareArchiveFlattensAnnotationAppearanceBlocksAcrossBinding(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("annotation", 100, 100); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddHighlightAnnotation(0, [4]float64{1, 2, 30, 4}, [3]uint8{255, 255, 0}, 128); err != nil {
		t.Fatal(err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 {
		t.Fatalf("annotations: %+v, %v", annotations, err)
	}
	path := annotations[0].SourcePath
	annotation, err := doc.Entry(path)
	if err != nil {
		t.Fatal(err)
	}
	opening := bytes.Index(annotation, []byte("<ofd:Appearance"))
	if opening < 0 {
		t.Fatal("annotation has no appearance")
	}
	openingEnd := opening + bytes.IndexByte(annotation[opening:], '>') + 1
	annotation = slices.Concat(annotation[:openingEnd], []byte("<ofd:PageBlock>"), annotation[openingEnd:])
	annotation = bytes.Replace(annotation, []byte("</ofd:Appearance>"), []byte("</ofd:PageBlock></ofd:Appearance>"), 1)
	if err := doc.SetEntry(path, annotation); err != nil {
		t.Fatal(err)
	}
	before, err := doc.PageSVG(0)
	if err != nil {
		t.Fatal(err)
	}
	prepared, err := doc.PrepareArchive()
	if err != nil {
		t.Fatal(err)
	}
	defer prepared.Close()
	preparedAnnotation, err := prepared.Entry(path)
	if err != nil || bytes.Contains(preparedAnnotation, []byte("PageBlock")) {
		t.Fatalf("prepared annotation: %s, %v", preparedAnnotation, err)
	}
	after, err := prepared.PageSVG(0)
	if err != nil || before != after {
		t.Fatalf("annotation appearance changed: %v", err)
	}
	issues, err := prepared.ArchiveViolations()
	if err != nil {
		t.Fatal(err)
	}
	for _, issue := range issues {
		if issue.Rule == "ANNOTATION" {
			t.Fatalf("annotation violation remains: %+v", issues)
		}
	}
	original, err := doc.Entry(path)
	if err != nil || !bytes.Equal(original, annotation) {
		t.Fatalf("source annotation changed: %v", err)
	}
}

func TestPrepareArchiveCreatesPublicResourcesAcrossBinding(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("page font", 100, 100); err != nil {
		t.Fatal(err)
	}
	publicPath := "Doc_0/PublicRes.xml"
	public, err := doc.Entry(publicPath)
	if err != nil {
		t.Fatal(err)
	}
	start := bytes.Index(public, []byte("<ofd:Fonts>"))
	end := bytes.Index(public, []byte("</ofd:Fonts>"))
	if start < 0 || end < start {
		t.Fatal("generated public resource has no Fonts container")
	}
	end += len("</ofd:Fonts>")
	fonts := append([]byte(nil), public[start:end]...)
	if err := doc.SetEntry(publicPath, bytes.Replace(public, fonts, nil, 1)); err != nil {
		t.Fatal(err)
	}
	pageResource := append([]byte(`<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="Res">`), fonts...)
	pageResource = append(pageResource, []byte("</ofd:Res>")...)
	if err := doc.SetEntry("Doc_0/PageRes.xml", pageResource); err != nil {
		t.Fatal(err)
	}
	pagePath := "Doc_0/Pages/Page_0/Content.xml"
	page, err := doc.Entry(pagePath)
	if err != nil {
		t.Fatal(err)
	}
	page = bytes.Replace(page, []byte("</ofd:Page>"), []byte("<ofd:PageRes>../../PageRes.xml</ofd:PageRes></ofd:Page>"), 1)
	if err := doc.SetEntry(pagePath, page); err != nil {
		t.Fatal(err)
	}
	rootPath := "Doc_0/Document.xml"
	root, err := doc.Entry(rootPath)
	if err != nil {
		t.Fatal(err)
	}
	link := []byte("<ofd:PublicRes>PublicRes.xml</ofd:PublicRes>")
	if !bytes.Contains(root, link) {
		t.Fatal("generated document has no PublicRes link")
	}
	if err := doc.SetEntry(rootPath, bytes.Replace(root, link, nil, 1)); err != nil {
		t.Fatal(err)
	}
	before, err := doc.PageSVG(0)
	if err != nil {
		t.Fatal(err)
	}
	fontsBefore, err := doc.PageFonts(0)
	if err != nil || len(fontsBefore) == 0 || fontsBefore[0].ResourcePath != "Doc_0/PageRes.xml" {
		t.Fatalf("page font before preparation: %+v, %v", fontsBefore, err)
	}

	prepared, err := doc.PrepareArchive()
	if err != nil {
		t.Fatal(err)
	}
	defer prepared.Close()
	newPath := "Doc_0/PublicRes_Archive.xml"
	moved, err := prepared.Entry(newPath)
	if err != nil || !bytes.Contains(moved, []byte("<ofd:Fonts>")) {
		t.Fatalf("new public resource: %s, %v", moved, err)
	}
	pageResource, err = prepared.Entry("Doc_0/PageRes.xml")
	if err != nil || bytes.Contains(pageResource, []byte("<ofd:Fonts>")) {
		t.Fatalf("old page resource: %s, %v", pageResource, err)
	}
	root, err = prepared.Entry(rootPath)
	if err != nil || !bytes.Contains(root, []byte("<ofd:PublicRes>PublicRes_Archive.xml</ofd:PublicRes>")) {
		t.Fatalf("prepared root: %s, %v", root, err)
	}
	fontsAfter, err := prepared.PageFonts(0)
	if err != nil || len(fontsAfter) == 0 || fontsAfter[0].ResourcePath != newPath {
		t.Fatalf("page font after preparation: %+v, %v", fontsAfter, err)
	}
	after, err := prepared.PageSVG(0)
	if err != nil || after != before {
		t.Fatalf("page SVG changed: %v", err)
	}
	referenceIssues, err := prepared.ValidateReferences()
	if err != nil || len(referenceIssues) != 0 {
		t.Fatalf("references: %+v, %v", referenceIssues, err)
	}
	archiveIssues, err := prepared.ArchiveViolations()
	if err != nil {
		t.Fatal(err)
	}
	for _, issue := range archiveIssues {
		if issue.Rule == "RESOURCE_PLACEMENT" {
			t.Fatalf("resource placement remains: %+v", issue)
		}
	}
}
