package ofdrw

import (
	"bytes"
	"encoding/base64"
	"fmt"
	"image/color"
	"image/png"
	"math"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"regexp"
	"runtime"
	"slices"
	"strconv"
	"strings"
	"testing"
	"unicode/utf16"
)

func TestNativeBindingsFitWindowsCallingConvention(t *testing.T) {
	apiType := reflect.TypeOf(nativeAPI{})
	for i := 0; i < apiType.NumField(); i++ {
		field := apiType.Field(i)
		if field.Name == "bufferFree" { // Go helper, never registered with purego.
			continue
		}
		if field.Type.NumIn() > 15 {
			t.Errorf("%s exceeds purego's Windows argument capacity", field.Name)
		}
		for argument := 0; argument < field.Type.NumIn(); argument++ {
			if field.Type.In(argument).Kind() == reflect.Struct {
				t.Errorf("%s passes a struct by value, unsupported on Windows", field.Name)
			}
		}
	}
}

func TestNativeHandlesSurviveGarbageCollection(t *testing.T) {
	documents := make([]*Document, 24)
	for i := range documents {
		doc, err := New()
		if err != nil {
			t.Fatal(err)
		}
		documents[i] = doc
		t.Cleanup(doc.Close)
	}
	for i, doc := range documents {
		runtime.GC()
		text := fmt.Sprintf("document %d", i)
		if err := doc.AddTextPage(text, 40, 30); err != nil {
			t.Fatal(err)
		}
		got, err := doc.PageText(0)
		if err != nil || len(got) != 1 || got[0] != text {
			t.Fatalf("after GC: text=%v, err=%v", got, err)
		}
		doc.Close()
		if _, err := doc.PageCount(); err == nil {
			t.Fatal("closed document still accessible")
		}
	}
}

func TestExplicitNativeLibraryPathDoesNotFallBack(t *testing.T) {
	if os.Getenv("OFDRW_TEST_INVALID_OVERRIDE") == "1" {
		_, err := New()
		if err == nil || !strings.Contains(err.Error(), os.Getenv("OFDRW_LIB_PATH")) {
			t.Fatalf("expected error for explicit missing library, got %v", err)
		}
		return
	}
	missing := filepath.Join(t.TempDir(), "missing-native-library")
	command := exec.Command(os.Args[0], "-test.run=^TestExplicitNativeLibraryPathDoesNotFallBack$")
	command.Env = append(os.Environ(), "OFDRW_TEST_INVALID_OVERRIDE=1", "OFDRW_LIB_PATH="+missing)
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("explicit native library override test failed: %v\n%s", err, output)
	}
}

func TestNegativeIndexesCannotReachNativeLibrary(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("stable", 20, 20); err != nil {
		t.Fatal(err)
	}
	checks := []struct {
		name string
		call func() error
	}{
		{"page text", func() error { _, err := doc.PageText(-1); return err }},
		{"page sources", func() error { _, err := doc.PageSources(-1); return err }},
		{"page fonts", func() error { _, err := doc.PageFonts(-1); return err }},
		{"font data", func() error { _, err := doc.FontData(-1); return err }},
		{"page font data page", func() error { _, err := doc.PageFontData(-1, 0); return err }},
		{"page font data font", func() error { _, err := doc.PageFontData(0, -1); return err }},
		{"add page font", func() error { _, err := doc.AddEmbeddedFontToPage(-1, "bad", nil); return err }},
		{"set font attribute", func() error { return doc.SetFontAttribute(-1, "Bold", nil) }},
		{"set page font attribute page", func() error { return doc.SetPageFontAttribute(-1, 0, "Bold", nil) }},
		{"set page font attribute font", func() error { return doc.SetPageFontAttribute(0, -1, "Bold", nil) }},
		{"replace font data", func() error { return doc.ReplaceFontData(-1, nil) }},
		{"replace page font data page", func() error { return doc.ReplacePageFontData(-1, 0, nil) }},
		{"replace page font data font", func() error { return doc.ReplacePageFontData(0, -1, nil) }},
		{"reorder document", func() error { return doc.ReorderDocumentPages(-1, []int{0}) }},
		{"reorder page", func() error { return doc.ReorderDocumentPages(0, []int{-1}) }},
		{"move source", func() error { return doc.MovePage(-1, 0) }},
		{"move target", func() error { return doc.MovePage(0, -1) }},
		{"duplicate page", func() error { _, err := doc.DuplicatePage(-1); return err }},
		{"duplicate source", func() error { _, err := doc.DuplicatePageAt(-1, 0); return err }},
		{"duplicate position", func() error { _, err := doc.DuplicatePageAt(0, -1); return err }},
		{"remove page", func() error { return doc.RemovePage(-1) }},
		{"append text", func() error { return doc.AppendTextPage(-1, "bad", 20, 20) }},
		{"append blank", func() error { return doc.AppendBlankPage(-1, 20, 20) }},
		{"insert text body", func() error { _, err := doc.InsertTextPage(-1, 0, "bad", 20, 20); return err }},
		{"insert text position", func() error { _, err := doc.InsertTextPage(0, -1, "bad", 20, 20); return err }},
		{"insert blank body", func() error { _, err := doc.InsertBlankPage(-1, 0, 20, 20); return err }},
		{"insert blank position", func() error { _, err := doc.InsertBlankPage(0, -1, 20, 20); return err }},
		{"positioned text", func() error { _, err := doc.AddTextObject(-1, "bad", [4]float64{0, 0, 10, 10}, 4, "1"); return err }},
		{"shaped text", func() error {
			_, err := doc.AddShapedTextObject(-1, "bad", [4]float64{0, 0, 10, 10}, 4, "1")
			return err
		}},
		{"text code", func() error { return doc.SetPageTextCode(-1, "1", 0, "bad") }},
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	for _, check := range checks {
		if err := check.call(); err == nil {
			t.Fatalf("%s accepted negative index", check.name)
		}
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("negative index changed document: %v", err)
	}
}

func TestPackageHeaderRoundTrip(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	info, err := doc.PackageInfo()
	if err != nil || info.Version == nil || *info.Version != "1.2" || info.DocType == nil || *info.DocType != "OFD" {
		t.Fatalf("initial package info: %+v, %v", info, err)
	}
	if err := doc.SetPackageHeader("1.3 & beta", "OFD-A"); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPackageHeader("", "OFD"); err == nil {
		t.Fatal("expected empty version to fail")
	}
	data, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	copy, err := FromBytes(data)
	if err != nil {
		t.Fatal(err)
	}
	defer copy.Close()
	info, err = copy.PackageInfo()
	if err != nil || info.Version == nil || *info.Version != "1.3 & beta" || info.DocType == nil || *info.DocType != "OFD-A" {
		t.Fatalf("roundtrip package info: %+v, %v", info, err)
	}
	if err := copy.SetEntry("OFDEntries.xml", []byte("<DocEntries/>")); err != nil {
		t.Fatal(err)
	}
	protected, err := copy.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if err := copy.SetPackageHeader("1.2", "OFD"); err == nil {
		t.Fatal("expected protected package header rejection")
	}
	if err := copy.SetDocumentInfoField(0, "Title", "edit"); err == nil {
		t.Fatal("expected protected metadata rejection")
	}
	if err := copy.AppendBlankPage(0, 20, 20); err == nil {
		t.Fatal("expected protected page rejection")
	}
	after, err := copy.ToBytes()
	if err != nil || !bytes.Equal(after, protected) {
		t.Fatalf("protected package changed: %v", err)
	}
}

func TestImportsIndirectFormGroupAcrossBinding(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "form-group-indirect.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 1 {
		t.Fatalf("form path: %v, %v", paths, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("form references: %v, %v", issues, err)
	}
}

func TestImportsSixteenBitPDFImageAcrossBinding(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "gray-16-bit.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	pngBytes, err := doc.PageImageData(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := png.Decode(bytes.NewReader(pngBytes))
	if err != nil {
		t.Fatal(err)
	}
	want := [2][2]uint8{{255, 127}, {0, 191}}
	for y, row := range want {
		for x, value := range row {
			gray := color.GrayModel.Convert(decoded.At(x, y)).(color.Gray).Y
			if gray != value {
				t.Fatalf("pixel (%d,%d): got %d, want %d", x, y, gray, value)
			}
		}
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("sixteen-bit image references: %v, %v", issues, err)
	}
}

func TestImportsJBIG2PDFImageAcrossBinding(t *testing.T) {
	for _, name := range []string{"jbig2-mmr.pdf", "jbig2-globals.pdf"} {
		t.Run(name, func(t *testing.T) {
			data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", name))
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			data, err = doc.PageImageData(0, 0)
			if err != nil {
				t.Fatal(err)
			}
			decoded, err := png.Decode(bytes.NewReader(data))
			if err != nil {
				t.Fatal(err)
			}
			var black, white bool
			for y := decoded.Bounds().Min.Y; y < decoded.Bounds().Max.Y; y++ {
				for x := decoded.Bounds().Min.X; x < decoded.Bounds().Max.X; x++ {
					value := color.GrayModel.Convert(decoded.At(x, y)).(color.Gray).Y
					black = black || value == 0
					white = white || value == 255
				}
			}
			if !black || !white {
				t.Fatal("JBIG2 image lost its black or white pixels")
			}
			issues, err := doc.ValidateReferences()
			if err != nil || len(issues) != 0 {
				t.Fatalf("JBIG2 references: %v, %v", issues, err)
			}
		})
	}
}

func TestImportsSampledShadingPDFAcrossBinding(t *testing.T) {
	for _, filename := range []string{
		"shading-sampled-axial.pdf", "shading-sampled-radial.pdf",
		"shading-sampled-cubic-axial.pdf", "shading-sampled-cubic-radial.pdf",
		"shading-nested-stitching-axial.pdf", "shading-nested-stitching-radial.pdf",
		"shading-indirect-functions-axial.pdf", "shading-indirect-functions-radial.pdf",
		"shading-calculator-axial.pdf", "shading-calculator-radial.pdf",
		"shading-function-array-axial.pdf", "shading-function-array-radial.pdf",
	} {
		t.Run(filename, func(t *testing.T) {
			data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", filename))
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			svg, err := doc.PageSVG(0)
			if err != nil {
				t.Fatal(err)
			}
			for _, color := range []string{"#ff0000", "#00ff00", "#0000ff"} {
				if !strings.Contains(svg, color) {
					t.Fatalf("sampled shading lost color %s", color)
				}
			}
			issues, err := doc.ValidateReferences()
			if err != nil || len(issues) != 0 {
				t.Fatalf("sampled shading references: %v, %v", issues, err)
			}
		})
	}
}

func TestImportsGroup4FaxPDFImageAcrossBinding(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "ccitt-g4.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	pngBytes, err := doc.PageImageData(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := png.Decode(bytes.NewReader(pngBytes))
	if err != nil {
		t.Fatal(err)
	}
	if decoded.Bounds().Dx() != 8 || decoded.Bounds().Dy() != 2 {
		t.Fatalf("fax image dimensions: %v", decoded.Bounds())
	}
	for _, check := range []struct {
		x, y int
		want uint8
	}{
		{0, 0, 255}, {7, 0, 0}, {0, 1, 0}, {7, 1, 255},
	} {
		got := color.GrayModel.Convert(decoded.At(check.x, check.y)).(color.Gray).Y
		if got != check.want {
			t.Fatalf("fax pixel (%d,%d): got %d, want %d", check.x, check.y, got, check.want)
		}
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("fax image references: %v, %v", issues, err)
	}
}

func TestImportsEmbeddedPDFFileAcrossBinding(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "embedded-file.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	attachments, err := doc.Attachments()
	if err != nil || len(attachments) != 1 || attachments[0].Name != "报告.txt" {
		t.Fatalf("embedded PDF attachments: %v, %v", attachments, err)
	}
	payload, err := doc.AttachmentData(0)
	if err != nil || !bytes.Equal(payload, []byte("fixture payload")) {
		t.Fatalf("embedded PDF payload: %q, %v", payload, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("embedded PDF references: %v, %v", issues, err)
	}
}

func TestImportsPDFXObjectAssociatedFilesAcrossBinding(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "xobject-associated-files.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 {
		t.Fatalf("XObject image: %v, %v", images, err)
	}
	attachments, err := doc.Attachments()
	if err != nil || len(attachments) != 2 || attachments[0].Name != "form-source.csv" || attachments[1].Name != "image-source.rgb" {
		t.Fatalf("XObject associated files: %v, %v", attachments, err)
	}
	for index, tc := range []struct {
		name string
		data []byte
	}{
		{name: "Form source", data: []byte("a,b\n")},
		{name: "image source", data: []byte{255, 0, 0}},
	} {
		t.Run(tc.name, func(t *testing.T) {
			payload, err := doc.AttachmentData(index)
			if err != nil || !bytes.Equal(payload, tc.data) {
				t.Fatalf("XObject associated file %d: %v, %v", index, payload, err)
			}
		})
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("XObject associated file references: %v, %v", issues, err)
	}
}

func TestImportsPDFPageResourceAssociatedFilesAcrossBinding(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "page-resource-associated-files.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	expected := []struct {
		name string
		data []byte
	}{
		{name: "page-tree.txt", data: []byte("page tree")},
		{name: "resources.txt", data: []byte("resources")},
		{name: "graphics.txt", data: []byte("graphics")},
		{name: "profile.txt", data: []byte("profile")},
		{name: "pattern-list.txt", data: []byte("pattern list")},
		{name: "pattern.txt", data: []byte("pattern")},
		{name: "shading.txt", data: []byte("shading")},
		{name: "properties.txt", data: []byte("properties")},
	}
	attachments, err := doc.Attachments()
	if err != nil || len(attachments) != len(expected) {
		t.Fatalf("page resource associated files: %v, %v", attachments, err)
	}
	for i, want := range expected {
		if attachments[i].Name != want.name {
			t.Fatalf("attachment %d name: %q, want %q", i, attachments[i].Name, want.name)
		}
		payload, err := doc.AttachmentData(i)
		if err != nil || !bytes.Equal(payload, want.data) {
			t.Fatalf("attachment %d payload: %q, %v", i, payload, err)
		}
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("page resource associated file references: %v, %v", issues, err)
	}
	serialized, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	reopened, err := FromBytes(serialized)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	for i, want := range expected {
		payload, err := reopened.AttachmentData(i)
		if err != nil || !bytes.Equal(payload, want.data) {
			t.Fatalf("reopened attachment %d payload: %q, %v", i, payload, err)
		}
	}
}

func TestImportsDefaultGrayJPEG2000AcrossBinding(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "default-gray-jpx.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "#bcbcbc") {
		t.Fatalf("default gray vector color: %q, %v", svg, err)
	}
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 || images[0].Format == nil || *images[0].Format != "PNG" {
		t.Fatalf("default gray JPEG2000 image: %v, %v", images, err)
	}
	payload, err := doc.PageImageData(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := png.Decode(bytes.NewReader(payload))
	if err != nil {
		t.Fatal(err)
	}
	red, green, blue, alpha := decoded.At(4, 16).RGBA()
	if red>>8 != 75 || green != red || blue != red || alpha != 0xffff {
		t.Fatalf("default gray 16-bit source: %d %d %d %d", red, green, blue, alpha)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("default gray references: %v, %v", issues, err)
	}
	serialized, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	reopened, err := FromBytes(serialized)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	copy, err := reopened.PageImageData(0, 0)
	if err != nil || !bytes.Equal(copy, payload) {
		t.Fatalf("reopened JPEG2000 image: %v, %v", copy, err)
	}
}

func TestExportsAndReimportsPDFAttachmentsAcrossBinding(t *testing.T) {
	source, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer source.Close()
	if err := source.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	if err := source.AddAttachment("报告.txt", []byte("unicode payload")); err != nil {
		t.Fatal(err)
	}
	if err := source.AddAttachment("a.txt", []byte("ascii payload")); err != nil {
		t.Fatal(err)
	}
	format := "text/plain"
	created := "2024-02-29T12:30:59"
	modified := "2024-03-01T08:00:00"
	usage := "Data"
	if err := source.SetAttachmentMetadata(0, AttachmentMetadata{
		Format: &format, CreationDate: &created, ModDate: &modified, Usage: &usage,
	}); err != nil {
		t.Fatal(err)
	}
	pdf, err := source.ToPDF()
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.HasPrefix(pdf, []byte("%PDF-2.0")) {
		t.Fatal("associated files require a PDF 2.0 header")
	}
	imported, err := FromPDF(pdf)
	if err != nil {
		t.Fatal(err)
	}
	defer imported.Close()
	attachments, err := imported.Attachments()
	if err != nil || len(attachments) != 2 || attachments[0].Name != "a.txt" || attachments[1].Name != "报告.txt" {
		t.Fatalf("round-trip PDF attachments: %v, %v", attachments, err)
	}
	if attachments[1].Format == nil || *attachments[1].Format != format ||
		attachments[1].CreationDate == nil || *attachments[1].CreationDate != created ||
		attachments[1].ModDate == nil || *attachments[1].ModDate != modified ||
		attachments[1].Usage != usage {
		t.Fatalf("round-trip PDF attachment metadata: %+v", attachments[1])
	}
	for index, expected := range []string{"ascii payload", "unicode payload"} {
		payload, err := imported.AttachmentData(index)
		if err != nil || !bytes.Equal(payload, []byte(expected)) {
			t.Fatalf("round-trip PDF attachment %d: %q, %v", index, payload, err)
		}
	}
}

func TestAllTextPreservesPageOrderAndEmptyPages(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("首页", 100, 100); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBlankPage(100, 100); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddTextPage("last", 100, 100); err != nil {
		t.Fatal(err)
	}
	text, err := doc.AllText()
	if err != nil || !reflect.DeepEqual(text, [][]string{{"首页"}, {}, {"last"}}) {
		t.Fatalf("all text: %v, %v", text, err)
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
	text, err = reopened.AllText()
	if err != nil || !reflect.DeepEqual(text, [][]string{{"首页"}, {}, {"last"}}) {
		t.Fatalf("reopened all text: %v, %v", text, err)
	}
}

func TestImportsDirectOCMDPDFAcrossBinding(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "direct-ocmd.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	text, err := doc.PageText(0)
	if err != nil || !slices.Equal(text, []string{"Shown"}) {
		t.Fatalf("direct OCMD text: %v, %v", text, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("direct OCMD references: %v, %v", issues, err)
	}
}

func TestImportsHairlinePDFTextAcrossBinding(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "hairline-text.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	text, err := doc.PageText(0)
	if err != nil || !slices.Equal(text, []string{"Hairline"}) {
		t.Fatalf("hairline text: %v, %v", text, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "stroke-width=\"0.1\"") {
		t.Fatalf("hairline SVG: %v", err)
	}
	file := filepath.Join(t.TempDir(), "hairline.ofd")
	if err := doc.Save(file); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(file)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	again, err := reopened.PageText(0)
	if err != nil || !slices.Equal(again, text) {
		t.Fatalf("reopened hairline text: %v, %v", again, err)
	}
}

func TestImportsInlinePDFImageAcrossBinding(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "inline-image.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 || images[0].Format == nil || *images[0].Format != "PNG" {
		t.Fatalf("inline images: %v, %v", images, err)
	}
	pngBytes, err := doc.PageImageData(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := png.Decode(bytes.NewReader(pngBytes))
	if err != nil {
		t.Fatal(err)
	}
	if decoded.Bounds().Dx() != 2 || decoded.Bounds().Dy() != 1 {
		t.Fatalf("inline image dimensions: %v", decoded.Bounds())
	}
	red := decoded.At(0, 0)
	blue := decoded.At(1, 0)
	r, _, b, _ := red.RGBA()
	if r != 0xffff || b != 0 {
		t.Fatalf("first inline pixel: %v", red)
	}
	r, _, b, _ = blue.RGBA()
	if r != 0 || b != 0xffff {
		t.Fatalf("second inline pixel: %v", blue)
	}
	file := filepath.Join(t.TempDir(), "inline.ofd")
	if err := doc.Save(file); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(file)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	again, err := reopened.PageImageData(0, 0)
	if err != nil || !bytes.Equal(again, pngBytes) {
		t.Fatalf("inline image roundtrip: %v", err)
	}
}

func TestImportsFilteredInlinePDFImageAcrossBinding(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "filtered-inline-image.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 {
		t.Fatalf("filtered inline images: %v, %v", images, err)
	}
	pngBytes, err := doc.PageImageData(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := png.Decode(bytes.NewReader(pngBytes))
	if err != nil {
		t.Fatal(err)
	}
	if decoded.Bounds().Dx() != 2 || decoded.Bounds().Dy() != 1 {
		t.Fatalf("filtered image dimensions: %v", decoded.Bounds())
	}
	r, g, b, _ := decoded.At(0, 0).RGBA()
	if r != 0xffff || g != 0 || b != 0 {
		t.Fatalf("first filtered pixel: %v", decoded.At(0, 0))
	}
	r, g, b, _ = decoded.At(1, 0).RGBA()
	if r != 0 || g != 0 || b != 0xffff {
		t.Fatalf("second filtered pixel: %v", decoded.At(1, 0))
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("filtered image references: %v, %v", issues, err)
	}
}

func TestImportsAxialShadingPDFAcrossBinding(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "shading-axial.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "<linearGradient") || !strings.Contains(svg, "<clipPath") {
		t.Fatalf("axial shading preview: %v", err)
	}
	preview, err := doc.PagePNG(0, 72)
	if err != nil || !bytes.HasPrefix(preview, []byte("\x89PNG")) {
		t.Fatalf("axial shading PNG: %v", err)
	}
	file := filepath.Join(t.TempDir(), "shading.ofd")
	if err := doc.Save(file); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(file)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	back, err := reopened.PageSVG(0)
	if err != nil || back != svg {
		t.Fatalf("axial shading roundtrip: %v", err)
	}
}

func TestImportsRadialShadingPDFAcrossBinding(t *testing.T) {
	for _, name := range []string{"shading-radial.pdf", "shading-radial-reverse.pdf"} {
		t.Run(name, func(t *testing.T) {
			fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", name)
			data, err := os.ReadFile(fixture)
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			svg, err := doc.PageSVG(0)
			if err != nil || !strings.Contains(svg, "<radialGradient") {
				t.Fatalf("radial shading preview: %v", err)
			}
			png, err := doc.PagePNG(0, 72)
			if err != nil || !bytes.HasPrefix(png, []byte("\x89PNG")) {
				t.Fatalf("radial shading PNG: %v", err)
			}
		})
	}
}

func TestImportsStitchedShadingPDFAcrossBinding(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "shading-stitching.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil || strings.Count(svg, `offset="0.5"`) < 2 {
		t.Fatalf("stitched shading preview: %v", err)
	}
	file := filepath.Join(t.TempDir(), "stitched.ofd")
	if err := doc.Save(file); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(file)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	back, err := reopened.PageSVG(0)
	if err != nil || back != svg {
		t.Fatalf("stitched shading roundtrip: %v", err)
	}
}

func TestImportsPDFShadingExtendModes(t *testing.T) {
	for _, test := range []struct {
		name   string
		extend string
	}{
		{"none", "0"}, {"start", "1"}, {"end", "2"}, {"both", "3"},
	} {
		t.Run(test.name, func(t *testing.T) {
			fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "shading-extend-"+test.name+".pdf")
			data, err := os.ReadFile(fixture)
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			pages, err := doc.Pages()
			if err != nil || len(pages) != 1 {
				t.Fatalf("pages: %v", err)
			}
			xml, err := doc.Entry(pages[0].Path)
			if err != nil || !bytes.Contains(xml, []byte(`Extend="`+test.extend+`"`)) {
				t.Fatalf("shading extend: %v", err)
			}
			svg, err := doc.PageSVG(0)
			if err != nil || !strings.Contains(svg, "<linearGradient") {
				t.Fatalf("shading preview: %v", err)
			}
		})
	}
}

func TestImportsNondefaultPDFShadingDomain(t *testing.T) {
	for _, name := range []string{"shading-domain.pdf", "shading-stitching-domain.pdf"} {
		t.Run(name, func(t *testing.T) {
			fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", name)
			data, err := os.ReadFile(fixture)
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			svg, err := doc.PageSVG(0)
			if err != nil || !strings.Contains(svg, "<linearGradient") {
				t.Fatalf("domain shading preview: %v", err)
			}
			file := filepath.Join(t.TempDir(), "domain.ofd")
			if err := doc.Save(file); err != nil {
				t.Fatal(err)
			}
			reopened, err := Open(file)
			if err != nil {
				t.Fatal(err)
			}
			defer reopened.Close()
			back, err := reopened.PageSVG(0)
			if err != nil || back != svg {
				t.Fatalf("domain shading roundtrip: %v", err)
			}
		})
	}
}

func TestImportsPDFShadingBBoxWithExistingClip(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "shading-bbox.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil || strings.Count(svg, "<clipPath ") < 2 {
		t.Fatalf("shading BBox clip: %v", err)
	}
	preview, err := doc.PagePNG(0, 72)
	if err != nil {
		t.Fatal(err)
	}
	bitmap, err := png.Decode(bytes.NewReader(preview))
	if err != nil {
		t.Fatal(err)
	}
	inside := bitmap.At(36, 36)
	outside := bitmap.At(20, 36)
	ir, ig, ib, _ := inside.RGBA()
	or, og, ob, _ := outside.RGBA()
	if ir == 0xffff && ig == 0xffff && ib == 0xffff {
		t.Fatal("shading BBox intersection was not painted")
	}
	if or != 0xffff || og != 0xffff || ob != 0xffff {
		t.Fatal("shading BBox leaked outside current clip")
	}
}

func TestImportsPDFShadingFunctionRange(t *testing.T) {
	for _, name := range []string{"shading-range.pdf", "shading-stitching-range.pdf", "shading-cmyk-range.pdf"} {
		t.Run(name, func(t *testing.T) {
			fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", name)
			data, err := os.ReadFile(fixture)
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			svg, err := doc.PageSVG(0)
			if err != nil || strings.Count(svg, "<stop offset=") <= 2 {
				t.Fatalf("function Range preview: %v", err)
			}
			file := filepath.Join(t.TempDir(), "range.ofd")
			if err := doc.Save(file); err != nil {
				t.Fatal(err)
			}
			reopened, err := Open(file)
			if err != nil {
				t.Fatal(err)
			}
			defer reopened.Close()
			back, err := reopened.PageSVG(0)
			if err != nil || back != svg {
				t.Fatalf("function Range roundtrip: %v", err)
			}
		})
	}
}

func TestImportsToUnicodePDF(t *testing.T) {
	for _, name := range []string{"type1-tounicode.pdf", "type1-subset-tounicode.pdf", "rotated-tounicode.pdf", "metadata-tounicode.pdf", "outline-tounicode.pdf", "type0-identity-tounicode.pdf", "type0-indirect-widths.pdf", "encoding-differences.pdf"} {
		t.Run(name, func(t *testing.T) {
			testImportToUnicodePDF(t, name)
		})
	}
}

func TestImportsMacRomanTrueTypePDFWithEmbeddedFont(t *testing.T) {
	fixtures := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	data, err := os.ReadFile(filepath.Join(fixtures, "truetype-macroman.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	expectedFont, err := os.ReadFile(filepath.Join(fixtures, "dejavu-sans.ttf"))
	if err != nil {
		t.Fatal(err)
	}
	document, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer document.Close()
	text, err := document.PageText(0)
	if err != nil || !slices.Equal(text, []string{"é"}) {
		t.Fatalf("MacRoman PDF text: %v, %v", text, err)
	}
	fonts, err := document.Fonts()
	if err != nil {
		t.Fatal(err)
	}
	embeddedIndex := -1
	for index, font := range fonts {
		if font.EmbeddedPath != nil {
			if embeddedIndex >= 0 || font.FontName != "DejaVu Sans" {
				t.Fatalf("unexpected embedded fonts: %+v", fonts)
			}
			embeddedIndex = index
		}
	}
	if embeddedIndex < 0 {
		t.Fatalf("missing embedded font: %+v", fonts)
	}
	fontData, err := document.FontData(embeddedIndex)
	if err != nil || !bytes.Equal(fontData, expectedFont) {
		t.Fatalf("imported font differs: %v", err)
	}
	output := filepath.Join(t.TempDir(), "macroman.ofd")
	if err := document.Save(output); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(output)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	text, err = reopened.PageText(0)
	if err != nil || !slices.Equal(text, []string{"é"}) {
		t.Fatalf("reopened MacRoman text: %v, %v", text, err)
	}
	fontData, err = reopened.FontData(embeddedIndex)
	if err != nil || !bytes.Equal(fontData, expectedFont) {
		t.Fatalf("reopened font differs: %v", err)
	}
}

func TestImportsPDFOptionalContentVisibility(t *testing.T) {
	for _, name := range []string{"optional-content.pdf", "optional-content-indirect.pdf"} {
		t.Run(name, func(t *testing.T) {
			fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", name)
			data, err := os.ReadFile(fixture)
			if err != nil {
				t.Fatal(err)
			}
			document, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer document.Close()
			text, err := document.PageText(0)
			if err != nil || !slices.Equal(text, []string{"B"}) {
				t.Fatalf("optional PDF text: %v, %v", text, err)
			}
			output := filepath.Join(t.TempDir(), "optional.ofd")
			if err := document.Save(output); err != nil {
				t.Fatal(err)
			}
			reopened, err := Open(output)
			if err != nil {
				t.Fatal(err)
			}
			defer reopened.Close()
			text, err = reopened.PageText(0)
			if err != nil || !slices.Equal(text, []string{"B"}) {
				t.Fatalf("reopened optional PDF text: %v, %v", text, err)
			}
		})
	}
}

func TestImportsPDFDocumentInfo(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "metadata-tounicode.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	document, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer document.Close()
	infos, err := document.Documents()
	if err != nil {
		t.Fatal(err)
	}
	if len(infos) != 1 || infos[0].Title == nil || *infos[0].Title != "示例文档" ||
		infos[0].CreationDate == nil || *infos[0].CreationDate != "2024-02-29" ||
		!slices.Equal(infos[0].Keywords, []string{"测试, Rust"}) {
		t.Fatalf("PDF document info: %+v", infos)
	}
}

func TestImportsPDFOutlineTree(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "outline-tounicode.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	document, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer document.Close()
	outlines, err := document.Outlines()
	if err != nil {
		t.Fatal(err)
	}
	if len(outlines) != 3 || outlines[0].Title != "章节一" || outlines[0].Expanded ||
		outlines[1].ParentIndex == nil || *outlines[1].ParentIndex != 0 ||
		outlines[2].Title != "Chapter 2" {
		t.Fatalf("PDF outline tree: %+v", outlines)
	}
}

func testImportToUnicodePDF(t *testing.T, name string) {
	t.Helper()
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", name)
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	document, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer document.Close()
	text, err := document.PageText(0)
	if err != nil || !slices.Equal(text, []string{"中文"}) {
		t.Fatalf("ToUnicode text: %v, %v", text, err)
	}
	output := filepath.Join(t.TempDir(), "unicode.ofd")
	if err := document.Save(output); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(output)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	text, err = reopened.PageText(0)
	if err != nil || !slices.Equal(text, []string{"中文"}) {
		t.Fatalf("reopened ToUnicode text: %v, %v", text, err)
	}
}

func TestDecryptsJavaPasswordEncryptedDocument(t *testing.T) {
	path := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "java-password-encrypted.ofd")
	encrypted, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer encrypted.Close()
	if yes, err := encrypted.IsEncrypted(); err != nil || !yes {
		t.Fatalf("encryption flag: %v, %v", yes, err)
	}
	info, err := encrypted.EncryptionInfo()
	if err != nil || len(info) != 1 || !slices.Equal(info[0].Usernames, []string{"777"}) {
		t.Fatalf("encryption info: %+v, %v", info, err)
	}
	if _, err := encrypted.DecryptWithPassword("wrong", ""); err == nil {
		t.Fatal("wrong password accepted")
	}
	if _, err := encrypted.DecryptWithPassword("12345678", "other"); err == nil {
		t.Fatal("wrong user accepted")
	}
	plain, err := encrypted.DecryptWithPassword("12345678", "777")
	if err != nil {
		t.Fatal(err)
	}
	defer plain.Close()
	if yes, err := plain.IsEncrypted(); err != nil || yes {
		t.Fatalf("plaintext flag: %v, %v", yes, err)
	}
	pages, err := plain.Pages()
	if err != nil || len(pages) != 2 {
		t.Fatalf("decrypted pages: %v, %v", pages, err)
	}
	text, err := plain.PageText(0)
	if err != nil || len(text) == 0 {
		t.Fatalf("decrypted text: %v, %v", text, err)
	}
	wrapped, err := plain.EncryptWithPasswords([]PasswordRecipient{
		{Username: "alice", Password: "first"},
		{Username: "bob", Password: "second", UserType: "Owner"},
	})
	if err != nil {
		t.Fatal(err)
	}
	defer wrapped.Close()
	if yes, err := wrapped.IsEncrypted(); err != nil || !yes {
		t.Fatalf("wrapped flag: %v, %v", yes, err)
	}
	output := filepath.Join(t.TempDir(), "wrapped.ofd")
	if err := wrapped.Save(output); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(output)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	restored, err := reopened.DecryptWithPassword("second", "bob")
	if err != nil {
		t.Fatal(err)
	}
	defer restored.Close()
	restoredText, err := restored.PageText(0)
	if err != nil || !slices.Equal(restoredText, text) {
		t.Fatalf("restored text: %v, %v", restoredText, err)
	}
	if _, err := plain.EncryptWithPasswords(nil); err == nil {
		t.Fatal("empty recipient list accepted")
	}
}

func TestCertificateEncryptionInteroperatesWithJava(t *testing.T) {
	fixtures := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	key, err := os.ReadFile(filepath.Join(fixtures, "java-sm2-private-key.bin"))
	if err != nil {
		t.Fatal(err)
	}
	cert, err := os.ReadFile(filepath.Join(fixtures, "java-sm2-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	encrypted, err := Open(filepath.Join(fixtures, "java-cert-encrypted.ofd"))
	if err != nil {
		t.Fatal(err)
	}
	defer encrypted.Close()
	if _, err := encrypted.DecryptWithSM2PrivateKey(make([]byte, 32), "777"); err == nil {
		t.Fatal("wrong SM2 key accepted")
	}
	plain, err := encrypted.DecryptWithSM2PrivateKey(key, "777")
	if err != nil {
		t.Fatal(err)
	}
	defer plain.Close()
	text, err := plain.PageText(0)
	if err != nil || len(text) == 0 {
		t.Fatalf("decrypted text: %v, %v", text, err)
	}
	wrapped, err := plain.EncryptWithCertificates([]CertificateRecipient{{Username: "go", CertificateDER: cert}})
	if err != nil {
		t.Fatal(err)
	}
	defer wrapped.Close()
	info, err := wrapped.EncryptionInfo()
	if err != nil || len(info) != 1 || info[0].ProtectionCaseID == nil || *info[0].ProtectionCaseID != "1.1.2" {
		t.Fatalf("certificate encryption info: %+v, %v", info, err)
	}
	output := filepath.Join(t.TempDir(), "cert-encrypted.ofd")
	if err := wrapped.Save(output); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(output)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	restored, err := reopened.DecryptWithSM2PrivateKey(key, "go")
	if err != nil {
		t.Fatal(err)
	}
	defer restored.Close()
	restoredText, err := restored.PageText(0)
	if err != nil || !slices.Equal(restoredText, text) {
		t.Fatalf("restored text: %v, %v", restoredText, err)
	}
	if _, err := plain.EncryptWithCertificates(nil); err == nil {
		t.Fatal("empty certificate recipients accepted")
	}
}

func TestMixedPasswordAndCertificateRecipients(t *testing.T) {
	fixtures := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	key, err := os.ReadFile(filepath.Join(fixtures, "java-sm2-private-key.bin"))
	if err != nil {
		t.Fatal(err)
	}
	certificate, err := os.ReadFile(filepath.Join(fixtures, "java-sm2-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	encrypted, err := Open(filepath.Join(fixtures, "java-cert-encrypted.ofd"))
	if err != nil {
		t.Fatal(err)
	}
	defer encrypted.Close()
	plain, err := encrypted.DecryptWithSM2PrivateKey(key, "")
	if err != nil {
		t.Fatal(err)
	}
	defer plain.Close()
	expected, err := plain.PageText(0)
	if err != nil {
		t.Fatal(err)
	}
	mixed, err := plain.EncryptWithRecipients([]EncryptionRecipient{
		{Type: "password", Username: "password", Password: "secret"},
		{Type: "certificate", Username: "certificate", CertificateDER: certificate},
	})
	if err != nil {
		t.Fatal(err)
	}
	defer mixed.Close()
	info, err := mixed.EncryptionInfo()
	if err != nil || len(info) != 1 || info[0].ProtectionCaseID != nil {
		t.Fatalf("mixed encryption info: %+v, %v", info, err)
	}
	output := filepath.Join(t.TempDir(), "mixed.ofd")
	if err := mixed.Save(output); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(output)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	byPassword, err := reopened.DecryptWithPassword("secret", "password")
	if err != nil {
		t.Fatal(err)
	}
	defer byPassword.Close()
	byCertificate, err := reopened.DecryptWithSM2PrivateKey(key, "certificate")
	if err != nil {
		t.Fatal(err)
	}
	defer byCertificate.Close()
	for _, restored := range []*Document{byPassword, byCertificate} {
		text, err := restored.PageText(0)
		if err != nil || !slices.Equal(text, expected) {
			t.Fatalf("restored text: %v, %v", text, err)
		}
	}
	if _, err := reopened.DecryptWithPassword("secret", "certificate"); err == nil {
		t.Fatal("wrong user type accepted")
	}
	if _, err := plain.EncryptWithRecipients(nil); err == nil {
		t.Fatal("empty recipients accepted")
	}
	javaMixed, err := Open(filepath.Join(fixtures, "java-mixed-encrypted.ofd"))
	if err != nil {
		t.Fatal(err)
	}
	defer javaMixed.Close()
	javaPassword, err := javaMixed.DecryptWithPassword("java-mixed-pass", "java-password-user")
	if err != nil {
		t.Fatal(err)
	}
	defer javaPassword.Close()
	javaCertificate, err := javaMixed.DecryptWithSM2PrivateKey(key, "java-cert-user")
	if err != nil {
		t.Fatal(err)
	}
	defer javaCertificate.Close()
	for _, restored := range []*Document{javaPassword, javaCertificate} {
		text, err := restored.PageText(0)
		if err != nil || !slices.Equal(text, expected) {
			t.Fatalf("Java mixed restored text: %v, %v", text, err)
		}
	}
}

func TestPasswordEncryptionErrorPaths(t *testing.T) {
	plain, err := New()
	if err != nil {
		t.Fatal(err)
	}
	if info, err := plain.EncryptionInfo(); err != nil || len(info) != 0 {
		t.Fatalf("plaintext encryption info: %+v, %v", info, err)
	}
	if copy, err := plain.DecryptWithPassword("anything", ""); err != nil {
		t.Fatal(err)
	} else {
		copy.Close()
	}
	if _, err := plain.DecryptWithPassword("", ""); err == nil {
		t.Fatal("empty password accepted")
	}
	if _, err := plain.EncryptWithPasswords([]PasswordRecipient{{Username: "", Password: "secret"}}); err == nil {
		t.Fatal("empty username accepted")
	}
	if _, err := plain.EncryptWithPasswords([]PasswordRecipient{{Username: "a", Password: "secret", UserType: "Admin"}}); err == nil {
		t.Fatal("invalid role accepted")
	}
	if _, err := plain.EncryptWithPasswords([]PasswordRecipient{{Username: "a", Password: "first"}, {Username: "a", Password: "second"}}); err == nil {
		t.Fatal("duplicate username accepted")
	}
	user := []PasswordRecipient{{Username: "a", Password: "secret"}}
	encrypted, err := plain.EncryptWithPasswords(user)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := encrypted.EncryptWithPasswords(user); err == nil {
		t.Fatal("double encryption accepted")
	}
	encrypted.Close()
	if _, err := encrypted.IsEncrypted(); err == nil {
		t.Fatal("closed encryption status accepted")
	}
	if _, err := encrypted.EncryptionInfo(); err == nil {
		t.Fatal("closed encryption info accepted")
	}
	if _, err := encrypted.DecryptWithPassword("secret", ""); err == nil {
		t.Fatal("closed decryption accepted")
	}
	plain.Close()
	if _, err := plain.EncryptWithPasswords(user); err == nil {
		t.Fatal("closed encryption accepted")
	}
}

func TestDocumentColorSpaceRoundTripAndValidation(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddDocumentColorSpace(0, ColorSpaceSpec{
		ColorType: "RGB", Palette: [][]uint16{{255, 0, 0}, {0, 0, 255}},
	})
	if err != nil {
		t.Fatal(err)
	}
	spaces, err := doc.DocumentColorSpaces(0)
	if err != nil || len(spaces) != 1 || spaces[0].ID != strconv.Itoa(id) || spaces[0].BitsPerComponent != 8 {
		t.Fatalf("document spaces: %+v, %v", spaces, err)
	}
	pageSpaces, err := doc.PageColorSpaces(0)
	if err != nil || len(pageSpaces) != 1 || pageSpaces[0].ResourcePath != spaces[0].ResourcePath {
		t.Fatalf("page spaces: %+v, %v", pageSpaces, err)
	}
	if got, err := doc.DocumentDefaultColorSpace(0); err != nil || got != nil {
		t.Fatalf("initial DefaultCS: %v, %v", got, err)
	}
	if err := doc.SetDocumentDefaultColorSpace(0, "999999"); err == nil {
		t.Fatal("unresolved DefaultCS accepted")
	}
	if err := doc.SetDocumentDefaultColorSpace(0, strconv.Itoa(id)); err != nil {
		t.Fatal(err)
	}
	if got, err := doc.DocumentDefaultColorSpace(0); err != nil || got == nil || *got != strconv.Itoa(id) {
		t.Fatalf("DefaultCS: %v, %v", got, err)
	}
	pageID, err := doc.AddPageColorSpace(0, ColorSpaceSpec{ColorType: "GRAY", BitsPerComponent: 4})
	if err != nil || pageID <= id {
		t.Fatalf("page ColorSpace: %d, %v", pageID, err)
	}
	pageSpaces, err = doc.PageColorSpaces(0)
	if err != nil || len(pageSpaces) != 2 || pageSpaces[0].ID != strconv.Itoa(pageID) {
		t.Fatalf("effective page spaces: %+v, %v", pageSpaces, err)
	}
	if err := doc.SetDocumentColorSpace(0, strconv.Itoa(id), ColorSpaceSpec{
		ColorType: "RGB", Palette: [][]uint16{{0, 255, 0}},
	}); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageColorSpace(0, strconv.Itoa(pageID), ColorSpaceSpec{
		ColorType: "GRAY", BitsPerComponent: 4, Palette: [][]uint16{{15}},
	}); err != nil {
		t.Fatal(err)
	}
	pageSpaces, err = doc.PageColorSpaces(0)
	if err != nil || len(pageSpaces) != 2 || pageSpaces[0].Palette[0][0] != 15 || pageSpaces[1].Palette[0][1] != 255 {
		t.Fatalf("updated spaces: %+v, %v", pageSpaces, err)
	}
	if err := doc.SetDocumentColorSpace(0, strconv.Itoa(pageID), ColorSpaceSpec{ColorType: "GRAY"}); err == nil {
		t.Fatal("wrong-scope ColorSpace update accepted")
	}
	if err := doc.SetPageColorSpace(0, strconv.Itoa(pageID), ColorSpaceSpec{
		ColorType: "GRAY", BitsPerComponent: 4, Palette: [][]uint16{{16}},
	}); err == nil {
		t.Fatal("invalid palette update accepted")
	}
	after, err := doc.PageColorSpaces(0)
	if err != nil || !reflect.DeepEqual(after, pageSpaces) {
		t.Fatalf("failed update changed spaces: %+v, %v", after, err)
	}
	if err := doc.SetDocumentDefaultColorSpace(0, ""); err != nil {
		t.Fatal(err)
	}
	if got, err := doc.DocumentDefaultColorSpace(0); err != nil || got != nil {
		t.Fatalf("cleared DefaultCS: %v, %v", got, err)
	}
	if _, err := doc.AddDocumentColorSpace(0, ColorSpaceSpec{ColorType: "RGB", Palette: [][]uint16{{256, 0, 0}}}); err == nil {
		t.Fatal("invalid palette accepted")
	}
	if _, err := doc.PageColorSpaces(-1); err == nil {
		t.Fatal("negative page index accepted")
	}
	if err := doc.Save(filepath.Join(t.TempDir(), "colors.ofd")); err != nil {
		t.Fatal(err)
	}
}

func TestColorSpaceWithoutPublicResourcesAndErrorPaths(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddDocumentColorSpace(-1, ColorSpaceSpec{ColorType: "RGB"}); err == nil {
		t.Fatal("negative document index accepted")
	}
	if _, err := doc.DocumentColorSpaces(-1); err == nil {
		t.Fatal("negative document index accepted")
	}
	if _, err := doc.DocumentDefaultColorSpace(-1); err == nil {
		t.Fatal("negative DefaultCS index accepted")
	}
	if err := doc.SetDocumentDefaultColorSpace(-1, ""); err == nil {
		t.Fatal("negative DefaultCS setter index accepted")
	}
	if _, err := doc.PageColorSpaces(0); err == nil {
		t.Fatal("missing page accepted")
	}
	if _, err := doc.AddDocumentColorSpace(0, ColorSpaceSpec{ColorType: "XYZ"}); err == nil {
		t.Fatal("invalid color type accepted")
	}
	id, err := doc.AddDocumentColorSpace(0, ColorSpaceSpec{ColorType: "GRAY", BitsPerComponent: 4,
		Palette: [][]uint16{{0}, {15}}})
	if err != nil {
		t.Fatal(err)
	}
	spaces, err := doc.DocumentColorSpaces(0)
	if err != nil || len(spaces) != 1 || spaces[0].ID != strconv.Itoa(id) || spaces[0].BitsPerComponent != 4 {
		t.Fatalf("new document color spaces: %+v, %v", spaces, err)
	}
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	if page, err := doc.PageColorSpaces(0); err != nil || len(page) != 1 {
		t.Fatalf("page color spaces: %+v, %v", page, err)
	}
	if err := doc.SetEntry("Doc_0/Res/bad.icc", []byte("bad profile")); err != nil {
		t.Fatal(err)
	}
	badICC := "bad.icc"
	if _, err := doc.AddDocumentColorSpace(0, ColorSpaceSpec{ColorType: "RGB", Profile: &badICC}); err == nil {
		t.Fatal("invalid ICC accepted")
	}
	if _, err := doc.DocumentColorSpaces(1); err == nil {
		t.Fatal("missing document accepted")
	}
	if _, err := doc.DocumentDefaultColorSpace(1); err == nil {
		t.Fatal("missing DefaultCS document accepted")
	}
	if err := doc.SetDocumentDefaultColorSpace(1, ""); err == nil {
		t.Fatal("missing DefaultCS setter document accepted")
	}
	if err := doc.SetDocumentDefaultColorSpace(0, "\x00"); err == nil {
		t.Fatal("NUL DefaultCS ID accepted")
	}
	if _, err := doc.AddPageColorSpace(-1, ColorSpaceSpec{ColorType: "RGB"}); err == nil {
		t.Fatal("negative page ColorSpace index accepted")
	}
	if _, err := doc.AddPageColorSpace(3, ColorSpaceSpec{ColorType: "RGB"}); err == nil {
		t.Fatal("missing page ColorSpace index accepted")
	}
	if _, err := doc.AddPageColorSpace(0, ColorSpaceSpec{ColorType: "RGB", BitsPerComponent: 3}); err == nil {
		t.Fatal("invalid page ColorSpace accepted")
	}
	doc.Close()
	if _, err := doc.DocumentColorSpaces(0); err == nil {
		t.Fatal("closed document accepted")
	}
	if _, err := doc.AddDocumentColorSpace(0, ColorSpaceSpec{ColorType: "RGB"}); err == nil {
		t.Fatal("closed document write accepted")
	}
	if _, err := doc.DocumentDefaultColorSpace(0); err == nil {
		t.Fatal("closed DefaultCS read accepted")
	}
	if err := doc.SetDocumentDefaultColorSpace(0, ""); err == nil {
		t.Fatal("closed DefaultCS write accepted")
	}
	if _, err := doc.AddPageColorSpace(0, ColorSpaceSpec{ColorType: "RGB"}); err == nil {
		t.Fatal("closed page ColorSpace write accepted")
	}
}

func TestRemoveUnreferencedColorSpaces(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	public, err := doc.AddDocumentColorSpace(0, ColorSpaceSpec{ColorType: "RGB"})
	if err != nil {
		t.Fatal(err)
	}
	local, err := doc.AddPageColorSpace(0, ColorSpaceSpec{ColorType: "GRAY"})
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetDocumentDefaultColorSpace(0, strconv.Itoa(public)); err != nil {
		t.Fatal(err)
	}
	if err := doc.RemoveDocumentColorSpace(0, strconv.Itoa(public)); err == nil {
		t.Fatal("referenced document ColorSpace removed")
	}
	if err := doc.RemovePageColorSpace(0, strconv.Itoa(local)); err != nil {
		t.Fatal(err)
	}
	if err := doc.RemovePageColorSpace(0, strconv.Itoa(public)); err == nil {
		t.Fatal("wrong-scope ColorSpace removed")
	}
	if err := doc.SetDocumentDefaultColorSpace(0, ""); err != nil {
		t.Fatal(err)
	}
	if err := doc.RemoveDocumentColorSpace(0, strconv.Itoa(public)); err != nil {
		t.Fatal(err)
	}
	spaces, err := doc.PageColorSpaces(0)
	if err != nil || len(spaces) != 0 {
		t.Fatalf("remaining spaces: %+v, %v", spaces, err)
	}
}

func TestReportsMissingColorSpaceReference(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	xml, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	modified := strings.Replace(string(xml), "</ofd:Content>",
		`<ofd:Layer ID="500"><ofd:PathObject ID="501" Boundary="0 0 1 1" Fill="true"><ofd:FillColor ColorSpace="999" Index="0"/><ofd:AbbreviatedData>M 0 0 L 1 1</ofd:AbbreviatedData></ofd:PathObject></ofd:Layer></ofd:Content>`, 1)
	if err := doc.SetEntry(pages[0].Path, []byte(modified)); err != nil {
		t.Fatal(err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	if !slices.ContainsFunc(issues, func(issue ValidationIssue) bool { return issue.Code == "color_space_reference" }) {
		t.Fatalf("missing color-space issue: %+v", issues)
	}
	colorID, err := doc.AddDocumentColorSpace(0, ColorSpaceSpec{ColorType: "RGB", Palette: [][]uint16{{255, 0, 0}}})
	if err != nil {
		t.Fatal(err)
	}
	modified = strings.Replace(modified, `ColorSpace="999" Index="0"`, fmt.Sprintf(`ColorSpace="%d" Index="1"`, colorID), 1)
	if err := doc.SetEntry(pages[0].Path, []byte(modified)); err != nil {
		t.Fatal(err)
	}
	issues, err = doc.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	if !slices.ContainsFunc(issues, func(issue ValidationIssue) bool { return issue.Code == "invalid_color_index" }) {
		t.Fatalf("missing palette issue: %+v", issues)
	}
}

func TestPageObjectNameRoundTripsAcrossBinding(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("named", 40, 30); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("page objects: %+v, %v", objects, err)
	}
	id := *objects[0].ID
	name := `发票 & "A"`
	if err := doc.SetPageObjectName(0, id, &name); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.PageObjects(0)
	if err != nil || objects[0].Name == nil || *objects[0].Name != name {
		t.Fatalf("object name: %+v, %v", objects, err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	invalid := "bad\x00name"
	if err := doc.SetPageObjectName(0, id, &invalid); err == nil {
		t.Fatal("accepted NUL in object name")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(after, before) {
		t.Fatalf("failed name edit changed package: %v", err)
	}
	reopened, err := FromBytes(before)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	objects, err = reopened.PageObjects(0)
	if err != nil || objects[0].Name == nil || *objects[0].Name != name {
		t.Fatalf("reopened name: %+v, %v", objects, err)
	}
	if err := doc.SetPageObjectName(0, id, nil); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.PageObjects(0)
	if err != nil || objects[0].Name != nil {
		t.Fatalf("name was not cleared: %+v, %v", objects, err)
	}
}

func TestPageObjectBlendModeRoundTripsAcrossBinding(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("blend", 40, 30); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("page objects: %+v, %v", objects, err)
	}
	id := *objects[0].ID
	mode := "darken"
	if err := doc.SetPageObjectBlendMode(0, id, &mode); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.PageObjects(0)
	if err != nil || objects[0].BlendMode == nil || *objects[0].BlendMode != "Darken" {
		t.Fatalf("blend mode: %+v, %v", objects, err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	invalid := "invalid"
	if err := doc.SetPageObjectBlendMode(0, id, &invalid); err == nil {
		t.Fatal("accepted invalid blend mode")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid mode changed document: %v", err)
	}
	if err := doc.SetPageObjectBlendMode(0, id, nil); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.PageObjects(0)
	if err != nil || objects[0].BlendMode != nil {
		t.Fatalf("cleared blend mode: %+v, %v", objects, err)
	}
	if err := doc.SetPageObjectBlendMode(-1, id, nil); err == nil {
		t.Fatal("accepted negative page index")
	}
	if err := doc.SetPageObjectBlendMode(0, "bad\x00id", nil); err == nil {
		t.Fatal("accepted NUL in object ID")
	}
	badMode := "bad\x00mode"
	if err := doc.SetPageObjectBlendMode(0, id, &badMode); err == nil {
		t.Fatal("accepted NUL in blend mode")
	}
	doc.Close()
	if err := doc.SetPageObjectBlendMode(0, id, nil); err == nil {
		t.Fatal("accepted edit on closed document")
	}
}

func TestPageObjectClipRectsRoundTrip(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddPathPage("M 0 0 L 20 0 L 20 20 L 0 20 C", [4]float64{0, 0, 20, 20}, 20, 20, true, false); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("page objects: %+v, %v", objects, err)
	}
	id := *objects[0].ID
	if err := doc.SetPageObjectClipRects(0, id, [][4]float64{{0, 0, 10, 20}}, false); err != nil {
		t.Fatal(err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "ofd-clip-") {
		t.Fatalf("clipped SVG: %v", err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectClipRects(0, id, [][4]float64{{0, 0, -1, 20}}, false); err == nil {
		t.Fatal("accepted invalid clip rectangle")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid clip changed document: %v", err)
	}
	if err := doc.SetPageObjectClipRects(0, id, nil, false); err != nil {
		t.Fatal(err)
	}
	svg, err = doc.PageSVG(0)
	if err != nil || strings.Contains(svg, "ofd-clip-") {
		t.Fatalf("cleared clip SVG: %v", err)
	}
	if err := doc.SetPageObjectClipRects(-1, id, nil, false); err == nil {
		t.Fatal("accepted negative page index")
	}
	if err := doc.SetPageObjectClipRects(0, "bad\x00id", nil, false); err == nil {
		t.Fatal("accepted NUL in object ID")
	}
	if err := doc.SetPageObjectClipRects(0, id, [][4]float64{{math.NaN(), 0, 1, 1}}, false); err == nil {
		t.Fatal("accepted non-finite clip")
	}
	doc.Close()
	if err := doc.SetPageObjectClipRects(0, id, nil, false); err == nil {
		t.Fatal("accepted clip edit on closed document")
	}
}

func TestExternalEmptyAndMalformedClips(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddPathPage("M 0 0 L 20 0 L 20 20 L 0 20 C", [4]float64{0, 0, 20, 20}, 20, 20, true, false); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("page objects: %+v, %v", objects, err)
	}
	if err := doc.SetPageObjectClipRects(0, *objects[0].ID, [][4]float64{{0, 0, 10, 10}}, false); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	pagePath := pages[0].Path
	original, err := doc.Entry(pagePath)
	if err != nil {
		t.Fatal(err)
	}
	start := bytes.Index(original, []byte("<ofd:Clips"))
	end := bytes.Index(original, []byte("</ofd:Clips>")) + len("</ofd:Clips>")
	if start < 0 || end < len("</ofd:Clips>") {
		t.Fatal("generated clip was not found")
	}
	replace := func(clip string) []byte {
		return append(append([]byte(nil), original[:start]...), append([]byte(clip), original[end:]...)...)
	}
	if err := doc.SetEntry(pagePath, replace("<ofd:Clips/>")); err != nil {
		t.Fatal(err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("empty Clips references: %+v, %v", issues, err)
	}
	clips, err := doc.PageObjectClips(0)
	if err != nil || len(clips) != 1 || len(clips[0].Clips) != 0 {
		t.Fatalf("empty Clips read: %+v, %v", clips, err)
	}
	if _, err := doc.PageSVG(0); err != nil {
		t.Fatal(err)
	}
	broken := "<ofd:Clips><ofd:Clip><ofd:Area><ofd:Path/><ofd:Text/></ofd:Area></ofd:Clip></ofd:Clips>"
	if err := doc.SetEntry(pagePath, replace(broken)); err != nil {
		t.Fatal(err)
	}
	issues, err = doc.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, issue := range issues {
		if issue.Code == "invalid_clip_area" {
			found = true
		}
	}
	if !found {
		t.Fatalf("malformed clip was not reported: %+v", issues)
	}
	if _, err := doc.PageObjectClips(0); err == nil {
		t.Fatal("malformed clip was accepted by reader")
	}
	if _, err := doc.PageSVG(0); err == nil {
		t.Fatal("malformed clip was accepted by preview")
	}
}

func TestPageObjectPathClipsRoundTrip(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddPathPage("M 0 0 L 20 0 L 20 20 L 0 20 C", [4]float64{0, 0, 20, 20}, 20, 20, true, false); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("page objects: %+v, %v", objects, err)
	}
	id := *objects[0].ID
	area := PathClipArea{Boundary: [4]float64{0, 0, 10, 20}, Commands: "M 0 0 L 10 0 L 10 20 L 0 20 C", EvenOdd: true}
	if err := doc.SetPageObjectPathClips(0, id, [][]PathClipArea{{area}}, false); err != nil {
		t.Fatal(err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "ofd-clip-") {
		t.Fatalf("path clip SVG: %v", err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectPathClips(0, id, [][]PathClipArea{{}}, false); err == nil {
		t.Fatal("accepted clip without area")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid clip changed document: %v", err)
	}
	width := 2.0
	paramID, err := doc.AddDocumentDrawParam(0, DrawParamStyle{LineWidth: &width})
	if err != nil {
		t.Fatal(err)
	}
	strokeFill := false
	strokeEnabled := true
	area = PathClipArea{Boundary: [4]float64{0, 0, 20, 20}, Commands: "M 2 10 L 18 10", Fill: &strokeFill, Stroke: &strokeEnabled, DrawParam: strconv.Itoa(paramID)}
	if err := doc.SetPageObjectPathClips(0, id, [][]PathClipArea{{area}}, false); err != nil {
		t.Fatal(err)
	}
	clipsInfo, err := doc.PageObjectClips(0)
	if err != nil || clipsInfo[0].Clips[0][0].Stroke == nil || !*clipsInfo[0].Clips[0][0].Stroke {
		t.Fatalf("stroked clip reader: %+v, %v", clipsInfo, err)
	}
	svg, err = doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "stroke-width=\"2\"") {
		t.Fatalf("stroked clip SVG: %v", err)
	}
	if err := doc.SetPageObjectPathClips(0, id, nil, false); err != nil {
		t.Fatal(err)
	}
	svg, err = doc.PageSVG(0)
	if err != nil || strings.Contains(svg, "ofd-clip-") {
		t.Fatalf("cleared path clip SVG: %v", err)
	}
	if err := doc.SetPageObjectPathClips(-1, id, nil, false); err == nil {
		t.Fatal("accepted negative page index")
	}
	if err := doc.SetPageObjectPathClips(0, "bad\x00id", nil, false); err == nil {
		t.Fatal("accepted NUL in object ID")
	}
	bad := area
	bad.Boundary[0] = math.NaN()
	if err := doc.SetPageObjectPathClips(0, id, [][]PathClipArea{{bad}}, false); err == nil {
		t.Fatal("accepted non-finite path boundary")
	}
	doc.Close()
	if err := doc.SetPageObjectPathClips(0, id, nil, false); err == nil {
		t.Fatal("accepted edit on closed document")
	}
}

func TestMixedTextAndPathClipsRoundTrip(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("font", 30, 30); err != nil {
		t.Fatal(err)
	}
	fonts, err := doc.Fonts()
	if err != nil || len(fonts) == 0 {
		t.Fatalf("fonts: %+v, %v", fonts, err)
	}
	id, err := doc.AddPathObject(0, "M 0 0 L 20 0 L 20 20 L 0 20 C", [4]float64{0, 0, 20, 20}, PathStyle{Fill: true})
	if err != nil {
		t.Fatal(err)
	}
	objectID := strconv.Itoa(id)
	clips := [][]ClipAreaSpec{{
		{Kind: "Path", Boundary: [4]float64{0, 0, 5, 20}, Commands: "M 0 0 L 5 0 L 5 20 L 0 20 C"},
		{Kind: "Text", Boundary: [4]float64{0, 0, 20, 20}, FontID: fonts[0].ID, Size: 14, Text: "O", X: 10, Y: 15},
	}}
	if err := doc.SetPageObjectClips(0, objectID, clips, false); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageObjectClips(0)
	if err != nil {
		t.Fatal(err)
	}
	var selected *PageObjectClipsInfo
	for i := range objects {
		if objects[i].ID != nil && *objects[i].ID == objectID {
			selected = &objects[i]
			break
		}
	}
	if selected == nil || len(selected.Clips) != 1 || len(selected.Clips[0]) != 2 || selected.Clips[0][0].Kind != "Path" || selected.Clips[0][1].Kind != "Text" || selected.Clips[0][1].Text == nil || *selected.Clips[0][1].Text != "O" || !strings.Contains(selected.Clips[0][1].XML, "TextCode") {
		t.Fatalf("mixed clip reader: %+v", selected)
	}
	if selected.Clips[0][0].Fill == nil || !*selected.Clips[0][0].Fill || selected.Clips[0][0].Stroke == nil || *selected.Clips[0][0].Stroke {
		t.Fatalf("path clip fill/stroke: %+v", selected.Clips[0][0])
	}
	if len(selected.Clips[0][1].TextCodes) != 1 || selected.Clips[0][1].TextCodes[0].X == nil || *selected.Clips[0][1].TextCodes[0].X != 10 {
		t.Fatalf("clip text position: %+v", selected.Clips[0][1].TextCodes)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "ofd-clip-") {
		t.Fatalf("mixed clip SVG: %v", err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	reopened, err := FromBytes(before)
	if err != nil {
		t.Fatal(err)
	}
	readAgain, err := reopened.PageObjectClips(0)
	reopened.Close()
	if err != nil || !reflect.DeepEqual(readAgain, objects) {
		t.Fatalf("reopened mixed clips: %v", err)
	}
	clips[0][1].FontID = "999999"
	if err := doc.SetPageObjectClips(0, objectID, clips, false); err == nil {
		t.Fatal("accepted missing clip font")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid clip changed document: %v", err)
	}
	floatPtr := func(value float64) *float64 { return &value }
	rich := [][]ClipAreaSpec{{{
		Kind: "Text", Boundary: [4]float64{0, 0, 20, 20}, FontID: fonts[0].ID,
		Size: 14, Text: "IX", X: 2, Y: 15,
		TextCodes:    []ClipTextCodeInfo{{X: floatPtr(2), Y: floatPtr(15), Text: "I"}, {X: floatPtr(8), Y: floatPtr(15), Text: "X"}},
		CGTransforms: []ClipCGTransformInfo{{CodePosition: 0, CodeCount: 2, GlyphCount: 1, Glyphs: []uint32{1}}},
	}}}
	if err := doc.SetPageObjectClips(0, objectID, rich, false); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.PageObjectClips(0)
	if err != nil || len(objects[1].Clips[0][0].TextCodes) != 2 || objects[1].Clips[0][0].CGTransforms[0].Glyphs[0] != 1 {
		t.Fatalf("rich clip round trip: %+v, %v", objects, err)
	}
	width := 2.0
	paramID, err := doc.AddDocumentDrawParam(0, DrawParamStyle{LineWidth: &width})
	if err != nil {
		t.Fatal(err)
	}
	fillOff := false
	strokeOn := true
	strokeClip := [][]ClipAreaSpec{{{
		Kind: "Text", Boundary: [4]float64{0, 0, 20, 20}, FontID: fonts[0].ID,
		Size: 14, Text: "O", X: 3, Y: 16,
		Fill: &fillOff, Stroke: &strokeOn, DrawParam: strconv.Itoa(paramID),
	}}}
	if err := doc.SetPageObjectClips(0, objectID, strokeClip, false); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.PageObjectClips(0)
	if err != nil || objects[1].Clips[0][0].Stroke == nil || !*objects[1].Clips[0][0].Stroke {
		t.Fatalf("stroked text clip reader: %+v, %v", objects, err)
	}
	svg, err = doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "stroke-width=\"2\"") {
		t.Fatalf("stroked text clip SVG: %v", err)
	}
	if err := doc.SetPageObjectClips(0, objectID, nil, false); err != nil {
		t.Fatal(err)
	}
	svg, err = doc.PageSVG(0)
	if err != nil || strings.Contains(svg, "ofd-clip-") {
		t.Fatalf("cleared mixed clip SVG: %v", err)
	}
	if err := doc.SetPageObjectClips(-1, objectID, nil, false); err == nil {
		t.Fatal("accepted negative page index")
	}
	if err := doc.SetPageObjectClips(0, "bad\x00id", nil, false); err == nil {
		t.Fatal("accepted NUL in object ID")
	}
	bad := ClipAreaSpec{Kind: "Path", Boundary: [4]float64{math.NaN(), 0, 1, 1}, Commands: "M 0 0 L 1 0"}
	if err := doc.SetPageObjectClips(0, objectID, [][]ClipAreaSpec{{bad}}, false); err == nil {
		t.Fatal("accepted non-finite clip boundary")
	}
	doc.Close()
	if err := doc.SetPageObjectClips(0, objectID, nil, false); err == nil {
		t.Fatal("accepted edit on closed document")
	}
}

func TestPageLayerTypeAndDrawParamRoundTrip(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("layer text", 40, 30); err != nil {
		t.Fatal(err)
	}
	layerID, err := doc.AddPageLayer(0, "Foreground", "")
	if err != nil {
		t.Fatal(err)
	}
	width := 1.25
	paramID, err := doc.AddDocumentDrawParam(0, DrawParamStyle{LineWidth: &width})
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageLayerDrawParam(0, strconv.Itoa(layerID), strconv.Itoa(paramID)); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageLayerType(0, strconv.Itoa(layerID), "background"); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageLayerName(0, strconv.Itoa(layerID), "图层 & A"); err != nil {
		t.Fatal(err)
	}
	layers, err := doc.PageLayers(0)
	if err != nil || len(layers) == 0 || layers[0].ID == nil || *layers[0].ID != strconv.Itoa(layerID) || layers[0].LayerType != "Background" || layers[0].DrawParam == nil || *layers[0].DrawParam != strconv.Itoa(paramID) {
		t.Fatalf("edited layer: %+v, %v", layers, err)
	}
	if layers[0].Name == nil || *layers[0].Name != "图层 & A" {
		t.Fatalf("edited layer name: %+v", layers[0])
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageLayerType(0, strconv.Itoa(layerID), "Invalid"); err == nil {
		t.Fatal("accepted invalid layer type")
	}
	if err := doc.SetPageLayerDrawParam(0, strconv.Itoa(layerID), "missing"); err == nil {
		t.Fatal("accepted missing DrawParam")
	}
	if err := doc.SetPageLayerName(0, strconv.Itoa(layerID), "bad\x00name"); err == nil {
		t.Fatal("accepted invalid layer name")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid edit changed document: %v", err)
	}
	if err := doc.SetPageLayerDrawParam(0, strconv.Itoa(layerID), ""); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageLayerName(0, strconv.Itoa(layerID), ""); err != nil {
		t.Fatal(err)
	}
	layers, err = doc.PageLayers(0)
	if err != nil || layers[0].DrawParam != nil || layers[0].Name != nil {
		t.Fatalf("cleared DrawParam: %+v, %v", layers, err)
	}
	if err := doc.SetPageLayerType(-1, strconv.Itoa(layerID), "Body"); err == nil {
		t.Fatal("accepted negative page index for layer type")
	}
	if err := doc.SetPageLayerDrawParam(-1, strconv.Itoa(layerID), ""); err == nil {
		t.Fatal("accepted negative page index for DrawParam")
	}
	if err := doc.SetPageLayerType(0, "bad\x00id", "Body"); err == nil {
		t.Fatal("accepted NUL in layer ID")
	}
	if err := doc.SetPageLayerType(0, strconv.Itoa(layerID), "bad\x00type"); err == nil {
		t.Fatal("accepted NUL in layer type")
	}
	if err := doc.SetPageLayerDrawParam(0, "bad\x00id", ""); err == nil {
		t.Fatal("accepted NUL in layer ID for DrawParam")
	}
	if err := doc.SetPageLayerDrawParam(0, strconv.Itoa(layerID), "bad\x00reference"); err == nil {
		t.Fatal("accepted NUL in DrawParam reference")
	}
	if err := doc.SetPageLayerDrawParam(0, "missing", ""); err == nil {
		t.Fatal("accepted missing layer")
	}
	doc.Close()
	if err := doc.SetPageLayerType(0, strconv.Itoa(layerID), "Body"); err == nil {
		t.Fatal("accepted layer type edit on closed document")
	}
	if err := doc.SetPageLayerDrawParam(0, strconv.Itoa(layerID), ""); err == nil {
		t.Fatal("accepted DrawParam edit on closed document")
	}
}

func TestReadsAndRendersJavaNestedPageBlock(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "helloworld_with_pageblock.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	content, err := doc.PageText(0)
	if err != nil || !slices.Equal(content, []string{"你好呀，OFD Reader&Writer！"}) {
		t.Fatalf("page text: %v, %v", content, err)
	}
	layers, err := doc.PageLayers(0)
	if err != nil || len(layers) != 1 || layers[0].ObjectCount != 1 {
		t.Fatalf("page layers: %+v, %v", layers, err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].Kind != "TextObject" {
		t.Fatalf("page objects: %+v, %v", objects, err)
	}
	text, err := doc.PageTextObjects(0)
	if err != nil || len(text) != 1 {
		t.Fatalf("text objects: %+v, %v", text, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "OFD Reader&amp;Writer") {
		t.Fatalf("page SVG: %v, %v", svg, err)
	}
	if objects[0].ID == nil {
		t.Fatal("missing PageBlock text object ID")
	}
	if err := doc.SetPageObjectVisible(0, *objects[0].ID, false); err != nil {
		t.Fatal(err)
	}
	content, err = doc.PageText(0)
	if err != nil || len(content) != 0 {
		t.Fatalf("hidden PageBlock text: %v, %v", content, err)
	}
	if err := doc.SetPageObjectVisible(0, *objects[0].ID, true); err != nil {
		t.Fatal(err)
	}
	added, err := doc.AddTextObject(0, "outside", [4]float64{10, 20, 40, 10}, 5, "")
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.MovePageObject(0, strconv.Itoa(added), *objects[0].ID); err != nil {
		t.Fatal(err)
	}
	content, err = doc.PageText(0)
	if err != nil || !slices.Equal(content, []string{"outside", "你好呀，OFD Reader&Writer！"}) {
		t.Fatalf("moved into PageBlock: %v, %v", content, err)
	}
	if err := doc.MovePageObject(0, *objects[0].ID, strconv.Itoa(added)); err != nil {
		t.Fatal(err)
	}
	content, err = doc.PageText(0)
	if err != nil || !slices.Equal(content, []string{"你好呀，OFD Reader&Writer！", "outside"}) {
		t.Fatalf("reordered in PageBlock: %v, %v", content, err)
	}
}

func TestGroupsPageObjectsIntoNestedBlocks(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("grouped", 30, 20); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("page objects: %v, %v", objects, err)
	}
	first := *objects[0].ID
	second, err := doc.AddPathObject(0, "M 0 0 L 3 3", [4]float64{1, 1, 4, 4}, PathStyle{Stroke: true})
	if err != nil {
		t.Fatal(err)
	}
	before, err := doc.PageSVG(0)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.GroupPageObjects(0, first, strconv.Itoa(second)); err != nil {
		t.Fatal(err)
	}
	if err := doc.GroupPageObjects(0, first, first); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	xml, err := doc.Entry(pages[0].Path)
	if err != nil || bytes.Count(xml, []byte("<ofd:PageBlock>")) != 2 {
		t.Fatalf("nested PageBlock: %s, %v", xml, err)
	}
	after, err := doc.PageSVG(0)
	if err != nil || after != before {
		t.Fatalf("grouped SVG changed: %v", err)
	}
	text, err := doc.PageText(0)
	if err != nil || !slices.Equal(text, []string{"grouped"}) {
		t.Fatalf("grouped page text: %v, %v", text, err)
	}
	if err := doc.GroupPageObjects(0, first, "missing"); err == nil {
		t.Fatal("expected missing object error")
	}
	if err := doc.UngroupPageObjects(0, first); err != nil {
		t.Fatal(err)
	}
	if err := doc.UngroupPageObjects(0, strconv.Itoa(second)); err != nil {
		t.Fatal(err)
	}
	xml, err = doc.Entry(pages[0].Path)
	if err != nil || bytes.Contains(xml, []byte("PageBlock")) {
		t.Fatalf("PageBlock remains after ungroup: %s, %v", xml, err)
	}
	if err := doc.UngroupPageObjects(0, first); err == nil {
		t.Fatal("expected ungroup error for direct layer object")
	}
}

func TestAddsPaginatedTableWithRepeatedHeader(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := DefaultTableStyle()
	style.PageSize = [2]float64{30, 26}
	style.Margins = [4]float64{2, 2, 2, 2}
	style.ColumnWidths = []float64{12, 12}
	style.RowHeight, style.FontSize, style.LineHeight = 8, 3, 3
	style.RepeatHeader = true
	rows := [][]string{{"H1", "H2"}, {"A\nA2", "B"}, {"C", "D"}, {"E", "F"}}
	indexes, err := doc.AddTable(0, rows, style)
	if err != nil || !slices.Equal(indexes, []int{0, 1, 2}) {
		t.Fatalf("table pages: %v, %v", indexes, err)
	}
	text, err := doc.PageText(1)
	if err != nil || !slices.Equal(text, []string{"H1", "H2", "C", "D"}) {
		t.Fatalf("second page: %v, %v", text, err)
	}
	pages, err := doc.PageCount()
	if err != nil || pages != 3 {
		t.Fatalf("page count: %d, %v", pages, err)
	}
	style.RowHeight = 0
	auto, err := doc.AddTable(0, rows, style)
	if err != nil || !slices.Equal(auto, []int{3, 4}) {
		t.Fatalf("automatic table pages: %v, %v", auto, err)
	}
	paths, err := doc.PagePaths(3)
	if err != nil || len(paths) < 4 || paths[0].Boundary[3] != 5 || paths[2].Boundary[3] != 8 {
		t.Fatalf("automatic row heights: %v, %v", paths, err)
	}
	invalid := style
	invalid.ColumnWidths = []float64{12}
	if _, err := doc.AddTable(0, rows, invalid); err == nil {
		t.Fatal("expected unequal column count")
	}
	invalid = style
	invalid.BorderWidth = math.NaN()
	if _, err := doc.AddTable(0, rows, invalid); err == nil {
		t.Fatal("expected nonfinite width")
	}
	if _, err := doc.AddTable(-1, rows, style); err == nil {
		t.Fatal("expected negative document index")
	}
	pages, err = doc.PageCount()
	if err != nil || pages != 5 {
		t.Fatalf("table changed after error: %d, %v", pages, err)
	}
	path := filepath.Join(t.TempDir(), "table.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	text, err = reopened.PageText(2)
	if err != nil || !slices.Equal(text, []string{"H1", "H2", "E", "F"}) {
		t.Fatalf("reopened table: %v, %v", text, err)
	}
	text, err = reopened.PageText(4)
	if err != nil || !slices.Equal(text, []string{"H1", "H2", "E", "F"}) {
		t.Fatalf("reopened automatic table: %v, %v", text, err)
	}
	doc.Close()
	if _, err := doc.AddTable(0, rows, style); err == nil {
		t.Fatal("expected closed document")
	}
}

func TestAddsMergedTableCellsAndPreservesRowGroups(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := DefaultTableStyle()
	style.PageSize = [2]float64{30, 23}
	style.Margins = [4]float64{2, 2, 2, 2}
	style.ColumnWidths = []float64{12, 12}
	style.RowHeight, style.FontSize, style.LineHeight = 0, 3, 3
	style.RepeatHeader = true
	cells := []TableCell{
		{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1, Text: "H1"}, {Row: 0, Column: 1, RowSpan: 1, ColumnSpan: 1, Text: "H2"},
		{Row: 1, Column: 0, RowSpan: 2, ColumnSpan: 1, Text: "L1\nL2\nL3\nL4"}, {Row: 1, Column: 1, RowSpan: 1, ColumnSpan: 1, Text: "Top"},
		{Row: 2, Column: 1, RowSpan: 1, ColumnSpan: 1, Text: "Low"}, {Row: 3, Column: 0, RowSpan: 1, ColumnSpan: 2, Text: "Wide"},
	}
	if _, err := doc.AddTableCells(-1, 4, cells, style); err == nil {
		t.Fatal("expected negative document")
	}
	if _, err := doc.AddTableCells(0, -1, cells, style); err == nil {
		t.Fatal("expected negative row count")
	}
	indexes, err := doc.AddTableCells(0, 4, cells, style)
	if err != nil || !slices.Equal(indexes, []int{0, 1}) {
		t.Fatalf("merged pages: %v, %v", indexes, err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 5 || !slices.Equal(paths[2].Boundary, []float64{2, 7, 12, 14}) {
		t.Fatalf("merged paths: %v, %v", paths, err)
	}
	text, err := doc.PageText(1)
	if err != nil || !slices.Equal(text, []string{"H1", "H2", "Wide"}) {
		t.Fatalf("merged text: %v, %v", text, err)
	}
	if _, err := doc.AddTableCells(0, 4, cells[:len(cells)-1], style); err == nil {
		t.Fatal("expected uncovered slot")
	}
	path := filepath.Join(t.TempDir(), "merged-table.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	text, err = reopened.PageText(1)
	if err != nil || !slices.Equal(text, []string{"H1", "H2", "Wide"}) {
		t.Fatalf("reopened merged table: %v, %v", text, err)
	}
}

func TestStylesIndividualTableCells(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := DefaultTableStyle()
	style.PageSize = [2]float64{30, 20}
	style.Margins = [4]float64{2, 2, 2, 2}
	style.ColumnWidths = []float64{20}
	style.FontSize, style.LineHeight = 3, 3
	background := [3]uint8{20, 30, 40}
	foreground := [3]uint8{1, 2, 3}
	cells := []TableCell{{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1, Text: "A", BackgroundColor: &background, TextColor: &foreground, Bold: true, Italic: true, Underline: true, Strikethrough: true, Align: "right"}}
	if indexes, err := doc.AddTableCells(0, 1, cells, style); err != nil || !slices.Equal(indexes, []int{0}) {
		t.Fatalf("styled table: %v, %v", indexes, err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 4 || !paths[0].Fill {
		t.Fatalf("styled paths: %v, %v", paths, err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || objects[0].Boundary[0] <= 10 || objects[0].Weight != 700 || !objects[0].Italic {
		t.Fatalf("right aligned text: %v, %v", objects, err)
	}
	cells[0].Align = "justify"
	if _, err := doc.AddTableCells(0, 1, cells, style); err == nil {
		t.Fatal("expected invalid alignment")
	}
	path := filepath.Join(t.TempDir(), "styled-table.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	text, err := reopened.PageText(0)
	if err != nil || !slices.Equal(text, []string{"A"}) {
		t.Fatalf("reopened table: %v, %v", text, err)
	}
	objects, err = reopened.PageTextObjects(0)
	if err != nil || len(objects) != 1 || objects[0].Weight != 700 || !objects[0].Italic {
		t.Fatalf("reopened table typography: %v, %v", objects, err)
	}
	paths, err = reopened.PagePaths(0)
	if err != nil || len(paths) != 4 {
		t.Fatalf("reopened table decorations: %v, %v", paths, err)
	}
}

func TestTableCellImageSizesRowAndRoundTrips(t *testing.T) {
	source, err := New()
	if err != nil {
		t.Fatal(err)
	}
	if err := source.AddBlankPage(10, 10); err != nil {
		t.Fatal(err)
	}
	png, err := source.PagePNG(0, 25.4)
	source.Close()
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := DefaultTableStyle()
	style.PageSize = [2]float64{20, 20}
	style.Margins = [4]float64{2, 2, 2, 2}
	style.ColumnWidths = []float64{12}
	style.RowHeight, style.FontSize, style.LineHeight = 0, 3, 3
	cells := []TableCell{{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1, Text: "C",
		Image: &TableCellImage{Data: png, Format: "png", Width: 8, Height: 6}}}
	if indexes, err := doc.AddTableCells(0, 1, cells, style); err != nil || !slices.Equal(indexes, []int{0}) {
		t.Fatalf("image table: %v, %v", indexes, err)
	}
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 || !slices.Equal(images[0].Boundary, []float64{3, 7, 8, 6}) {
		t.Fatalf("image boundary: %v, %v", images, err)
	}
	cells[0].Image.Width = 11
	if _, err := doc.AddTableCells(0, 1, cells, style); err == nil {
		t.Fatal("expected invalid image width")
	}
	path := filepath.Join(t.TempDir(), "image-table.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	images, err = reopened.PageImages(0)
	if err != nil || len(images) != 1 {
		t.Fatalf("reopened image: %v, %v", images, err)
	}
	issues, err := reopened.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("image references: %v, %v", issues, err)
	}
}

func TestOrderedTableCellBlocks(t *testing.T) {
	source, err := New()
	if err != nil {
		t.Fatal(err)
	}
	if err := source.AddBlankPage(10, 10); err != nil {
		t.Fatal(err)
	}
	png, err := source.PagePNG(0, 25.4)
	source.Close()
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := DefaultTableStyle()
	style.PageSize = [2]float64{40, 40}
	style.Margins = [4]float64{2, 2, 2, 2}
	style.ColumnWidths = []float64{20}
	style.RowHeight, style.FontSize, style.LineHeight = 0, 3, 3
	color := [3]uint8{4, 5, 6}
	cells := []TableCell{{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1, Align: "center", Blocks: []TableCellBlock{
		{Type: "text", Text: "A", Align: "left"},
		{Type: "image", Data: png, Format: "png", Width: 6, Height: 4},
		{Type: "text", Text: "B", TextColor: &color, Align: "right"},
		{Type: "image", Data: png, Format: "png", Width: 8, Height: 5},
	}}}
	if indexes, err := doc.AddTableCells(0, 1, cells, style); err != nil || !slices.Equal(indexes, []int{0}) {
		t.Fatalf("block table: %v, %v", indexes, err)
	}
	text, err := doc.PageText(0)
	if err != nil || !slices.Equal(text, []string{"A", "B"}) {
		t.Fatalf("block text: %v, %v", text, err)
	}
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 2 || !slices.Equal(images[0].Boundary, []float64{9, 7, 6, 4}) || !slices.Equal(images[1].Boundary, []float64{8, 16, 8, 5}) {
		t.Fatalf("block images: %v, %v", images, err)
	}
	cells[0].Text = "legacy"
	if _, err := doc.AddTableCells(0, 1, cells, style); err == nil {
		t.Fatal("expected mixed legacy content error")
	}
	path := filepath.Join(t.TempDir(), "block-table.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	images, err = reopened.PageImages(0)
	if err != nil || len(images) != 2 {
		t.Fatalf("reopened block images: %v, %v", images, err)
	}
}

func TestTableSpacerAndRuleBlocks(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := DefaultTableStyle()
	style.PageSize = [2]float64{40, 40}
	style.Margins = [4]float64{2, 2, 2, 2}
	style.ColumnWidths = []float64{20}
	style.RowHeight, style.FontSize, style.LineHeight = 0, 3, 3
	red := [3]uint8{255, 0, 0}
	cells := []TableCell{{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1, Blocks: []TableCellBlock{
		{Type: "spacer", Height: 3},
		{Type: "rule", Thickness: 1, Color: &red},
		{Type: "spacer", Height: 2},
	}}}
	if indexes, err := doc.AddTableCells(0, 1, cells, style); err != nil || !slices.Equal(indexes, []int{0}) {
		t.Fatalf("table blocks: %v, %v", indexes, err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 2 || !slices.Equal(paths[1].Boundary, []float64{3, 7, 18, 1}) {
		t.Fatalf("rule geometry: %v, %v", paths, err)
	}
	invalid := []TableCell{{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1, Blocks: []TableCellBlock{{Type: "spacer", Height: 0}}}}
	if _, err := doc.AddTableCells(0, 1, invalid, style); err == nil {
		t.Fatal("expected invalid spacer height error")
	}
	file := filepath.Join(t.TempDir(), "rule-table.ofd")
	if err := doc.Save(file); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(file)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	paths, err = reopened.PagePaths(0)
	if err != nil || len(paths) != 2 {
		t.Fatalf("reopened rule: %v, %v", paths, err)
	}
}

func TestNestedTableBlocks(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := DefaultTableStyle()
	style.PageSize = [2]float64{40, 40}
	style.Margins = [4]float64{2, 2, 2, 2}
	style.ColumnWidths = []float64{20}
	style.RowHeight, style.FontSize, style.LineHeight = 0, 3, 3
	inner := &NestedTable{RowCount: 1, ColumnWidths: []float64{7, 7}, RowHeight: 10, Cells: []TableCell{
		{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1, Text: "A"},
		{Row: 0, Column: 1, RowSpan: 1, ColumnSpan: 1, Text: "B", VerticalAlign: "bottom"},
	}}
	cells := []TableCell{{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1,
		Blocks: []TableCellBlock{{Type: "text", Text: "Before"}, {Type: "table", Table: inner}, {Type: "text", Text: "After"}}}}
	if indexes, err := doc.AddTableCells(0, 1, cells, style); err != nil || !slices.Equal(indexes, []int{0}) {
		t.Fatalf("nested table: %v, %v", indexes, err)
	}
	texts, err := doc.PageText(0)
	if err != nil || !slices.Equal(texts, []string{"Before", "A", "B", "After"}) {
		t.Fatalf("nested table text: %v, %v", texts, err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 4 || objects[2].Boundary[1] != 13 {
		t.Fatalf("nested table vertical alignment: %v, %v", objects, err)
	}
	inner.Cells = inner.Cells[:1]
	if _, err := doc.AddTableCells(0, 1, cells, style); err == nil {
		t.Fatal("expected uncovered nested grid error")
	}
	if count, err := doc.PageCount(); err != nil || count != 1 {
		t.Fatalf("invalid nested table changed document: %v, %v", count, err)
	}
}

func TestImageOnlyTableWithoutText(t *testing.T) {
	source, err := New()
	if err != nil {
		t.Fatal(err)
	}
	if err := source.AddBlankPage(10, 10); err != nil {
		t.Fatal(err)
	}
	png, err := source.PagePNG(0, 25.4)
	source.Close()
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := DefaultTableStyle()
	style.PageSize = [2]float64{20, 20}
	style.Margins = [4]float64{2, 2, 2, 2}
	style.ColumnWidths = []float64{12}
	style.RowHeight, style.FontSize, style.LineHeight = 0, 3, 3
	cells := []TableCell{{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1, Blocks: []TableCellBlock{
		{Type: "image", Data: png, Format: "png", Width: 8, Height: 6},
	}}}
	if indexes, err := doc.AddTableCells(0, 1, cells, style); err != nil || !slices.Equal(indexes, []int{0}) {
		t.Fatalf("image-only table: %v, %v", indexes, err)
	}
	text, err := doc.PageText(0)
	if err != nil || len(text) != 0 {
		t.Fatalf("image-only text: %v, %v", text, err)
	}
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 {
		t.Fatalf("image-only content: %v, %v", images, err)
	}
}

func TestTableCellTextGeometry(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := DefaultTableStyle()
	style.PageSize = [2]float64{30, 50}
	style.Margins = [4]float64{2, 2, 2, 2}
	style.ColumnWidths = []float64{20}
	style.RowHeight, style.FontSize, style.LineHeight = 0, 3, 4
	fontSize, lineHeight := 5.0, 7.0
	cell := TableCell{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1,
		Text: "A\nB", FontSize: &fontSize, LineHeight: &lineHeight}
	if indexes, err := doc.AddTableCells(0, 1, []TableCell{cell}, style); err != nil || !slices.Equal(indexes, []int{0}) {
		t.Fatalf("table: %v, %v", indexes, err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 2 || objects[0].Size == nil || *objects[0].Size != 5 || objects[1].Boundary[1]-objects[0].Boundary[1] != 7 {
		t.Fatalf("text geometry: %v, %v", objects, err)
	}
	invalid := 4.0
	cell.LineHeight = &invalid
	if _, err := doc.AddTableCells(0, 1, []TableCell{cell}, style); err == nil {
		t.Fatal("expected invalid line height")
	}
	if count, err := doc.PageCount(); err != nil || count != 1 {
		t.Fatalf("invalid table changed document: %v, %v", count, err)
	}
}

func TestTableCellEdgePadding(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := DefaultTableStyle()
	style.PageSize = [2]float64{30, 30}
	style.Margins = [4]float64{2, 2, 2, 2}
	style.ColumnWidths = []float64{20}
	style.RowHeight, style.FontSize, style.LineHeight, style.Padding = 0, 3, 4, 1
	padding := [4]float64{2, 3, 4, 5}
	cell := TableCell{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1, Text: "A\nB", Padding: &padding}
	if _, err := doc.AddTableCells(0, 1, []TableCell{cell}, style); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 2 || objects[0].Boundary[0] != 7 || objects[0].Boundary[1] != 4 || objects[1].Boundary[1] != 8 {
		t.Fatalf("padded text: %v, %v", objects, err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 1 || paths[0].Boundary[3] != 14 {
		t.Fatalf("padded row height: %v, %v", paths, err)
	}
	for _, bad := range [][4]float64{{1, 10, 1, 10}, {-1, 1, 1, 1}} {
		cell.Padding = &bad
		if _, err := doc.AddTableCells(0, 1, []TableCell{cell}, style); err == nil {
			t.Fatalf("expected invalid padding: %v", bad)
		}
	}
	if count, err := doc.PageCount(); err != nil || count != 1 {
		t.Fatalf("invalid padding changed document: %v, %v", count, err)
	}
}

func TestTableCellBorderOverrideAndOmission(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := DefaultTableStyle()
	style.PageSize = [2]float64{50, 20}
	style.Margins = [4]float64{2, 2, 2, 2}
	style.ColumnWidths = []float64{20, 20}
	style.RowHeight, style.FontSize, style.LineHeight = 8, 3, 3
	wide, none := 0.8, 0.0
	red := [3]uint8{255, 0, 0}
	cells := []TableCell{
		{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1, Text: "A", BorderWidth: &wide, BorderColor: &red},
		{Row: 0, Column: 1, RowSpan: 1, ColumnSpan: 1, Text: "B", BorderWidth: &none},
	}
	if _, err := doc.AddTableCells(0, 1, cells, style); err != nil {
		t.Fatal(err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 1 {
		t.Fatalf("border paths: %v, %v", paths, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "#ff0000") {
		t.Fatalf("border SVG: %v", err)
	}
	negative := -1.0
	cells[0].BorderWidth = &negative
	if _, err := doc.AddTableCells(0, 1, cells, style); err == nil {
		t.Fatal("expected invalid border width")
	}
	if count, err := doc.PageCount(); err != nil || count != 1 {
		t.Fatalf("invalid border changed document: %v, %v", count, err)
	}
}

func TestTableCellLetterSpacingSetsTextDeltas(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := DefaultTableStyle()
	style.PageSize = [2]float64{50, 20}
	style.Margins = [4]float64{2, 2, 2, 2}
	style.ColumnWidths = []float64{20, 20}
	style.RowHeight, style.FontSize, style.LineHeight = 8, 3, 4
	spacing := 1.0
	cells := []TableCell{
		{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1, Text: "ABCD"},
		{Row: 0, Column: 1, RowSpan: 1, ColumnSpan: 1, Text: "ABCD", LetterSpacing: &spacing},
	}
	if _, err := doc.AddTableCells(0, 1, cells, style); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 2 || math.Abs(objects[1].Boundary[2]-objects[0].Boundary[2]-3) > 1e-9 ||
		objects[0].Codes[0].DeltaX == nil || len(strings.Fields(*objects[0].Codes[0].DeltaX)) != 3 || objects[1].Codes[0].DeltaX == nil ||
		len(strings.Fields(*objects[1].Codes[0].DeltaX)) != 3 {
		t.Fatalf("letter spacing: %v, %v", objects, err)
	}
	cells[0].Text, cells[1].Text = "e\u0301X", "e\u0301X"
	if _, err := doc.AddTableCells(0, 1, cells, style); err != nil {
		t.Fatal(err)
	}
	unicodeObjects, err := doc.PageTextObjects(1)
	if err != nil || len(unicodeObjects) != 2 ||
		math.Abs(unicodeObjects[1].Boundary[2]-unicodeObjects[0].Boundary[2]-1) > 1e-9 ||
		unicodeObjects[1].Codes[0].DeltaX == nil ||
		!strings.HasPrefix(*unicodeObjects[1].Codes[0].DeltaX, "0 ") {
		t.Fatalf("grapheme spacing: %v, %v", unicodeObjects, err)
	}
	tooNarrow := -100.0
	cells[1].LetterSpacing = &tooNarrow
	if _, err := doc.AddTableCells(0, 1, cells, style); err == nil {
		t.Fatal("expected invalid letter spacing")
	}
	if count, err := doc.PageCount(); err != nil || count != 2 {
		t.Fatalf("invalid letter spacing changed document: %v, %v", count, err)
	}
	path := filepath.Join(t.TempDir(), "grapheme-spacing.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	texts, err := reopened.PageText(1)
	if err != nil || !slices.Equal(texts, []string{"e\u0301X", "e\u0301X"}) {
		t.Fatalf("saved grapheme text: %v, %v", texts, err)
	}
}

func TestTableCellsSelectFontResources(t *testing.T) {
	font, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-sans.ttf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	serif, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-serif.ttf"))
	if err != nil {
		t.Fatal(err)
	}
	defaultFont, err := doc.AddEmbeddedFont("DejaVu Serif", serif)
	if err != nil {
		t.Fatal(err)
	}
	selected, err := doc.AddEmbeddedFont("DejaVu Sans", font)
	if err != nil {
		t.Fatal(err)
	}
	style := DefaultTableStyle()
	style.FontID = strconv.Itoa(defaultFont)
	style.PageSize = [2]float64{50, 30}
	style.Margins = [4]float64{2, 2, 2, 2}
	style.ColumnWidths = []float64{20, 20}
	style.RowHeight, style.FontSize, style.LineHeight = 0, 3, 4
	cells := []TableCell{
		{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1, Text: "WWWW"},
		{Row: 0, Column: 1, RowSpan: 1, ColumnSpan: 1, Text: "WWWW", FontID: strconv.Itoa(selected)},
	}
	if _, err := doc.AddTableCells(0, 1, cells, style); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 2 || objects[0].FontID == nil || objects[1].FontID == nil ||
		*objects[0].FontID != strconv.Itoa(defaultFont) || *objects[1].FontID != strconv.Itoa(selected) ||
		objects[0].Boundary[2] == objects[1].Boundary[2] {
		t.Fatalf("selected table fonts: %v, %v", objects, err)
	}
	cells[0].FontID = "999999"
	if _, err := doc.AddTableCells(0, 1, cells, style); err == nil {
		t.Fatal("expected unknown font error")
	}
	if count, err := doc.PageCount(); err != nil || count != 1 {
		t.Fatalf("invalid font changed document: %v, %v", count, err)
	}
}

func TestPromotesAndAppliesPageTemplate(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("shared", 30, 20); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBlankPage(30, 20); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("objects: %v, %v", objects, err)
	}
	objectID := *objects[0].ID
	if err := doc.AddPageObjectURIAction(0, objectID, "https://example.org/template"); err != nil {
		t.Fatal(err)
	}
	id, err := doc.PromotePageToTemplate(0, []int{1}, "Background")
	if err != nil {
		t.Fatal(err)
	}
	text, err := doc.PageTextWithTemplates(1)
	if err != nil || !slices.Equal(text, []string{"shared"}) {
		t.Fatalf("template text: %v, %v", text, err)
	}
	templateSources, err := doc.PageSources(1)
	if err != nil || len(templateSources) != 2 {
		t.Fatalf("template sources: %v, %v", templateSources, err)
	}
	actions, err := doc.PageSourceObjectActions(1, templateSources[0].Path, objectID)
	if err != nil || len(actions) != 1 || actions[0].Attributes["URI"] != "https://example.org/template" {
		t.Fatalf("template actions: %v, %v", actions, err)
	}
	if _, err := doc.PageSourceObjectActions(1, "OFD.xml", objectID); err == nil {
		t.Fatal("expected unrelated source error")
	}
	if err := doc.AddBlankPage(30, 20); err != nil {
		t.Fatal(err)
	}
	if err := doc.ApplyPageTemplate(2, fmt.Sprint(id), "Foreground"); err != nil {
		t.Fatal(err)
	}
	pdf, err := doc.ToPDF()
	if err != nil || bytes.Count(pdf, []byte("/Subtype /Link")) != 3 {
		t.Fatalf("template PDF links: %v", err)
	}
	sources, err := doc.PageSources(2)
	if err != nil || len(sources) != 2 || sources[0].TemplateID == nil || *sources[0].TemplateID != fmt.Sprint(id) {
		t.Fatalf("template source: %v, %v", sources, err)
	}
	if err := doc.ApplyPageTemplate(2, fmt.Sprint(id), "Foreground"); err == nil {
		t.Fatal("expected duplicate template error")
	}
	if _, err := doc.PromotePageToTemplate(0, []int{-1}, "Background"); err == nil {
		t.Fatal("expected negative page")
	}
	if _, err := doc.PromotePageToTemplate(-1, nil, "Background"); err == nil {
		t.Fatal("expected negative source page")
	}
	if _, err := doc.PromotePageToTemplate(0, nil, "bad"); err == nil {
		t.Fatal("expected invalid ZOrder")
	}
	if _, err := doc.PromotePageToTemplate(0, nil, "bad\x00order"); err == nil {
		t.Fatal("expected invalid ZOrder string")
	}
	if err := doc.ApplyPageTemplate(-1, fmt.Sprint(id), "Background"); err == nil {
		t.Fatal("expected negative page")
	}
	if err := doc.ApplyPageTemplate(2, "bad\x00id", "Background"); err == nil {
		t.Fatal("expected invalid template ID string")
	}
	if err := doc.ApplyPageTemplate(2, fmt.Sprint(id), "bad\x00order"); err == nil {
		t.Fatal("expected invalid ZOrder string")
	}
	if err := doc.AddTextPage("second source", 30, 20); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.PromotePageToTemplate(3, nil, "Background"); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "template.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	text, err = reopened.PageTextWithTemplates(2)
	if err != nil || !slices.Equal(text, []string{"shared"}) {
		t.Fatalf("reopened template: %v, %v", text, err)
	}
	doc.Close()
	if err := doc.ApplyPageTemplate(2, fmt.Sprint(id), "Background"); err == nil {
		t.Fatal("expected closed document")
	}
}

func TestReportsMissingFontInTemplatePage(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("template", 30, 20); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBlankPage(30, 20); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.PromotePageToTemplate(0, []int{1}, "Background"); err != nil {
		t.Fatal(err)
	}
	sources, err := doc.PageSources(1)
	if err != nil || len(sources) != 2 {
		t.Fatalf("template sources: %v, %v", sources, err)
	}
	xml, err := doc.Entry(sources[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	font := regexp.MustCompile(`Font="[^"]+"`)
	if !font.Match(xml) {
		t.Fatal("template has no font")
	}
	broken := font.ReplaceAll(xml, []byte(`Font="999999"`))
	if err := doc.SetEntry(sources[0].Path, broken); err != nil {
		t.Fatal(err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	for _, issue := range issues {
		if issue.Path == sources[0].Path && issue.Code == "missing_font" {
			return
		}
	}
	t.Fatalf("missing template font issue: %+v", issues)
}

func TestRemovesTemplateReferenceAndDefinition(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("shared", 30, 20); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBlankPage(30, 20); err != nil {
		t.Fatal(err)
	}
	id, err := doc.PromotePageToTemplate(0, []int{1}, "Background")
	if err != nil {
		t.Fatal(err)
	}
	name := fmt.Sprint(id)
	if err := doc.RemoveDocumentTemplate(0, name); err == nil {
		t.Fatal("expected in-use error")
	}
	if err := doc.RemovePageTemplate(-1, name); err == nil {
		t.Fatal("expected negative page")
	}
	if err := doc.RemoveDocumentTemplate(-1, name); err == nil {
		t.Fatal("expected negative document")
	}
	if err := doc.RemovePageTemplate(1, "bad\x00id"); err == nil {
		t.Fatal("expected invalid ID")
	}
	if err := doc.RemoveDocumentTemplate(0, "bad\x00id"); err == nil {
		t.Fatal("expected invalid ID")
	}
	if err := doc.RemovePageTemplate(1, "missing"); err == nil {
		t.Fatal("expected missing reference")
	}
	if err := doc.RemovePageTemplate(1, name); err != nil {
		t.Fatal(err)
	}
	text, err := doc.PageTextWithTemplates(1)
	if err != nil || len(text) != 0 {
		t.Fatalf("page still uses template: %v, %v", text, err)
	}
	if err := doc.RemoveDocumentTemplate(0, name); err == nil {
		t.Fatal("expected in-use error")
	}
	if err := doc.RemovePageTemplate(0, name); err != nil {
		t.Fatal(err)
	}
	if err := doc.RemoveDocumentTemplate(0, name); err != nil {
		t.Fatal(err)
	}
	if err := doc.ApplyPageTemplate(1, name, "Background"); err == nil {
		t.Fatal("expected missing template")
	}
	if err := doc.RemoveDocumentTemplate(0, name); err == nil {
		t.Fatal("expected missing definition")
	}
	if err := doc.RemoveDocumentTemplate(1, name); err == nil {
		t.Fatal("expected invalid document")
	}
	path := filepath.Join(t.TempDir(), "removed-template.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	text, err = reopened.PageTextWithTemplates(0)
	if err != nil || len(text) != 0 {
		t.Fatalf("reopened page still uses template: %v, %v", text, err)
	}
}

func TestRemovesAnnotationWithoutRemovingOtherMarks(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("body", 210, 297); err != nil {
		t.Fatal(err)
	}
	first, err := doc.AddTextWatermark(0, "first", [4]float64{10, 20, 40, 8}, 4)
	if err != nil {
		t.Fatal(err)
	}
	second, err := doc.AddTextWatermark(0, "second", [4]float64{20, 30, 40, 8}, 4)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.RemoveAnnotation(-1, fmt.Sprint(first)); err == nil {
		t.Fatal("expected negative page error")
	}
	if err := doc.RemoveAnnotation(0, "bad\x00id"); err == nil {
		t.Fatal("expected invalid ID")
	}
	if err := doc.RemoveAnnotation(0, "missing"); err == nil {
		t.Fatal("expected missing annotation")
	}
	if err := doc.RemoveAnnotation(0, fmt.Sprint(first)); err != nil {
		t.Fatal(err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 || annotations[0].ID != fmt.Sprint(second) {
		t.Fatalf("remaining annotations: %v, %v", annotations, err)
	}
	if err := doc.RemoveAnnotation(0, fmt.Sprint(first)); err == nil {
		t.Fatal("expected already removed error")
	}
	path := filepath.Join(t.TempDir(), "removed-annotation.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	annotations, err = reopened.Annotations()
	if err != nil || len(annotations) != 1 || annotations[0].ID != fmt.Sprint(second) {
		t.Fatalf("reopened annotations: %v, %v", annotations, err)
	}
}

func TestAddsURIActionToPageObject(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("link", 100, 60); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("objects: %v, %v", objects, err)
	}
	id := *objects[0].ID
	if err := doc.AddPageObjectURIAction(-1, id, "https://example.org"); err == nil {
		t.Fatal("expected negative page error")
	}
	if _, err := doc.PageObjectActions(-1, id); err == nil {
		t.Fatal("expected negative page error")
	}
	if err := doc.AddPageObjectURIAction(0, id, "bad\x00uri"); err == nil {
		t.Fatal("expected NUL error")
	}
	if _, err := doc.PageObjectActions(0, "bad\x00id"); err == nil {
		t.Fatal("expected NUL error")
	}
	if err := doc.AddPageObjectURIAction(0, id, ""); err == nil {
		t.Fatal("expected empty URI error")
	}
	if err := doc.AddPageObjectURIAction(0, id, "https://example.org/?a=1&b=2"); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddPageObjectURIAction(0, id, "mailto:test@example.org"); err != nil {
		t.Fatal(err)
	}
	actions, err := doc.PageObjectActions(0, id)
	if err != nil || len(actions) != 2 || actions[0].Event != "CLICK" || actions[0].Kind != "URI" || actions[0].Attributes["URI"] != "https://example.org/?a=1&b=2" {
		t.Fatalf("actions: %v, %v", actions, err)
	}
	pdf, err := doc.ToPDF()
	if err != nil || bytes.Count(pdf, []byte("/Subtype /Link")) != 1 {
		t.Fatalf("PDF object link: %v", err)
	}
	path := filepath.Join(t.TempDir(), "uri-action.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	actions, err = reopened.PageObjectActions(0, id)
	if err != nil || len(actions) != 2 {
		t.Fatalf("reopened actions: %v, %v", actions, err)
	}
}

func TestActionRegionsRoundTripAndPDFLinks(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("link", 100, 60); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("objects: %v, %v", objects, err)
	}
	id := *objects[0].ID
	if err := doc.AddPageObjectURIAction(0, id, "https://example.org"); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectActionRegions(0, id, -1, nil); err == nil {
		t.Fatal("expected negative index error")
	}
	if err := doc.SetPageObjectActionRegions(0, id, 0, [][4]float64{{2, 3, -1, 5}}); err == nil {
		t.Fatal("expected invalid rectangle error")
	}
	if err := doc.SetPageObjectActionRegions(0, id, 0, [][4]float64{{2, 3, 4, 5}, {10, 12, 3, 2}}); err != nil {
		t.Fatal(err)
	}
	actions, err := doc.PageObjectActions(0, id)
	if err != nil || len(actions) != 1 || actions[0].Region == nil || len(actions[0].Region.Areas) != 2 {
		t.Fatalf("regions: %v, %v", actions, err)
	}
	if actions[0].Region.Areas[0].Start != [2]float64{2, 3} {
		t.Fatalf("first area: %v", actions[0].Region.Areas[0])
	}
	pdf, err := doc.ToPDF()
	if err != nil || bytes.Count(pdf, []byte("/Subtype /Link")) != 1 || bytes.Count(pdf, []byte("/QuadPoints")) != 1 {
		t.Fatalf("PDF links: %v", err)
	}
	path := filepath.Join(t.TempDir(), "regions.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	reopenedActions, err := reopened.PageObjectActions(0, id)
	if err != nil || len(reopenedActions) != 1 || reopenedActions[0].Region == nil || len(reopenedActions[0].Region.Areas) != 2 {
		t.Fatalf("reopened regions: %v, %v", reopenedActions, err)
	}
	if err := reopened.SetPageObjectActionRegions(0, id, 0, nil); err != nil {
		t.Fatal(err)
	}
	reopenedActions, err = reopened.PageObjectActions(0, id)
	if err != nil || reopenedActions[0].Region != nil {
		t.Fatalf("cleared regions: %v, %v", reopenedActions, err)
	}
}

func TestAddsGotoActionToPageObject(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("jump", 100, 60); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBlankPage(100, 60); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("objects: %v, %v", objects, err)
	}
	pages, err := doc.Pages()
	if err != nil || len(pages) != 2 {
		t.Fatalf("pages: %v, %v", pages, err)
	}
	left, top, zoom := 12.5, 21.0, 1.5
	destination := Destination{PageID: pages[1].ID, Kind: "XYZ", Left: &left, Top: &top, Zoom: &zoom}
	if err := doc.AddPageObjectGotoAction(-1, *objects[0].ID, destination); err == nil {
		t.Fatal("expected negative page")
	}
	if err := doc.AddPageObjectGotoAction(0, "bad\x00id", destination); err == nil {
		t.Fatal("expected invalid object ID")
	}
	bad := destination
	bad.PageID = "missing"
	if err := doc.AddPageObjectGotoAction(0, *objects[0].ID, bad); err == nil {
		t.Fatal("expected invalid target page")
	}
	bad = destination
	bad.Left = new(float64)
	*bad.Left = math.NaN()
	if err := doc.AddPageObjectGotoAction(0, *objects[0].ID, bad); err == nil {
		t.Fatal("expected NaN")
	}
	if err := doc.AddPageObjectGotoAction(0, *objects[0].ID, destination); err != nil {
		t.Fatal(err)
	}
	actions, err := doc.PageObjectActions(0, *objects[0].ID)
	if err != nil || len(actions) != 1 || actions[0].Kind != "Goto" || actions[0].Destination == nil || actions[0].Destination.PageID != destination.PageID || actions[0].Destination.Left == nil || *actions[0].Destination.Left != left {
		t.Fatalf("actions: %v, %v", actions, err)
	}
	pdf, err := doc.ToPDF()
	if err != nil || bytes.Count(pdf, []byte("/Subtype /Link")) != 1 {
		t.Fatalf("PDF object GoTo link: %v", err)
	}
	path := filepath.Join(t.TempDir(), "goto-action.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	actions, err = reopened.PageObjectActions(0, *objects[0].ID)
	if err != nil || len(actions) != 1 || actions[0].Destination == nil {
		t.Fatalf("reopened actions: %v, %v", actions, err)
	}
}

func TestAddsAttachmentActionsAndValidatesTargets(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("click", 40, 30); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddAttachment("note.txt", []byte("hello")); err != nil {
		t.Fatal(err)
	}
	attachments, err := doc.Attachments()
	if err != nil || len(attachments) != 1 {
		t.Fatalf("attachments: %v, %v", attachments, err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("objects: %v, %v", objects, err)
	}
	window := true
	if err := doc.AddPageObjectAttachmentAction(0, *objects[0].ID, attachments[0].ID, &window); err != nil {
		t.Fatal(err)
	}
	window = false
	annotationID, err := doc.AddAttachmentLinkAnnotation(0, attachments[0].ID, [4]float64{10, 1, 5, 5}, &window)
	if err != nil {
		t.Fatal(err)
	}
	actions, err := doc.PageObjectActions(0, *objects[0].ID)
	if err != nil || len(actions) != 1 || actions[0].Kind != "GotoA" || actions[0].Attributes["NewWindow"] != "true" {
		t.Fatalf("actions: %v, %v", actions, err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 || annotations[0].ID != strconv.Itoa(annotationID) || annotations[0].Actions[0].Kind != "GotoA" {
		t.Fatalf("annotations: %v, %v", annotations, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
	if err := doc.AddPageObjectAttachmentAction(0, *objects[0].ID, "missing", nil); err == nil {
		t.Fatal("expected missing attachment")
	}
	if _, err := doc.AddAttachmentLinkAnnotation(0, "missing", [4]float64{1, 1, 2, 2}, nil); err == nil {
		t.Fatal("expected missing attachment link")
	}
}

func TestAddsAudioVideoResourcesAndPlaybackActions(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("click", 50, 40); err != nil {
		t.Fatal(err)
	}
	audioID, err := doc.AddMultimediaResource(0, "Audio", "MP3", []byte("audio bytes"))
	if err != nil {
		t.Fatal(err)
	}
	videoID, err := doc.AddMultimediaResource(0, "Video", "MP4", []byte("video bytes"))
	if err != nil {
		t.Fatal(err)
	}
	resources, err := doc.PageMultimediaResources(0)
	if err != nil || len(resources) != 2 || resources[0].ID != strconv.Itoa(audioID) || resources[0].MediaType == nil || *resources[0].MediaType != "Audio" || resources[0].Size != 11 || resources[1].ID != strconv.Itoa(videoID) {
		t.Fatalf("resources: %v, %v", resources, err)
	}
	documentResources, err := doc.DocumentMultimediaResources(0)
	if err != nil || len(documentResources) != 2 || documentResources[0].ID != resources[0].ID {
		t.Fatalf("document resources: %v, %v", documentResources, err)
	}
	documentData, err := doc.DocumentMultimediaData(0, strconv.Itoa(audioID))
	if err != nil || string(documentData) != "audio bytes" {
		t.Fatalf("document audio data: %q, %v", documentData, err)
	}
	if _, err := doc.DocumentMultimediaData(0, "missing"); err == nil {
		t.Fatal("expected missing document multimedia resource")
	}
	if _, err := doc.DocumentMultimediaResources(-1); err == nil {
		t.Fatal("expected negative document index")
	}
	data, err := doc.PageMultimediaData(0, strconv.Itoa(audioID))
	if err != nil || string(data) != "audio bytes" {
		t.Fatalf("audio data: %q, %v", data, err)
	}
	if _, err := doc.PageMultimediaData(0, "missing"); err == nil {
		t.Fatal("expected missing multimedia resource")
	}
	if _, err := doc.PageMultimediaResources(-1); err == nil {
		t.Fatal("expected negative page index")
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("objects: %v, %v", objects, err)
	}
	id := *objects[0].ID
	volume := uint8(75)
	repeat := true
	if err := doc.AddPageObjectSoundAction(0, id, strconv.Itoa(audioID), SoundActionOptions{Volume: &volume, Repeat: &repeat}); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddPageObjectMovieAction(0, id, strconv.Itoa(videoID), "Pause"); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddSoundLinkAnnotation(0, strconv.Itoa(audioID), [4]float64{10, 1, 5, 5}, SoundActionOptions{}); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddMovieLinkAnnotation(0, strconv.Itoa(videoID), [4]float64{20, 1, 5, 5}, ""); err != nil {
		t.Fatal(err)
	}
	actions, err := doc.PageObjectActions(0, id)
	if err != nil || len(actions) != 2 || actions[0].Kind != "Sound" || actions[1].Kind != "Movie" {
		t.Fatalf("actions: %v, %v", actions, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
	if err := doc.RemoveDocumentMultimediaResource(0, strconv.Itoa(audioID)); err == nil {
		t.Fatal("expected referenced multimedia resource rejection")
	}
	if err := doc.AddPageObjectSoundAction(0, id, strconv.Itoa(videoID), SoundActionOptions{}); err == nil {
		t.Fatal("expected wrong media type")
	}
	if _, err := doc.AddMovieLinkAnnotation(0, strconv.Itoa(videoID), [4]float64{1, 1, 2, 2}, "Bad"); err == nil {
		t.Fatal("expected invalid operator")
	}
}

func TestAddsStandaloneImageMultimediaResource(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(10, 10); err != nil {
		t.Fatal(err)
	}
	png, err := doc.PagePNG(0, 25.4)
	if err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddMultimediaResource(0, "Image", "pNg", png)
	if err != nil {
		t.Fatal(err)
	}
	resources, err := doc.DocumentMultimediaResources(0)
	if err != nil || len(resources) != 1 || resources[0].ID != strconv.Itoa(id) || resources[0].MediaType == nil || *resources[0].MediaType != "Image" || resources[0].Format == nil || *resources[0].Format != "PNG" {
		t.Fatalf("image resources: %v, %v", resources, err)
	}
	data, err := doc.PageMultimediaData(0, strconv.Itoa(id))
	if err != nil || !bytes.Equal(data, png) {
		t.Fatalf("image data mismatch: %v", err)
	}
	if _, err := doc.AddMultimediaResource(0, "Image", "PNG", []byte("bad")); err == nil {
		t.Fatal("expected invalid image")
	}
}

func TestReplacesDocumentMultimediaData(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(10, 10); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddMultimediaResource(0, "Audio", "MP3", []byte("original"))
	if err != nil {
		t.Fatal(err)
	}
	resources, err := doc.DocumentMultimediaResources(0)
	if err != nil || len(resources) != 1 {
		t.Fatalf("resources: %v, %v", resources, err)
	}
	oldPath := resources[0].Path
	resourceID := strconv.Itoa(id)
	if err := doc.ReplaceDocumentMultimediaData(0, resourceID, []byte("updated")); err != nil {
		t.Fatal(err)
	}
	updated, err := doc.DocumentMultimediaData(0, resourceID)
	if err != nil || string(updated) != "updated" {
		t.Fatalf("updated media: %q, %v", updated, err)
	}
	original, err := doc.Entry(oldPath)
	if err != nil || string(original) != "original" {
		t.Fatalf("original media: %q, %v", original, err)
	}
	if err := doc.ReplaceDocumentMultimediaData(0, resourceID, nil); err == nil {
		t.Fatal("expected empty media error")
	}
	if err := doc.ReplaceDocumentMultimediaData(-1, resourceID, []byte("bad")); err == nil {
		t.Fatal("expected negative document index")
	}
	path := filepath.Join(t.TempDir(), "replaced.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	if err := reopened.ReplaceDocumentMultimediaData(0, resourceID, []byte("again")); err != nil {
		t.Fatal(err)
	}
	again, err := reopened.DocumentMultimediaData(0, resourceID)
	if err != nil || string(again) != "again" {
		t.Fatalf("reopened media: %q, %v", again, err)
	}
	if err := reopened.ReplacePageMultimediaData(0, resourceID, []byte("page update")); err != nil {
		t.Fatal(err)
	}
	pageData, err := reopened.PageMultimediaData(0, resourceID)
	if err != nil || string(pageData) != "page update" {
		t.Fatalf("page media: %q, %v", pageData, err)
	}
	if err := reopened.ReplacePageMultimediaData(0, "missing", []byte("bad")); err == nil {
		t.Fatal("expected missing page resource")
	}
	if err := reopened.ReplacePageMultimediaData(-1, resourceID, []byte("bad")); err == nil {
		t.Fatal("expected negative page index")
	}
	resources, err = reopened.DocumentMultimediaResources(0)
	if err != nil || len(resources) != 1 {
		t.Fatalf("resources before removal: %v, %v", resources, err)
	}
	mediaPath := resources[0].Path
	if err := reopened.RemoveDocumentMultimediaResource(0, resourceID); err != nil {
		t.Fatal(err)
	}
	resources, err = reopened.DocumentMultimediaResources(0)
	if err != nil || len(resources) != 0 {
		t.Fatalf("resources after removal: %v, %v", resources, err)
	}
	retained, err := reopened.Entry(mediaPath)
	if err != nil || string(retained) != "page update" {
		t.Fatalf("retained media: %q, %v", retained, err)
	}
	if err := reopened.RemoveDocumentMultimediaResource(-1, resourceID); err == nil {
		t.Fatal("expected negative document index")
	}
}

func TestAddsPageLocalMultimediaResource(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	for range 2 {
		if err := doc.AddBlankPage(10, 10); err != nil {
			t.Fatal(err)
		}
	}
	id, err := doc.AddPageMultimediaResource(0, "Audio", "WAV", []byte("page audio"))
	if err != nil {
		t.Fatal(err)
	}
	documentResources, err := doc.DocumentMultimediaResources(0)
	if err != nil || len(documentResources) != 0 {
		t.Fatalf("document resources: %v, %v", documentResources, err)
	}
	pageResources, err := doc.PageMultimediaResources(0)
	if err != nil || len(pageResources) != 1 || pageResources[0].ID != strconv.Itoa(id) {
		t.Fatalf("page resources: %v, %v", pageResources, err)
	}
	secondResources, err := doc.PageMultimediaResources(1)
	if err != nil || len(secondResources) != 0 {
		t.Fatalf("second page resources: %v, %v", secondResources, err)
	}
	data, err := doc.PageMultimediaData(0, strconv.Itoa(id))
	if err != nil || string(data) != "page audio" {
		t.Fatalf("page media: %q, %v", data, err)
	}
	if _, err := doc.AddPageMultimediaResource(1, "Image", "PNG", []byte("bad")); err == nil {
		t.Fatal("expected invalid image")
	}
	if _, err := doc.AddPageMultimediaResource(-1, "Audio", "WAV", []byte("bad")); err == nil {
		t.Fatal("expected negative page index")
	}
	path := filepath.Join(t.TempDir(), "page-media.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	data, err = reopened.PageMultimediaData(0, strconv.Itoa(id))
	if err != nil || string(data) != "page audio" {
		t.Fatalf("reopened page media: %q, %v", data, err)
	}
	resources, err := reopened.PageMultimediaResources(0)
	if err != nil || len(resources) != 1 {
		t.Fatalf("reopened resources: %v, %v", resources, err)
	}
	if err := reopened.RemovePageMultimediaResource(-1, strconv.Itoa(id)); err == nil {
		t.Fatal("expected negative page index")
	}
	if err := reopened.RemovePageMultimediaResource(1, strconv.Itoa(id)); err == nil {
		t.Fatal("expected resource not on second page")
	}
	if err := reopened.RemovePageMultimediaResource(0, strconv.Itoa(id)); err != nil {
		t.Fatal(err)
	}
	data, err = reopened.Entry(resources[0].Path)
	if err != nil || string(data) != "page audio" {
		t.Fatalf("retained payload: %q, %v", data, err)
	}
	if _, err := reopened.PageMultimediaData(0, strconv.Itoa(id)); err == nil {
		t.Fatal("expected removed resource to be unreadable")
	}
}

func TestReplacingSharedPageMultimediaUsesCopy(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	for range 2 {
		if err := doc.AddBlankPage(10, 10); err != nil {
			t.Fatal(err)
		}
	}
	id, err := doc.AddPageMultimediaResource(0, "Audio", "WAV", []byte("original"))
	if err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	firstDir := pages[0].Path[:strings.LastIndex(pages[0].Path, "/")]
	firstName := firstDir[strings.LastIndex(firstDir, "/")+1:]
	link := fmt.Sprintf("../%s/RustPageRes_%d.xml", firstName, id)
	secondXML, err := doc.Entry(pages[1].Path)
	if err != nil {
		t.Fatal(err)
	}
	updated := strings.Replace(string(secondXML), "<ofd:Content>", "<ofd:PageRes>"+link+"</ofd:PageRes><ofd:Content>", 1)
	if err := doc.SetEntry(pages[1].Path, []byte(updated)); err != nil {
		t.Fatal(err)
	}
	if err := doc.ReplacePageMultimediaData(0, strconv.Itoa(id), []byte("first only")); err != nil {
		t.Fatal(err)
	}
	first, err := doc.PageMultimediaData(0, strconv.Itoa(id))
	if err != nil || string(first) != "first only" {
		t.Fatalf("first page media: %q, %v", first, err)
	}
	second, err := doc.PageMultimediaData(1, strconv.Itoa(id))
	if err != nil || string(second) != "original" {
		t.Fatalf("second page media: %q, %v", second, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
	path := filepath.Join(t.TempDir(), "shared-page-media.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	second, err = reopened.PageMultimediaData(1, strconv.Itoa(id))
	if err != nil || string(second) != "original" {
		t.Fatalf("reopened second page media: %q, %v", second, err)
	}
}

func TestAddsBookmarkActionAndRemovesSelectedAction(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("click", 100, 60); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("objects: %v, %v", objects, err)
	}
	id := *objects[0].ID
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBookmark(0, "Section & One", Destination{PageID: pages[0].ID, Kind: "XYZ"}); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddPageObjectBookmarkAction(-1, id, "Section & One"); err == nil {
		t.Fatal("expected negative page")
	}
	if err := doc.AddPageObjectBookmarkAction(0, id, "bad\x00name"); err == nil {
		t.Fatal("expected NUL name")
	}
	if err := doc.AddPageObjectBookmarkAction(0, id, "missing"); err == nil {
		t.Fatal("expected missing bookmark")
	}
	if err := doc.AddPageObjectURIAction(0, id, "https://example.org"); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddPageObjectBookmarkAction(0, id, "Section & One"); err != nil {
		t.Fatal(err)
	}
	actions, err := doc.PageObjectActions(0, id)
	if err != nil || len(actions) != 2 || actions[1].Bookmark == nil || *actions[1].Bookmark != "Section & One" {
		t.Fatalf("actions: %v, %v", actions, err)
	}
	if err := doc.RemovePageObjectAction(-1, id, 0); err == nil {
		t.Fatal("expected negative page")
	}
	if err := doc.RemovePageObjectAction(0, id, -1); err == nil {
		t.Fatal("expected negative action")
	}
	if err := doc.RemovePageObjectAction(0, "bad\x00id", 0); err == nil {
		t.Fatal("expected NUL ID")
	}
	if err := doc.RemovePageObjectAction(0, id, 9); err == nil {
		t.Fatal("expected out-of-range action")
	}
	if err := doc.RemovePageObjectAction(0, id, 0); err != nil {
		t.Fatal(err)
	}
	actions, err = doc.PageObjectActions(0, id)
	if err != nil || len(actions) != 1 || actions[0].Kind != "Goto" {
		t.Fatalf("remaining action: %v, %v", actions, err)
	}
	path := filepath.Join(t.TempDir(), "bookmark-action.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	actions, err = reopened.PageObjectActions(0, id)
	if err != nil || len(actions) != 1 || actions[0].Bookmark == nil {
		t.Fatalf("reopened actions: %v, %v", actions, err)
	}
}

func TestEditsKeywordsAndRemovesStandardMetadata(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("body", 100, 60); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddDocumentKeyword(-1, "bad"); err == nil {
		t.Fatal("expected negative document index")
	}
	if err := doc.RemoveDocumentKeyword(0, -1); err == nil {
		t.Fatal("expected negative keyword index")
	}
	if err := doc.RemoveDocumentInfoField(-1, "Title"); err == nil {
		t.Fatal("expected negative document index")
	}
	if err := doc.RemoveDocumentInfoField(0, "Title"); err == nil {
		t.Fatal("expected missing title")
	}
	if err := doc.RemoveDocumentInfoField(0, "Title\x00"); err == nil {
		t.Fatal("expected NUL field")
	}
	if err := doc.AddDocumentKeyword(0, "bad\x00keyword"); err == nil {
		t.Fatal("expected NUL keyword")
	}
	if err := doc.AddDocumentKeyword(0, " "); err == nil {
		t.Fatal("expected empty keyword")
	}
	if err := doc.RemoveDocumentKeyword(0, 0); err == nil {
		t.Fatal("expected missing keyword")
	}
	if err := doc.SetDocumentInfoField(0, "Title", "Rust & OFD"); err != nil {
		t.Fatal(err)
	}
	for field, value := range map[string]string{
		"CreationDate": "2024-02-30",
		"ModDate":      "2024-2-3",
		"DocUsage":     "Magazine",
	} {
		if err := doc.SetDocumentInfoField(0, field, value); err == nil {
			t.Fatalf("expected invalid %s to fail", field)
		}
	}
	if err := doc.SetDocumentInfoField(0, "CreationDate", "2024-02-29"); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetDocumentInfoField(0, "DocUsage", "EBook"); err != nil {
		t.Fatal(err)
	}
	const docID = "0123456789abcdef0123456789abcdef"
	if err := doc.SetDocumentInfoField(0, "DocID", "bad"); err == nil {
		t.Fatal("expected malformed document ID error")
	}
	if err := doc.RemoveDocumentInfoField(0, "DocID"); err == nil {
		t.Fatal("expected required document ID removal error")
	}
	if err := doc.SetDocumentInfoField(0, "DocID", docID); err != nil {
		t.Fatal(err)
	}
	for _, keyword := range []string{"票据 & 发票", "duplicate", "duplicate"} {
		if err := doc.AddDocumentKeyword(0, keyword); err != nil {
			t.Fatal(err)
		}
	}
	if err := doc.RemoveDocumentKeyword(0, 1); err != nil {
		t.Fatal(err)
	}
	if err := doc.RemoveDocumentKeyword(0, 2); err == nil {
		t.Fatal("expected out of bounds keyword")
	}
	if err := doc.RemoveDocumentInfoField(0, "Title"); err != nil {
		t.Fatal(err)
	}
	info, err := doc.Documents()
	if err != nil || info[0].DocID != docID || info[0].Title != nil || len(info[0].Keywords) != 2 || info[0].Keywords[0] != "票据 & 发票" || info[0].CreationDate == nil || *info[0].CreationDate != "2024-02-29" || info[0].DocUsage == nil || *info[0].DocUsage != "EBook" {
		t.Fatalf("metadata: %v, %v", info, err)
	}
	path := filepath.Join(t.TempDir(), "keywords.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	info, err = reopened.Documents()
	if err != nil || info[0].DocID != docID || len(info[0].Keywords) != 2 || info[0].Keywords[1] != "duplicate" {
		t.Fatalf("reopened keywords: %v, %v", info, err)
	}
	ofdXML, err := reopened.Entry("OFD.xml")
	if err != nil {
		t.Fatal(err)
	}
	if err := reopened.SetEntry("OFD.xml", bytes.Replace(ofdXML, []byte(docID), []byte("bad"), 1)); err != nil {
		t.Fatal(err)
	}
	issues, err := reopened.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	if !slices.ContainsFunc(issues, func(issue ValidationIssue) bool { return issue.Code == "invalid_doc_id" }) {
		t.Fatalf("missing invalid_doc_id issue: %v", issues)
	}
	if err := reopened.SetEntry("OFD.xml", ofdXML); err != nil {
		t.Fatal(err)
	}
	withoutID := bytes.Replace(ofdXML, []byte("<ofd:DocID>"+docID+"</ofd:DocID>"), nil, 1)
	if bytes.Equal(withoutID, ofdXML) {
		t.Fatal("expected document ID element in package")
	}
	if err := reopened.SetEntry("OFD.xml", withoutID); err != nil {
		t.Fatal(err)
	}
	legacy, err := reopened.Documents()
	if err != nil || len(legacy) != 1 || legacy[0].DocID != "" {
		t.Fatalf("legacy document info: %v, %v", legacy, err)
	}
	if issues, err := reopened.ValidateReferences(); err != nil || len(issues) != 0 {
		t.Fatalf("legacy document validation: %v, %v", issues, err)
	}
	if err := reopened.SetEntry("OFD.xml", ofdXML); err != nil {
		t.Fatal(err)
	}
	doc.Close()
	if err := doc.AddDocumentKeyword(0, "closed"); err == nil {
		t.Fatal("expected closed document error")
	}
	if err := doc.RemoveDocumentKeyword(0, 0); err == nil {
		t.Fatal("expected closed document error")
	}
	if err := doc.RemoveDocumentInfoField(0, "Title"); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestSetsAndRemovesCustomDocumentMetadata(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("body", 100, 60); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetDocumentCustomData(-1, "Key", "value"); err == nil {
		t.Fatal("expected negative document")
	}
	if err := doc.RemoveDocumentCustomData(-1, "Key"); err == nil {
		t.Fatal("expected negative document")
	}
	if err := doc.SetDocumentCustomData(0, "bad\x00name", "value"); err == nil {
		t.Fatal("expected NUL name")
	}
	if err := doc.SetDocumentCustomData(0, "Key", "bad\x00value"); err == nil {
		t.Fatal("expected NUL value")
	}
	if err := doc.RemoveDocumentCustomData(0, "missing"); err == nil {
		t.Fatal("expected missing custom data")
	}
	if err := doc.SetDocumentCustomData(0, "客户&类型", "<VIP>"); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetDocumentCustomData(0, "客户&类型", "更新 & 完成"); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetDocumentCustomData(0, "Other", "keep"); err != nil {
		t.Fatal(err)
	}
	info, err := doc.Documents()
	if err != nil || len(info) != 1 || len(info[0].CustomData) != 2 || info[0].CustomData[0].Value != "更新 & 完成" {
		t.Fatalf("custom data: %v, %v", info, err)
	}
	if err := doc.RemoveDocumentCustomData(0, "客户&类型"); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "custom-metadata.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	info, err = reopened.Documents()
	if err != nil || len(info[0].CustomData) != 1 || info[0].CustomData[0].Name != "Other" {
		t.Fatalf("reopened custom data: %v, %v", info, err)
	}
	doc.Close()
	if err := doc.SetDocumentCustomData(0, "Key", "value"); err == nil {
		t.Fatal("expected closed document error")
	}
	if err := doc.RemoveDocumentCustomData(0, "Other"); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestImportsPDFUserUnitGeometry(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "user-unit.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	pages, err := doc.Pages()
	if err != nil || len(pages) != 1 || !slices.Equal(pages[0].PhysicalBox, []float64{0, 0, 50.8, 50.8}) {
		t.Fatalf("scaled page: %v, %v", pages, err)
	}
	text, err := doc.PageText(0)
	if err != nil || !slices.Equal(text, []string{"Hi"}) {
		t.Fatalf("scaled text: %v, %v", text, err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 2 {
		t.Fatalf("scaled paths: %v, %v", paths, err)
	}
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 {
		t.Fatalf("scaled image: %v, %v", images, err)
	}
	path := filepath.Join(t.TempDir(), "user-unit.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	text, err = reopened.PageText(0)
	if err != nil || !slices.Equal(text, []string{"Hi"}) {
		t.Fatalf("reopened text: %v, %v", text, err)
	}
}

func TestImportsAndExportsStandalonePDFPopup(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "standalone-popup.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 {
		t.Fatalf("annotations: %+v, %v", annotations, err)
	}
	popup := annotations[0]
	if popup.Remark == nil || *popup.Remark != "standalone review" || popup.Parameters["PDFPopupStandalone"] != "true" || popup.Parameters["PDFTextOpen"] != "true" {
		t.Fatalf("popup metadata: %+v", popup)
	}
	if issues, err := doc.ValidateReferences(); err != nil || len(issues) != 0 {
		t.Fatalf("references: %+v, %v", issues, err)
	}
	exported, err := doc.ToPDF()
	if err != nil {
		t.Fatal(err)
	}
	reopened, err := FromPDF(exported)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	again, err := reopened.Annotations()
	if err != nil || len(again) != 1 || again[0].Parameters["PDFPopupStandalone"] != "true" {
		t.Fatalf("reimported popup: %+v, %v", again, err)
	}
}

func TestImportsAndExportsPDFInk(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "ink.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 {
		t.Fatalf("annotations: %+v, %v", annotations, err)
	}
	ink := annotations[0]
	if ink.Subtype == nil || *ink.Subtype != "Ink" || ink.Remark == nil || *ink.Remark != "two pen strokes" {
		t.Fatalf("ink metadata: %+v", ink)
	}
	objects, err := doc.AnnotationObjects(0, ink.ID)
	if err != nil || len(objects) != 2 {
		t.Fatalf("ink strokes: %+v, %v", objects, err)
	}
	if issues, err := doc.ValidateReferences(); err != nil || len(issues) != 0 {
		t.Fatalf("references: %+v, %v", issues, err)
	}
	exported, err := doc.ToPDF()
	if err != nil {
		t.Fatal(err)
	}
	reopened, err := FromPDF(exported)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	again, err := reopened.Annotations()
	if err != nil || len(again) != 1 || again[0].Subtype == nil || *again[0].Subtype != "Ink" {
		t.Fatalf("reimported ink: %+v, %v", again, err)
	}
}

func TestImportsAndExportsPDFPolygonAndPolyLine(t *testing.T) {
	for _, fixture := range []struct {
		name, subtype string
		paths         int
	}{
		{"polygon.pdf", "Polygon", 1},
		{"polyline.pdf", "PolyLine", 3},
	} {
		t.Run(fixture.subtype, func(t *testing.T) {
			data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", fixture.name))
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			annotations, err := doc.Annotations()
			if err != nil || len(annotations) != 1 {
				t.Fatalf("annotations: %+v, %v", annotations, err)
			}
			shape := annotations[0]
			if shape.Subtype == nil || *shape.Subtype != fixture.subtype {
				t.Fatalf("shape: %+v", shape)
			}
			objects, err := doc.AnnotationObjects(0, shape.ID)
			if err != nil || len(objects) != fixture.paths {
				t.Fatalf("paths: %+v, %v", objects, err)
			}
			if issues, err := doc.ValidateReferences(); err != nil || len(issues) != 0 {
				t.Fatalf("references: %+v, %v", issues, err)
			}
			exported, err := doc.ToPDF()
			if err != nil {
				t.Fatal(err)
			}
			reopened, err := FromPDF(exported)
			if err != nil {
				t.Fatal(err)
			}
			defer reopened.Close()
			again, err := reopened.Annotations()
			if err != nil || len(again) != 1 || again[0].Subtype == nil || *again[0].Subtype != fixture.subtype {
				t.Fatalf("reimported shape: %+v, %v", again, err)
			}
		})
	}
}

func TestImportsAndExportsLinkedPDFPopup(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "popup.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 {
		t.Fatalf("annotations: %+v, %v", annotations, err)
	}
	note := annotations[0]
	if note.Subtype == nil || *note.Subtype != "Text" || note.Parameters["PDFPopupOpen"] != "true" {
		t.Fatalf("popup metadata: %+v", note)
	}
	if issues, err := doc.ValidateReferences(); err != nil || len(issues) != 0 {
		t.Fatalf("references: %+v, %v", issues, err)
	}
	exported, err := doc.ToPDF()
	if err != nil {
		t.Fatal(err)
	}
	reopened, err := FromPDF(exported)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	again, err := reopened.Annotations()
	if err != nil || len(again) != 1 || again[0].Parameters["PDFPopupOpen"] != "true" {
		t.Fatalf("reimported popup: %+v, %v", again, err)
	}
}

func TestImportsPDFLinkAnnotations(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "links.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 2 {
		t.Fatalf("annotations: %+v, %v", annotations, err)
	}
	if annotations[0].Actions[0].Attributes["URI"] != "https://example.com/?a=1&b=2" {
		t.Fatalf("URI action: %+v", annotations[0].Actions)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	if annotations[1].Actions[0].Destination.PageID != pages[1].ID {
		t.Fatalf("GoTo action: %+v", annotations[1].Actions)
	}
	pdf, err := doc.ToPDF()
	if err != nil || bytes.Count(pdf, []byte("/Subtype /Link")) != 2 {
		t.Fatalf("PDF links: %v", err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %+v, %v", issues, err)
	}
}

func TestImportsPDFSelectedColorSpacesWithFormResources(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "color-spaces.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 4 {
		t.Fatalf("paths: %d, %v", len(paths), err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil {
		t.Fatal(err)
	}
	for _, color := range []string{"#808080", "#ff0000", "#0000ff", "#00ffff"} {
		if !strings.Contains(svg, color) {
			t.Fatalf("missing color %s", color)
		}
	}
}

func TestImportsPDFCalibratedColors(t *testing.T) {
	for _, space := range []string{"calgray", "calrgb"} {
		t.Run(space, func(t *testing.T) {
			fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", space+".pdf")
			data, err := os.ReadFile(fixture)
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			paths, err := doc.PagePaths(0)
			if err != nil || len(paths) != 1 {
				t.Fatalf("paths: %+v, %v", paths, err)
			}
			svg, err := doc.PageSVG(0)
			if err != nil || !strings.Contains(svg, `#808080`) {
				t.Fatalf("%s SVG: %v, %v", space, svg, err)
			}
			issues, err := doc.ValidateReferences()
			if err != nil || len(issues) != 0 {
				t.Fatalf("references: %v, %v", issues, err)
			}
		})
	}
}

func TestImportsPDFLab(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "lab.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 1 {
		t.Fatalf("paths: %+v, %v", paths, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `#777777`) {
		t.Fatalf("Lab SVG: %v, %v", svg, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
}

func TestImportsPDFLabJPEG(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "lab-jpeg.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 || images[0].Format == nil || *images[0].Format != "PNG" {
		t.Fatalf("images: %+v, %v", images, err)
	}
	imageData, err := doc.PageImageData(0, 0)
	if err != nil || !bytes.HasPrefix(imageData, []byte("\x89PNG\r\n\x1a\n")) {
		t.Fatalf("image bytes: %d, %v", len(imageData), err)
	}
	decoded, err := png.Decode(bytes.NewReader(imageData))
	if err != nil {
		t.Fatal(err)
	}
	red, green, blue, _ := decoded.At(0, 0).RGBA()
	for _, value := range []uint32{red >> 8, green >> 8, blue >> 8} {
		if math.Abs(float64(value)-119) > 5 {
			t.Fatalf("Lab JPEG pixel: %d, %d, %d", red>>8, green>>8, blue>>8)
		}
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
}

func TestImportsPDFUnmarkedYUVK(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "unmarked-yuvk.pdf")
	data, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 || images[0].Format == nil || *images[0].Format != "PNG" {
		t.Fatalf("images: %+v, %v", images, err)
	}
	imageData, err := doc.PageImageData(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := png.Decode(bytes.NewReader(imageData))
	if err != nil {
		t.Fatal(err)
	}
	if decoded.Bounds().Dx() != 2 || decoded.Bounds().Dy() != 1 {
		t.Fatalf("unexpected YUVK image bounds: %v", decoded.Bounds())
	}
	black := decoded.At(0, 0)
	white := decoded.At(1, 0)
	for _, sample := range []struct {
		pixel color.Color
		want  uint32
	}{
		{black, 0},
		{white, 65535},
	} {
		red, green, blue, _ := sample.pixel.RGBA()
		if red != sample.want || green != sample.want || blue != sample.want {
			t.Fatalf("YUVK pixel: %d %d %d, want %d", red, green, blue, sample.want)
		}
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
}

func TestReadsJavaSignedTemplateWithMixedPathCase(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-sign", "src", "test", "resources", "v4signed.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	sources, err := doc.PageSources(0)
	if err != nil || len(sources) < 2 || sources[0].Path != "Doc_0/Tpls/Tpl_0/Content.xml" {
		t.Fatalf("template sources: %v, %v", sources, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.HasPrefix(svg, "<svg ") {
		t.Fatalf("template SVG: %v", err)
	}
}

func TestReadsJavaMediaWithMixedPathCase(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-converter", "src", "test", "resources", "containsJPEG.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	media, err := doc.PageMultimediaResources(0)
	if err != nil || len(media) != 2 || media[0].Path != "DOC_0/Res/Image_4.JPEG" {
		t.Fatalf("media: %v, %v", media, err)
	}
	imageData, err := doc.PageImageData(0, 0)
	if err != nil || !bytes.HasPrefix(imageData, []byte("\xff\xd8")) {
		t.Fatalf("JPEG image: %d bytes, %v", len(imageData), err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
}

func TestDocumentResourcesUseArchiveCaseForReadingAndEditing(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-converter", "src", "test", "resources", "containsJPEG.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	const root = "Doc_0/Document.xml"
	original, err := doc.Entry(root)
	if err != nil {
		t.Fatal(err)
	}
	changed := bytes.Replace(original, []byte(">DocumentRes.xml<"), []byte(">DOCUMENTRES.XML<"), 1)
	changed = bytes.Replace(changed, []byte(">PublicRes.xml<"), []byte(">PUBLICRES.XML<"), 1)
	if bytes.Equal(changed, original) {
		t.Fatal("fixture resource references were not found")
	}
	if err := doc.SetEntry(root, changed); err != nil {
		t.Fatal(err)
	}
	imageData, err := doc.PageImageData(0, 0)
	if err != nil || !bytes.HasPrefix(imageData, []byte("\xff\xd8")) {
		t.Fatalf("JPEG image: %d bytes, %v", len(imageData), err)
	}
	before, err := doc.Entry("Doc_0/PublicRes.xml")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddDocumentDrawParam(0, DrawParamStyle{}); err != nil {
		t.Fatal(err)
	}
	after, err := doc.Entry("Doc_0/PublicRes.xml")
	if err != nil || bytes.Equal(after, before) {
		t.Fatalf("public resource was not updated: %v", err)
	}
	if _, err := doc.Entry("Doc_0/PUBLICRES.XML"); err == nil {
		t.Fatal("unexpected case-variant archive entry")
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
}

func TestImportsPDFIndexedVectorColors(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "indexed-colors.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 3 {
		t.Fatalf("paths: %d, %v", len(paths), err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil {
		t.Fatal(err)
	}
	for _, color := range []string{"#ff0000", "#00ff00", "#0000ff"} {
		if !strings.Contains(svg, color) {
			t.Fatalf("missing color %s", color)
		}
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %+v, %v", issues, err)
	}
}

func TestImportsPDFDeviceCMYKPathsAndImage(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "cmyk.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 2 {
		t.Fatalf("paths: %d, %v", len(paths), err)
	}
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 {
		t.Fatalf("images: %d, %v", len(images), err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
}

func TestImportsNestedPDFFormsWithScopedResources(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "forms.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 2 {
		t.Fatalf("paths: %d, %v", len(paths), err)
	}
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 {
		t.Fatalf("images: %d, %v", len(images), err)
	}
	text, err := doc.PageText(0)
	if err != nil || len(text) != 1 || text[0] != "Hi" {
		t.Fatalf("text: %v, %v", text, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
}

func TestImportsNamedPDFLinkDestinations(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "named-links.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 2 {
		t.Fatalf("annotations: %+v, %v", annotations, err)
	}
	if annotations[0].Actions[0].Destination.Kind != "XYZ" || annotations[0].Actions[0].Destination.Zoom == nil || *annotations[0].Actions[0].Destination.Zoom != 1.25 {
		t.Fatalf("name tree target: %+v", annotations[0].Actions)
	}
	if annotations[1].Actions[0].Destination.Kind != "Fit" {
		t.Fatalf("legacy target: %+v", annotations[1].Actions)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	if annotations[0].Actions[0].Destination.PageID != pages[1].ID || annotations[1].Actions[0].Destination.PageID != pages[1].ID {
		t.Fatalf("wrong page targets: %+v", annotations)
	}
	pdf, err := doc.ToPDF()
	if err != nil || bytes.Count(pdf, []byte("/Subtype /Link")) != 2 {
		t.Fatalf("PDF links: %v", err)
	}
}

func TestImportsPDFLinkQuadrilaterals(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "quad-links.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 2 {
		t.Fatalf("annotations: %+v, %v", annotations, err)
	}
	region := annotations[0].Actions[0].Region
	if region == nil || len(region.Areas) != 2 || region.Areas[0].Commands[3].Kind != "Close" {
		t.Fatalf("quadrilateral areas: %+v", region)
	}
	pdf, err := doc.ToPDF()
	if err != nil || bytes.Count(pdf, []byte("/Subtype /Link")) != 2 {
		t.Fatalf("PDF links: %v", err)
	}
}

func TestImportsPDFRotatedPage(t *testing.T) {
	for _, tc := range []struct {
		angle string
		box   []float64
	}{
		{"90", []float64{0, 0, 76.2, 50.8}},
		{"180", []float64{0, 0, 50.8, 76.2}},
		{"270", []float64{0, 0, 76.2, 50.8}},
	} {
		t.Run(tc.angle, func(t *testing.T) {
			data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "rotate-"+tc.angle+".pdf"))
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			pages, err := doc.Pages()
			if err != nil || len(pages) != 1 || len(pages[0].PhysicalBox) != 4 ||
				math.Abs(pages[0].PhysicalBox[2]-tc.box[2]) > 1e-8 ||
				math.Abs(pages[0].PhysicalBox[3]-tc.box[3]) > 1e-8 {
				t.Fatalf("rotated page: %v, %v", pages, err)
			}
			text, err := doc.PageText(0)
			if err != nil || !slices.Equal(text, []string{"Hi"}) {
				t.Fatalf("rotated text: %v, %v", text, err)
			}
			paths, err := doc.PagePaths(0)
			if err != nil || len(paths) != 2 {
				t.Fatalf("rotated paths: %v, %v", paths, err)
			}
			images, err := doc.PageImages(0)
			if err != nil || len(images) != 1 {
				t.Fatalf("rotated image: %v, %v", images, err)
			}
			path := filepath.Join(t.TempDir(), "rotated.ofd")
			if err := doc.Save(path); err != nil {
				t.Fatal(err)
			}
			reopened, err := Open(path)
			if err != nil {
				t.Fatal(err)
			}
			defer reopened.Close()
			text, err = reopened.PageText(0)
			if err != nil || !slices.Equal(text, []string{"Hi"}) {
				t.Fatalf("reopened text: %v, %v", text, err)
			}
		})
	}
}

func TestImportsPDFRGBPageGroupWithRotatedText(t *testing.T) {
	source, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "page-group-rotated-text.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(source)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	text, err := doc.PageText(0)
	if err != nil || !slices.Equal(text, []string{"AB"}) {
		t.Fatalf("page text: %v, %v", text, err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 1 {
		t.Fatalf("page paths: %v, %v", paths, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "#ff0000") {
		t.Fatalf("page SVG: %v", err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
}

func TestImportsPDFAffineImage(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "affine-image.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 || images[0].CTM == nil {
		t.Fatalf("affine image: %v, %v", images, err)
	}
	path := filepath.Join(t.TempDir(), "affine-image.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	images, err = reopened.PageImages(0)
	if err != nil || len(images) != 1 {
		t.Fatalf("reopened affine image: %v, %v", images, err)
	}
}

func TestImportsPDFPackedGrayImage(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "packed-gray.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 {
		t.Fatalf("packed gray image: %v, %v", images, err)
	}
	path := filepath.Join(t.TempDir(), "packed-gray.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	images, err = reopened.PageImages(0)
	if err != nil || len(images) != 1 {
		t.Fatalf("reopened packed gray image: %v, %v", images, err)
	}
}

func TestImportsNegativePDFHorizontalScale(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "negative-horizontal-scale.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	text, err := doc.PageText(0)
	if err != nil || !slices.Equal(text, []string{"AB", "C", "D"}) {
		t.Fatalf("mirrored text: %v, %v", text, err)
	}
	runs, err := doc.PageTextObjects(0)
	if err != nil || len(runs) != 3 || runs[0].HScale == nil || *runs[0].HScale != 0.5 {
		t.Fatalf("mirrored text runs: %v, %v", runs, err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 3 || len(objects[0].CTM) != 6 || math.Abs(objects[0].CTM[0]+1) > 1e-9 {
		t.Fatalf("mirrored object transform: %v, %v", objects, err)
	}
	if math.Abs(objects[2].Boundary[0]-11*25.4/72) > 1e-6 {
		t.Fatalf("following text position: %v", objects[2].Boundary)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("reference issues: %v, %v", issues, err)
	}
}

func TestImportsPDFVectorFixture(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "vector.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	count, err := doc.PageCount()
	if err != nil || count != 1 {
		t.Fatalf("page count: %d, %v", count, err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 2 {
		t.Fatalf("paths: %v, %v", paths, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "#ff0000") {
		t.Fatalf("svg: %v, %v", svg, err)
	}
	path := filepath.Join(t.TempDir(), "imported.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	paths, err = reopened.PagePaths(0)
	if err != nil || len(paths) != 2 {
		t.Fatalf("reopened paths: %v, %v", paths, err)
	}
	if _, err := FromPDF([]byte("invalid")); err == nil {
		t.Fatal("expected invalid PDF error")
	}
	if _, err := FromPDF(nil); err == nil {
		t.Fatal("expected empty PDF error")
	}
	imagePDF, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "image.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	imageDoc, err := FromPDF(imagePDF)
	if err != nil {
		t.Fatal(err)
	}
	defer imageDoc.Close()
	images, err := imageDoc.PageImages(0)
	if err != nil || len(images) != 1 {
		t.Fatalf("images: %v, %v", images, err)
	}
	textPDF, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "text.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	textDoc, err := FromPDF(textPDF)
	if err != nil {
		t.Fatal(err)
	}
	defer textDoc.Close()
	text, err := textDoc.PageText(0)
	if err != nil || len(text) != 1 || text[0] != "Hello PDF" {
		t.Fatalf("text: %v, %v", text, err)
	}
	spacedPDF, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "spacing.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	spacedDoc, err := FromPDF(spacedPDF)
	if err != nil {
		t.Fatal(err)
	}
	defer spacedDoc.Close()
	spacedText, err := spacedDoc.PageText(0)
	if err != nil || len(spacedText) != 2 || spacedText[0] != "AB" || spacedText[1] != "CD" {
		t.Fatalf("spaced text: %v, %v", spacedText, err)
	}
	scaledPDF, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "horizontal-scale.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	scaledDoc, err := FromPDF(scaledPDF)
	if err != nil {
		t.Fatal(err)
	}
	defer scaledDoc.Close()
	scaledText, err := scaledDoc.PageText(0)
	if err != nil || !slices.Equal(scaledText, []string{"AB", "C", "D"}) {
		t.Fatalf("scaled text: %v, %v", scaledText, err)
	}
	scaledObjects, err := scaledDoc.PageTextObjects(0)
	if err != nil || len(scaledObjects) != 3 || math.Abs(scaledObjects[1].Boundary[0]-16*25.4/72) > 1e-6 {
		t.Fatalf("scaled objects: %v, %v", scaledObjects, err)
	}
	if scaledObjects[0].HScale == nil || *scaledObjects[0].HScale != 0.5 ||
		scaledObjects[2].HScale == nil || *scaledObjects[2].HScale != 1.5 {
		t.Fatalf("horizontal scales: %v", scaledObjects)
	}
	appearancePDF, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "text-appearance.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	appearanceDoc, err := FromPDF(appearancePDF)
	if err != nil {
		t.Fatal(err)
	}
	defer appearanceDoc.Close()
	appearanceText, err := appearanceDoc.PageText(0)
	if err != nil || !slices.Equal(appearanceText, []string{"A", "B", "C", "D", "E"}) {
		t.Fatalf("text appearance: %v, %v", appearanceText, err)
	}
	appearanceObjects, err := appearanceDoc.PageTextObjects(0)
	if err != nil || len(appearanceObjects) != 5 ||
		math.Abs(appearanceObjects[4].Boundary[1]-appearanceObjects[0].Boundary[1]+2*25.4/72) > 1e-6 {
		t.Fatalf("text rise: %v, %v", appearanceObjects, err)
	}
}

func TestImportsPDFClippingFixture(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "clip.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 2 {
		t.Fatalf("paths: %v, %v", paths, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "<clipPath ") {
		t.Fatalf("clip path missing: %v, %v", svg, err)
	}
	path := filepath.Join(t.TempDir(), "clipped.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	svg, err = reopened.PageSVG(0)
	if err != nil || !strings.Contains(svg, "<clipPath ") {
		t.Fatalf("reopened clip path missing: %v, %v", svg, err)
	}
	issues, err := reopened.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
}

func TestImportsPDFEvenOddFillFixture(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "evenodd.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `fill-rule="evenodd"`) {
		t.Fatalf("even-odd SVG: %v, %v", svg, err)
	}
	path := filepath.Join(t.TempDir(), "even_odd_import.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	svg, err = reopened.PageSVG(0)
	if err != nil || !strings.Contains(svg, `fill-rule="evenodd"`) {
		t.Fatalf("reopened even-odd SVG: %v, %v", svg, err)
	}
}

func TestImportsPDFStrokeStyleFixture(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "stroke.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `stroke-linecap="round"`) || !strings.Contains(svg, `stroke-linejoin="bevel"`) || !strings.Contains(svg, `stroke-dasharray="`) {
		t.Fatalf("stroke SVG: %v, %v", svg, err)
	}
	path := filepath.Join(t.TempDir(), "stroke_import.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	reopenedSVG, err := reopened.PageSVG(0)
	if err != nil || reopenedSVG != svg {
		t.Fatalf("reopened stroke SVG: %v", err)
	}
}

func TestImportsPDFAlphaFixture(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "alpha.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `fill-opacity="0.5019607843137255"`) || !strings.Contains(svg, `stroke-opacity="0.25098039215686274"`) {
		t.Fatalf("alpha SVG: %v, %v", svg, err)
	}
	path := filepath.Join(t.TempDir(), "alpha_import.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	reopenedSVG, err := reopened.PageSVG(0)
	if err != nil || reopenedSVG != svg {
		t.Fatalf("reopened alpha SVG: %v", err)
	}
}

func TestImportsPDFBlendModeFixture(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "blend.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil || strings.Count(svg, "mix-blend-mode:darken") != 1 {
		t.Fatalf("blend SVG: %v, %v", svg, err)
	}
	pngBytes, err := doc.PagePNG(0, 25.4)
	if err != nil {
		t.Fatal(err)
	}
	image, err := png.Decode(bytes.NewReader(pngBytes))
	if err != nil {
		t.Fatal(err)
	}
	r, g, b, _ := image.At(12, 12).RGBA()
	if r > 0x1000 || g > 0x1000 || b > 0x1000 {
		t.Fatalf("darkened center pixel: %x %x %x", r, g, b)
	}
	path := filepath.Join(t.TempDir(), "blend_import.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	reopenedSVG, err := reopened.PageSVG(0)
	if err != nil || reopenedSVG != svg {
		t.Fatalf("reopened blend SVG: %v", err)
	}
}

func TestImportsPDFTextClipFixture(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "text-clip.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	text, err := doc.PageText(0)
	if err != nil || !slices.Equal(text, []string{"A", "B"}) {
		t.Fatalf("clipped text: %q, %v", text, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || strings.Count(svg, `<clipPath id="ofd-clip-`) != 1 {
		t.Fatalf("text clip SVG: %v, %v", svg, err)
	}
	pngBytes, err := doc.PagePNG(0, 25.4)
	if err != nil {
		t.Fatal(err)
	}
	image, err := png.Decode(bytes.NewReader(pngBytes))
	if err != nil {
		t.Fatal(err)
	}
	redPixels := 0
	for x := 14; x < 22; x++ {
		for y := 5; y < 16; y++ {
			r, g, _, _ := image.At(x, y).RGBA()
			if r > 0xe000 && g < 0xe000 {
				redPixels++
			}
		}
	}
	if redPixels < 4 {
		t.Fatalf("clipped glyph red pixels: %d", redPixels)
	}
	path := filepath.Join(t.TempDir(), "text_clip_import.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	reopenedSVG, err := reopened.PageSVG(0)
	if err != nil || reopenedSVG != svg {
		t.Fatalf("reopened text clip SVG: %v", err)
	}
}

func TestImportsPDFSoftMaskFixture(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "soft-mask.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	imageBytes, err := doc.PageImageData(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	image, err := png.Decode(bytes.NewReader(imageBytes))
	if err != nil {
		t.Fatal(err)
	}
	for _, sample := range []struct {
		x, y  int
		alpha uint32
	}{
		{0, 0, 0},
		{1, 0, 0x8080},
		{0, 1, 0xffff},
	} {
		_, _, _, alpha := image.At(sample.x, sample.y).RGBA()
		if alpha != sample.alpha {
			t.Fatalf("soft mask alpha at (%d,%d): %x", sample.x, sample.y, alpha)
		}
	}
	path := filepath.Join(t.TempDir(), "soft_mask_import.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	reopenedImage, err := reopened.PageImageData(0, 0)
	if err != nil || !bytes.Equal(reopenedImage, imageBytes) {
		t.Fatalf("reopened soft mask image: %v", err)
	}
}

func TestImportsPDFCMYKICCJPEGFixture(t *testing.T) {
	for _, name := range []string{"cmyk-icc-jpeg.pdf", "ycck-icc-jpeg.pdf"} {
		t.Run(name, func(t *testing.T) {
			data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", name))
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			imageBytes, err := doc.PageImageData(0, 0)
			if err != nil {
				t.Fatal(err)
			}
			image, err := png.Decode(bytes.NewReader(imageBytes))
			if err != nil {
				t.Fatal(err)
			}
			if image.Bounds().Dx() != 2 || image.Bounds().Dy() != 1 {
				t.Fatalf("unexpected ICC JPEG dimensions: %v", image.Bounds())
			}
			path := filepath.Join(t.TempDir(), "icc_import.ofd")
			if err := doc.Save(path); err != nil {
				t.Fatal(err)
			}
			reopened, err := Open(path)
			if err != nil {
				t.Fatal(err)
			}
			defer reopened.Close()
			reopenedImage, err := reopened.PageImageData(0, 0)
			if err != nil || !bytes.Equal(reopenedImage, imageBytes) {
				t.Fatalf("reopened ICC JPEG image: %v", err)
			}
		})
	}
}

func TestImportsPDFCMYKJPEGColorKeyMask(t *testing.T) {
	for _, name := range []string{
		"cmyk-mask.pdf", "ycck-mask.pdf",
		"cmyk-decode-mask.pdf", "ycck-decode-mask.pdf",
	} {
		t.Run(name, func(t *testing.T) {
			data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", name))
			if err != nil {
				t.Fatal(err)
			}
			doc, err := FromPDF(data)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			imageBytes, err := doc.PageImageData(0, 0)
			if err != nil {
				t.Fatal(err)
			}
			image, err := png.Decode(bytes.NewReader(imageBytes))
			if err != nil {
				t.Fatal(err)
			}
			if image.Bounds().Dx() != 2 || image.Bounds().Dy() != 1 {
				t.Fatalf("unexpected masked JPEG dimensions: %v", image.Bounds())
			}
			_, _, _, firstAlpha := image.At(0, 0).RGBA()
			_, _, _, secondAlpha := image.At(1, 0).RGBA()
			if firstAlpha != 0 || secondAlpha != 0xffff {
				t.Fatalf("masked JPEG alpha: %x, %x", firstAlpha, secondAlpha)
			}
			path := filepath.Join(t.TempDir(), "masked_jpeg.ofd")
			if err := doc.Save(path); err != nil {
				t.Fatal(err)
			}
			reopened, err := Open(path)
			if err != nil {
				t.Fatal(err)
			}
			defer reopened.Close()
			reopenedImage, err := reopened.PageImageData(0, 0)
			if err != nil || !bytes.Equal(reopenedImage, imageBytes) {
				t.Fatalf("reopened masked JPEG image: %v", err)
			}
		})
	}
}

func TestImportsPDFIndexedImageFixture(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "indexed-image.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	imageBytes, err := doc.PageImageData(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	image, err := png.Decode(bytes.NewReader(imageBytes))
	if err != nil {
		t.Fatal(err)
	}
	for _, sample := range []struct {
		x, y    int
		r, g, b uint32
	}{
		{0, 0, 0xffff, 0, 0},
		{1, 0, 0, 0xffff, 0},
		{0, 1, 0, 0, 0xffff},
	} {
		r, g, b, _ := image.At(sample.x, sample.y).RGBA()
		if r != sample.r || g != sample.g || b != sample.b {
			t.Fatalf("indexed image color at (%d,%d): %x %x %x", sample.x, sample.y, r, g, b)
		}
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %+v, %v", issues, err)
	}
}

func TestImportsPDFChainedImageFilters(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "asciihex-runlength-image.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	imageBytes, err := doc.PageImageData(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	image, err := png.Decode(bytes.NewReader(imageBytes))
	if err != nil {
		t.Fatal(err)
	}
	if r, _, _, _ := image.At(0, 0).RGBA(); r != 0 {
		t.Fatalf("top image pixel: %x", r)
	}
	if r, _, _, _ := image.At(0, 1).RGBA(); r != 0xffff {
		t.Fatalf("bottom image pixel: %x", r)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %+v, %v", issues, err)
	}
}

func TestImportsPDFLZWPredictorImage(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "lzw-predictor-image.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	imageBytes, err := doc.PageImageData(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	image, err := png.Decode(bytes.NewReader(imageBytes))
	if err != nil {
		t.Fatal(err)
	}
	for _, sample := range []struct {
		x, y int
		gray uint32
	}{
		{0, 0, 10 * 257},
		{1, 0, 20 * 257},
		{0, 1, 30 * 257},
		{1, 1, 50 * 257},
	} {
		r, g, b, _ := image.At(sample.x, sample.y).RGBA()
		if r != sample.gray || g != sample.gray || b != sample.gray {
			t.Fatalf("LZW image color at (%d,%d): %x %x %x", sample.x, sample.y, r, g, b)
		}
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %+v, %v", issues, err)
	}
}

func TestImportsPDFColorKeyMaskFixture(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "color-key-mask.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	imageBytes, err := doc.PageImageData(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	image, err := png.Decode(bytes.NewReader(imageBytes))
	if err != nil {
		t.Fatal(err)
	}
	_, _, _, transparent := image.At(0, 0).RGBA()
	_, _, _, opaque := image.At(1, 0).RGBA()
	if transparent != 0 || opaque != 0xffff {
		t.Fatalf("color key mask alpha: transparent=%x opaque=%x", transparent, opaque)
	}
	path := filepath.Join(t.TempDir(), "color_key_import.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	reopenedImage, err := reopened.PageImageData(0, 0)
	if err != nil || !bytes.Equal(reopenedImage, imageBytes) {
		t.Fatalf("reopened image differs: %v", err)
	}
}

func TestImportsPDFExplicitMaskFixture(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "explicit-mask.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	imageBytes, err := doc.PageImageData(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	image, err := png.Decode(bytes.NewReader(imageBytes))
	if err != nil {
		t.Fatal(err)
	}
	for _, sample := range []struct {
		x, y  int
		alpha uint32
	}{
		{0, 0, 0xffff},
		{1, 0, 0},
		{0, 1, 0},
		{1, 1, 0xffff},
	} {
		_, _, _, alpha := image.At(sample.x, sample.y).RGBA()
		if alpha != sample.alpha {
			t.Fatalf("explicit mask alpha at (%d,%d): %x", sample.x, sample.y, alpha)
		}
	}
	path := filepath.Join(t.TempDir(), "explicit_mask_import.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	reopenedImage, err := reopened.PageImageData(0, 0)
	if err != nil || !bytes.Equal(reopenedImage, imageBytes) {
		t.Fatalf("reopened image differs: %v", err)
	}
}

func TestImportsPDFStencilMaskFixture(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "stencil-mask.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(data)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	imageBytes, err := doc.PageImageData(0, 0)
	if err != nil {
		t.Fatal(err)
	}
	image, err := png.Decode(bytes.NewReader(imageBytes))
	if err != nil {
		t.Fatal(err)
	}
	for _, sample := range []struct {
		x, y  int
		alpha uint32
	}{
		{0, 0, 0xffff},
		{1, 0, 0},
		{0, 1, 0},
		{1, 1, 0xffff},
	} {
		red, green, blue, alpha := image.At(sample.x, sample.y).RGBA()
		if alpha != sample.alpha || (alpha != 0 && (red != 0 || green != 0xffff || blue != 0)) {
			t.Fatalf("stencil mask color at (%d,%d): %x %x %x %x", sample.x, sample.y, red, green, blue, alpha)
		}
	}
	path := filepath.Join(t.TempDir(), "stencil_mask_import.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	reopenedImage, err := reopened.PageImageData(0, 0)
	if err != nil || !bytes.Equal(reopenedImage, imageBytes) {
		t.Fatalf("reopened image differs: %v", err)
	}
}

func TestPageTextInRegion(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("区域", 80, 50); err != nil {
		t.Fatal(err)
	}
	text, err := doc.PageTextInRegion(0, 9, 9, 20, 10)
	if err != nil || !slices.Equal(text, []string{"区域"}) {
		t.Fatalf("text in region: %q, %v", text, err)
	}
	text, err = doc.PageTextInRegion(0, 0, 0, 5, 5)
	if err != nil || len(text) != 0 {
		t.Fatalf("text outside region: %q, %v", text, err)
	}
	if _, err := doc.PageTextInRegion(-1, 0, 0, 20, 20); err == nil {
		t.Fatal("expected negative page index error")
	}
	if _, err := doc.PageTextInRegion(0, 0, 0, 0, 20); err == nil {
		t.Fatal("expected invalid region error")
	}
	if _, err := doc.PageTextInRegion(1, 0, 0, 20, 20); err == nil {
		t.Fatal("expected out-of-range page error")
	}
	if _, err := doc.PageTextInRegion(0, math.NaN(), 0, 20, 20); err == nil {
		t.Fatal("expected nonfinite region error")
	}
	path := filepath.Join(t.TempDir(), "region.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	text, err = reopened.PageTextInRegion(0, 9, 9, 20, 10)
	if err != nil || !slices.Equal(text, []string{"区域"}) {
		t.Fatalf("reopened text in region: %q, %v", text, err)
	}
	reopened.Close()
	if _, err := reopened.PageTextInRegion(0, 9, 9, 20, 10); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestHiddenTextIsAbsentFromExtractionAndPreview(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("shown", 80, 50); err != nil {
		t.Fatal(err)
	}
	objectID, err := doc.AddTextObject(0, "hidden", [4]float64{10, 20, 40, 10}, 5, "")
	if err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	path := pages[0].Path
	xml, err := doc.Entry(path)
	if err != nil {
		t.Fatal(err)
	}
	marker := []byte(fmt.Sprintf("<ofd:TextObject ID=\"%d\"", objectID))
	if !bytes.Contains(xml, marker) {
		t.Fatal("inserted text object was not found")
	}
	xml = bytes.Replace(xml, marker, append(marker, []byte(` Visible="false"`)...), 1)
	if err := doc.SetEntry(path, xml); err != nil {
		t.Fatal(err)
	}
	text, err := doc.PageText(0)
	if err != nil || !slices.Equal(text, []string{"shown"}) {
		t.Fatalf("page text: %q, %v", text, err)
	}
	text, err = doc.PageTextInRegion(0, 0, 0, 80, 50)
	if err != nil || !slices.Equal(text, []string{"shown"}) {
		t.Fatalf("region text: %q, %v", text, err)
	}
	matches, err := doc.KeywordPositions([]string{"hidden"}, nil)
	if err != nil || len(matches) != 0 {
		t.Fatalf("hidden keyword: %v, %v", matches, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || strings.Contains(svg, "hidden") {
		t.Fatalf("hidden SVG text: %v", err)
	}
}

func TestKeywordPositions(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "keyword.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	matches, err := doc.KeywordPositions([]string{"办理", "不动产权"}, nil)
	if err != nil || len(matches) != 3 {
		t.Fatalf("keyword positions: %v, %v", matches, err)
	}
	for _, match := range matches {
		if match.PageIndex != 0 || match.BoxMM[2] <= 0 || match.BoxMM[3] <= 0 {
			t.Fatalf("invalid keyword position: %+v", match)
		}
	}
	selected, err := doc.KeywordPositions([]string{"办理"}, []int{0})
	if err != nil || len(selected) != 1 || selected[0].Keyword != "办理" {
		t.Fatalf("selected keyword: %v, %v", selected, err)
	}
	empty, err := doc.KeywordPositions([]string{"办理"}, []int{})
	if err != nil || len(empty) != 0 {
		t.Fatalf("empty page selection: %v, %v", empty, err)
	}
	if _, err := doc.KeywordPositions([]string{""}, nil); err == nil {
		t.Fatal("expected empty keyword error")
	}
	if _, err := doc.KeywordPositions([]string{"办理"}, []int{-1}); err == nil {
		t.Fatal("expected negative page index error")
	}
	if _, err := doc.KeywordPositions([]string{"办理"}, []int{1}); err == nil {
		t.Fatal("expected out-of-range page error")
	}
	doc.Close()
	if _, err := doc.KeywordPositions([]string{"办理"}, nil); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestKeywordPositionsStayWithinOneTextObject(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(80, 50); err != nil {
		t.Fatal(err)
	}
	for i, value := range []string{"A", "B"} {
		if _, err := doc.AddTextObject(0, value, [4]float64{10 + float64(i)*10, 20, 10, 8}, 5, ""); err != nil {
			t.Fatal(err)
		}
	}
	matches, err := doc.KeywordPositions([]string{"AB"}, nil)
	if err != nil || len(matches) != 0 {
		t.Fatalf("joined separate text objects: %v, %v", matches, err)
	}
	matches, err = doc.KeywordPositions([]string{"A", "B"}, nil)
	if err != nil || len(matches) != 2 {
		t.Fatalf("individual text objects: %v, %v", matches, err)
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
	matches, err = reopened.KeywordPositions([]string{"AB"}, nil)
	if err != nil || len(matches) != 0 {
		t.Fatalf("joined separate text objects after reopening: %v, %v", matches, err)
	}
}

func TestKeywordBoxUsesCharacterDirectionAndHorizontalScale(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("A", 30, 20); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("text object: %+v, %v", objects, err)
	}
	scale := 0.5
	direction := uint16(90)
	if err := doc.SetPageTextTypography(0, *objects[0].ID, TextTypographyStyle{
		Size: 5, HScale: &scale, CharDirection: &direction,
	}); err != nil {
		t.Fatal(err)
	}
	matches, err := doc.KeywordPositions([]string{"A"}, nil)
	if err != nil || len(matches) != 1 || matches[0].BoxMM != [4]float64{10, 15, 5, 2.5} {
		t.Fatalf("rotated keyword box: %+v, %v", matches, err)
	}
}

func TestCompositeImageObject(t *testing.T) {
	sourcePath := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "发票示例.ofd")
	source, err := Open(sourcePath)
	if err != nil {
		t.Fatal(err)
	}
	png, err := source.PageImageData(0, 0)
	source.Close()
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("base", 40, 40); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddCompositeImageObject(0, png, "png", [4]float64{2, 3, 20, 20})
	if err != nil || id == 0 {
		t.Fatalf("composite image: %d, %v", id, err)
	}
	objects, err := doc.PageCompositeObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil || *objects[0].ID != fmt.Sprint(id) {
		t.Fatalf("composite objects: %v, %v", objects, err)
	}
	units, err := doc.PageCompositeUnits(0)
	if err != nil || len(units) != 1 || units[0].ID != objects[0].ResourceID || units[0].Width <= 0 || units[0].Height <= 0 {
		t.Fatalf("composite units: %v, %v", units, err)
	}
	if units[0].Thumbnail != nil || units[0].Substitution != nil {
		t.Fatalf("unexpected composite image references: %v", units[0])
	}
	if _, err := doc.PageCompositeUnits(-1); err == nil {
		t.Fatal("expected negative page index error")
	}
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 0 {
		t.Fatalf("direct images: %v, %v", images, err)
	}
	images, err = doc.PageCompositeImages(0)
	if err != nil || len(images) != 1 || images[0].Format == nil || *images[0].Format != "PNG" {
		t.Fatalf("composite images: %v, %v", images, err)
	}
	mediaID := images[0].ResourceID
	if err := doc.SetPageCompositeUnitImages(0, objects[0].ResourceID, &mediaID, &mediaID); err != nil {
		t.Fatal(err)
	}
	units, err = doc.PageCompositeUnits(0)
	if err != nil || units[0].Thumbnail == nil || *units[0].Thumbnail != mediaID || units[0].Substitution == nil || *units[0].Substitution != mediaID {
		t.Fatalf("composite image references: %v, %v", units, err)
	}
	if err := doc.SetPageCompositeUnitImages(0, objects[0].ResourceID, nil, nil); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageCompositeUnitImages(-1, objects[0].ResourceID, nil, nil); err == nil {
		t.Fatal("expected negative page index error")
	}
	data, err := doc.PageCompositeImageData(0, 0)
	if err != nil || !bytes.Equal(data, png) {
		t.Fatalf("composite image bytes: %v, %v", len(data), err)
	}
	if _, err := doc.PageCompositeImageData(0, 1); err == nil {
		t.Fatal("expected composite image index error")
	}
	if _, err := doc.PageCompositeImageData(-1, 0); err == nil {
		t.Fatal("expected negative page index")
	}
	if _, err := doc.PageCompositeImageData(0, -1); err == nil {
		t.Fatal("expected negative composite image index")
	}
	if _, err := doc.PageCompositeImages(-1); err == nil {
		t.Fatal("expected negative page index")
	}
	if _, err := doc.PageCompositeImages(10); err == nil {
		t.Fatal("expected out-of-range page index")
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "data:image/png;base64,") {
		t.Fatalf("svg: %v, %v", svg, err)
	}
	if _, err := doc.AddCompositeImageObject(-1, png, "png", [4]float64{2, 3, 20, 20}); err == nil {
		t.Fatal("expected negative page index")
	}
	if _, err := doc.AddCompositeImageObject(0, []byte("bad"), "png", [4]float64{2, 3, 20, 20}); err == nil {
		t.Fatal("expected invalid image")
	}
	objects, err = doc.PageCompositeObjects(0)
	if err != nil || len(objects) != 1 {
		t.Fatalf("document mutated after error: %v, %v", objects, err)
	}
	path := filepath.Join(t.TempDir(), "composite-image.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	data, err = reopened.PageCompositeImageData(0, 0)
	if err != nil || !bytes.Equal(data, png) {
		t.Fatalf("reopened composite image bytes: %v, %v", len(data), err)
	}
	if issues, err := reopened.ValidateReferences(); err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
	entries, err := reopened.Entries()
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		if !strings.Contains(entry, "CompositeRes_") {
			continue
		}
		data, err := reopened.Entry(entry)
		if err != nil {
			t.Fatal(err)
		}
		broken := bytes.Replace(data, []byte(`ResourceID="`), []byte(`ResourceID="999999`), 1)
		if err := reopened.SetEntry(entry, broken); err != nil {
			t.Fatal(err)
		}
		issues, err := reopened.ValidateReferences()
		if err != nil || !slices.ContainsFunc(issues, func(issue ValidationIssue) bool { return issue.Code == "missing_media" }) {
			t.Fatalf("nested media reference: %v, %v", issues, err)
		}
		cyclic := bytes.Replace(data, []byte("<ofd:ImageObject "), []byte("<ofd:CompositeObject "), 1)
		start := bytes.Index(cyclic, []byte(`ResourceID="`)) + len(`ResourceID="`)
		end := start + bytes.IndexByte(cyclic[start:], '"')
		cyclic = slices.Concat(cyclic[:start], []byte(objects[0].ResourceID), cyclic[end:])
		if err := reopened.SetEntry(entry, cyclic); err != nil {
			t.Fatal(err)
		}
		issues, err = reopened.ValidateReferences()
		if err != nil || !slices.ContainsFunc(issues, func(issue ValidationIssue) bool { return issue.Code == "composite_cycle" }) {
			t.Fatalf("composite cycle: %v, %v", issues, err)
		}
		reopened.Close()
		if _, err := reopened.PageCompositeImages(0); err == nil {
			t.Fatal("expected closed document error")
		}
		if _, err := reopened.PageCompositeImageData(0, 0); err == nil {
			t.Fatal("expected closed document error")
		}
		return
	}
	t.Fatal("composite resource was not saved")
}

func TestRendersConverterFixturesWithOptionalDrawingFields(t *testing.T) {
	root := filepath.Join("..", "..", "..", "..", "ofdrw-converter", "src", "test", "resources")
	for _, name := range []string{"发票示例.ofd", "文字横向-数科.ofd"} {
		doc, err := Open(filepath.Join(root, name))
		if err != nil {
			t.Fatal(err)
		}
		svg, err := doc.PageSVG(0)
		if err != nil || !strings.Contains(svg, "<svg ") {
			t.Fatalf("render %s: %v", name, err)
		}
		doc.Close()
	}
	intro, err := Open(filepath.Join(root, "intro-数科.ofd"))
	if err != nil {
		t.Fatal(err)
	}
	defer intro.Close()
	paths, err := intro.PagePaths(7)
	if err != nil || !slices.ContainsFunc(paths, func(path PathObject) bool { return len(path.Boundary) == 0 }) {
		t.Fatalf("paths without boundary: %v, %v", paths, err)
	}
	if svg, err := intro.PageSVG(7); err != nil || !strings.Contains(svg, "<path ") {
		t.Fatalf("render path without boundary: %v", err)
	}
}

func TestReadsJavaFixture(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "helloworld.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	count, err := doc.PageCount()
	if err != nil || count != 1 {
		t.Fatalf("page count: %d, %v", count, err)
	}
	pages, err := doc.Pages()
	if err != nil || len(pages) != 1 || len(pages[0].PhysicalBox) != 4 || pages[0].PhysicalBox[2] != 210 {
		t.Fatalf("pages: %v, %v", pages, err)
	}
	entries, err := doc.Entries()
	if err != nil || len(entries) == 0 || entries[len(entries)-1] != "OFD.xml" {
		t.Fatalf("entries: %v, %v", entries, err)
	}
	text, err := doc.PageText(0)
	if err != nil || len(text) != 1 || text[0] != "你好呀，OFD Reader&Writer！" {
		t.Fatalf("page text: %q, %v", text, err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || len(objects[0].Boundary) != 4 || objects[0].Boundary[0] != 31.7 || len(objects[0].Codes) != 1 || objects[0].Codes[0].Text != text[0] {
		t.Fatalf("page text objects: %v, %v", objects, err)
	}
	if _, err := doc.PageTextObjects(-1); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.PageTextObjects(1); err == nil {
		t.Fatal("expected out-of-range error")
	}
}

func TestReplacesUnshapedTextCodeAtomically(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("AB", 20, 20); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("text object: %+v, %v", objects, err)
	}
	id := *objects[0].ID
	if err := doc.SetPageTextCode(0, id, 0, "A & <B>"); err != nil {
		t.Fatal(err)
	}
	content, err := doc.PageText(0)
	if err != nil || len(content) != 1 || content[0] != "A & <B>" {
		t.Fatalf("text content: %q, %v", content, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "A &amp; &lt;B&gt;") {
		t.Fatalf("text SVG: %v, %v", svg, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	before, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageTextCode(0, id, 1, "X"); err == nil {
		t.Fatal("expected missing TextCode")
	}
	if err := doc.SetPageTextCode(0, id, -1, "X"); err == nil {
		t.Fatal("expected negative TextCode index")
	}
	after, err := doc.Entry(pages[0].Path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid edit changed page: %v", err)
	}
}

func TestSetsTextCodeLayoutAtomically(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("AB", 30, 20); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("text object: %+v, %v", objects, err)
	}
	x, y := 2.0, 7.0
	layout := TextCodeLayout{X: &x, Y: &y, DeltaX: []float64{3.5}}
	if err := doc.SetPageTextCodeLayout(0, *objects[0].ID, 0, layout); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.PageTextObjects(0)
	if err != nil || objects[0].Codes[0].DeltaX == nil || *objects[0].Codes[0].DeltaX != "3.5" {
		t.Fatalf("updated TextCode layout: %+v, %v", objects, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `x="2 5.5" y="7 7"`) {
		t.Fatalf("text positions: %v, %v", svg, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	before, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageTextCodeLayout(0, *objects[0].ID, 0, TextCodeLayout{X: &x, Y: &y, DeltaX: []float64{1, 2}}); err == nil {
		t.Fatal("expected delta count error")
	}
	if err := doc.SetPageTextCodeLayout(0, *objects[0].ID, -1, layout); err == nil {
		t.Fatal("expected negative TextCode index error")
	}
	after, err := doc.Entry(pages[0].Path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid edit changed page: %v", err)
	}
}

func TestAppendsPositionedTextCodeAtomically(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("AB", 30, 20); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("text object: %+v, %v", objects, err)
	}
	id := *objects[0].ID
	if err := doc.AppendPageTextCode(0, id, "C&D", TextCodeLayout{DeltaX: []float64{2, 2.5}}); err != nil {
		t.Fatal(err)
	}
	content, err := doc.PageText(0)
	if err != nil || !slices.Equal(content, []string{"AB", "C&D"}) {
		t.Fatalf("text content: %v, %v", content, err)
	}
	objects, err = doc.PageTextObjects(0)
	if err != nil || len(objects[0].Codes) != 2 || objects[0].Codes[1].DeltaX == nil || *objects[0].Codes[1].DeltaX != "2 2.5" {
		t.Fatalf("appended TextCode: %+v, %v", objects, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	before, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.AppendPageTextCode(0, id, "XY", TextCodeLayout{DeltaX: []float64{1, 2}}); err == nil {
		t.Fatal("expected delta count error")
	}
	after, err := doc.Entry(pages[0].Path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid append changed page: %v", err)
	}
}

func TestRemovesTextCodeWithoutMovingInheritedRun(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("A", 30, 20); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("text object: %+v, %v", objects, err)
	}
	id := *objects[0].ID
	x, y := 5.0, 6.0
	if err := doc.AppendPageTextCode(0, id, "B", TextCodeLayout{X: &x, Y: &y}); err != nil {
		t.Fatal(err)
	}
	if err := doc.AppendPageTextCode(0, id, "C", TextCodeLayout{}); err != nil {
		t.Fatal(err)
	}
	if err := doc.RemovePageTextCode(0, id, 1); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.PageTextObjects(0)
	if err != nil || len(objects[0].Codes) != 2 || objects[0].Codes[1].X == nil || *objects[0].Codes[1].X != 5 || objects[0].Codes[1].Y == nil || *objects[0].Codes[1].Y != 6 {
		t.Fatalf("successor position: %+v, %v", objects, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	before, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.RemovePageTextCode(0, id, 9); err == nil {
		t.Fatal("expected missing run error")
	}
	if err := doc.RemovePageTextCode(0, id, -1); err == nil {
		t.Fatal("expected negative index error")
	}
	after, err := doc.Entry(pages[0].Path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid removal changed page: %v", err)
	}
	if err := doc.RemovePageTextCode(0, id, 0); err != nil {
		t.Fatal(err)
	}
	if err := doc.RemovePageTextCode(0, id, 0); err == nil {
		t.Fatal("expected last-run protection")
	}
}

func TestSetsTextFontAndRejectsInvalidReferences(t *testing.T) {
	fontPath := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-serif.ttf")
	font, err := os.ReadFile(fontPath)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("AB", 30, 20); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("text object: %+v, %v", objects, err)
	}
	fontID, err := doc.AddEmbeddedFont("DejaVu Serif", font)
	if err != nil {
		t.Fatal(err)
	}
	id := fmt.Sprint(fontID)
	if err := doc.SetPageTextFont(0, *objects[0].ID, id); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.PageTextObjects(0)
	if err != nil || objects[0].FontID == nil || *objects[0].FontID != id {
		t.Fatalf("updated font: %+v, %v", objects, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	before, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageTextFont(0, *objects[0].ID, "999999"); err == nil {
		t.Fatal("expected unavailable font error")
	}
	if err := doc.SetPageTextFont(0, "missing", id); err == nil {
		t.Fatal("expected missing object error")
	}
	if err := doc.SetPageTextFont(0, *objects[0].ID, "bad\x00id"); err == nil {
		t.Fatal("expected NUL rejection")
	}
	after, err := doc.Entry(pages[0].Path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid edit changed page: %v", err)
	}
}

func TestSetsTextPaintFlagsAndRestoresDefaults(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("AB", 30, 20); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("text object: %+v, %v", objects, err)
	}
	fill, stroke := false, true
	if err := doc.SetPageTextPaintFlags(0, *objects[0].ID, TextPaintFlags{Fill: &fill, Stroke: &stroke}); err != nil {
		t.Fatal(err)
	}
	paints, err := doc.PageObjectPaints(0)
	if err != nil || len(paints) != 1 || paints[0].FillEnabled || !paints[0].StrokeEnabled {
		t.Fatalf("text paint flags: %+v, %v", paints, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	before, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageTextPaintFlags(0, "missing", TextPaintFlags{}); err == nil {
		t.Fatal("expected missing text object error")
	}
	after, err := doc.Entry(pages[0].Path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid edit changed page: %v", err)
	}
	if err := doc.SetPageTextPaintFlags(0, *objects[0].ID, TextPaintFlags{}); err != nil {
		t.Fatal(err)
	}
	paints, err = doc.PageObjectPaints(0)
	if err != nil || len(paints) != 1 || !paints[0].FillEnabled || paints[0].StrokeEnabled {
		t.Fatalf("default text paint flags: %+v, %v", paints, err)
	}
}

func TestSetsTextTypographyAtomically(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("AB", 20, 20); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("text object: %+v, %v", objects, err)
	}
	id := *objects[0].ID
	scale := 0.75
	read := uint16(180)
	char := uint16(90)
	weight := uint16(700)
	italic := true
	if err := doc.SetPageTextTypography(0, id, TextTypographyStyle{
		Size: 4, HScale: &scale, ReadDirection: &read, CharDirection: &char, Weight: &weight, Italic: &italic,
	}); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.PageTextObjects(0)
	if err != nil || objects[0].Size == nil || *objects[0].Size != 4 || objects[0].HScale == nil || *objects[0].HScale != scale || objects[0].ReadDirection != read || objects[0].CharDirection != char || objects[0].Weight != weight || !objects[0].Italic {
		t.Fatalf("updated typography: %+v, %v", objects, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	before, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	bad := uint16(750)
	if err := doc.SetPageTextTypography(0, id, TextTypographyStyle{Size: 4, Weight: &bad}); err == nil {
		t.Fatal("expected invalid weight")
	}
	after, err := doc.Entry(pages[0].Path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid update changed page: %v", err)
	}
	if err := doc.SetPageTextTypography(0, id, TextTypographyStyle{Size: 3}); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.PageTextObjects(0)
	if err != nil || objects[0].Size == nil || *objects[0].Size != 3 || objects[0].HScale != nil || objects[0].ReadDirection != 0 || objects[0].CharDirection != 0 || objects[0].Weight != 400 || objects[0].Italic {
		t.Fatalf("default typography: %+v, %v", objects, err)
	}
}

func TestReadsTextTypographyAttributesAndDefaults(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("AB", 20, 20); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ReadDirection != 0 || objects[0].CharDirection != 0 || objects[0].Weight != 400 || objects[0].Italic {
		t.Fatalf("default typography: %+v, %v", objects, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	source, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	styled := bytes.Replace(source, []byte("<ofd:TextObject ID="), []byte("<ofd:TextObject ReadDirection=\"180\" CharDirection=\"90\" Weight=\"700\" Italic=\"true\" ID="), 1)
	if err := doc.SetEntry(pages[0].Path, styled); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ReadDirection != 180 || objects[0].CharDirection != 90 || objects[0].Weight != 700 || !objects[0].Italic {
		t.Fatalf("styled typography: %+v, %v", objects, err)
	}
	invalid := bytes.Replace(styled, []byte("Weight=\"700\""), []byte("Weight=\"750\""), 1)
	if err := doc.SetEntry(pages[0].Path, invalid); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.PageTextObjects(0); err == nil {
		t.Fatal("expected invalid text weight")
	}
}

func TestMovesPagesAndKeepsOrderAfterAppend(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	for _, value := range []string{"A", "B", "C"} {
		if err := doc.AddTextPage(value, 20, 20); err != nil {
			t.Fatal(err)
		}
	}
	if err := doc.MovePage(0, 2); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddTextPage("D", 20, 20); err != nil {
		t.Fatal(err)
	}
	for index, expected := range []string{"B", "C", "A", "D"} {
		text, err := doc.PageText(index)
		if err != nil || len(text) != 1 || text[0] != expected {
			t.Fatalf("page %d after move: %v, %v", index, text, err)
		}
	}
	if err := doc.MovePage(0, 4); err == nil {
		t.Fatal("expected out-of-range destination error")
	}
	if err := doc.MovePage(-1, 0); err == nil {
		t.Fatal("expected negative source index error")
	}
	path := filepath.Join(t.TempDir(), "moved.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	doc.Close()
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	for index, expected := range []string{"B", "C", "A", "D"} {
		text, err := reopened.PageText(index)
		if err != nil || len(text) != 1 || text[0] != expected {
			t.Fatalf("reopened page %d: %v, %v", index, text, err)
		}
	}
}

func TestReordersDocumentPagesAtomically(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	for _, value := range []string{"A", "B", "C"} {
		if err := doc.AddTextPage(value, 20, 20); err != nil {
			t.Fatal(err)
		}
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.ReorderDocumentPages(0, []int{0, 0, 2}); err == nil {
		t.Fatal("expected invalid permutation")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed reorder changed document: %v", err)
	}
	if err := doc.ReorderDocumentPages(0, []int{2, 0, 1}); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddTextPage("D", 20, 20); err != nil {
		t.Fatal(err)
	}
	for index, expected := range []string{"C", "A", "B", "D"} {
		text, err := doc.PageText(index)
		if err != nil || len(text) != 1 || text[0] != expected {
			t.Fatalf("page %d: %v, %v", index, text, err)
		}
	}
}

func TestInsertsPagesAtBodyPositions(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	for _, value := range []string{"A", "C"} {
		if err := doc.AddTextPage(value, 20, 20); err != nil {
			t.Fatal(err)
		}
	}
	if index, err := doc.InsertTextPage(0, 1, "B", 20, 20); err != nil || index != 1 {
		t.Fatalf("insert text page: %d, %v", index, err)
	}
	if index, err := doc.InsertBlankPage(0, 0, 20, 20); err != nil || index != 0 {
		t.Fatalf("insert blank page: %d, %v", index, err)
	}
	for index, expected := range [][]string{{}, {"A"}, {"B"}, {"C"}} {
		text, err := doc.PageText(index)
		if err != nil || !slices.Equal(text, expected) {
			t.Fatalf("page %d: %v, %v", index, text, err)
		}
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := doc.InsertBlankPage(0, -1, 20, 20); err == nil {
		t.Fatal("accepted negative position")
	}
	if _, err := doc.InsertTextPage(0, 5, "bad", 20, 20); err == nil {
		t.Fatal("accepted out-of-range position")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(after, before) {
		t.Fatalf("invalid insert changed package: %v", err)
	}
}

func TestDuplicatesPageAcrossBinding(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("copy me", 40, 30); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddTextWatermark(0, "copied mark", [4]float64{1, 1, 12, 6}, 4); err != nil {
		t.Fatal(err)
	}
	index, err := doc.DuplicatePage(0)
	if err != nil || index != 1 {
		t.Fatalf("duplicate page: %d, %v", index, err)
	}
	text, err := doc.PageText(1)
	if err != nil || !slices.Equal(text, []string{"copy me"}) {
		t.Fatalf("copied text: %v, %v", text, err)
	}
	pages, err := doc.Pages()
	if err != nil || pages[0].ID == pages[1].ID {
		t.Fatalf("copied page IDs: %+v, %v", pages, err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 2 || annotations[0].ID == annotations[1].ID {
		t.Fatalf("copied annotations: %+v, %v", annotations, err)
	}
	svg, err := doc.PageSVG(1)
	if err != nil || !strings.Contains(svg, "copied mark") {
		t.Fatalf("copied annotation preview: %v, %v", svg, err)
	}
	if _, err := doc.DuplicatePage(-1); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.DuplicatePage(2); err == nil {
		t.Fatal("expected out-of-range index error")
	}
	output := filepath.Join(t.TempDir(), "duplicated.ofd")
	if err := doc.Save(output); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(output)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	text, err = reopened.PageText(1)
	if err != nil || !slices.Equal(text, []string{"copy me"}) {
		t.Fatalf("reopened copied text: %v, %v", text, err)
	}
	annotations, err = reopened.Annotations()
	if err != nil || len(annotations) != 2 {
		t.Fatalf("reopened annotations: %+v, %v", annotations, err)
	}
}

func TestDuplicatesPageAtBodyPosition(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	for _, value := range []string{"A", "B"} {
		if err := doc.AddTextPage(value, 40, 30); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := doc.AddTextWatermark(1, "mark", [4]float64{1, 1, 12, 6}, 4); err != nil {
		t.Fatal(err)
	}
	if index, err := doc.DuplicatePageAt(1, 0); err != nil || index != 0 {
		t.Fatalf("duplicate at position: %d, %v", index, err)
	}
	for index, expected := range []string{"B", "A", "B"} {
		text, err := doc.PageText(index)
		if err != nil || !slices.Equal(text, []string{expected}) {
			t.Fatalf("page %d: %v, %v", index, text, err)
		}
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 2 {
		t.Fatalf("copied annotations: %v, %v", annotations, err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := doc.DuplicatePageAt(-1, 0); err == nil {
		t.Fatal("accepted negative source index")
	}
	if _, err := doc.DuplicatePageAt(0, 4); err == nil {
		t.Fatal("accepted out-of-range position")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(after, before) {
		t.Fatalf("invalid duplicate changed package: %v", err)
	}
}

func TestRemovesPageAndKeepsLaterAppendOrder(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	for _, value := range []string{"A", "B", "C"} {
		if err := doc.AddTextPage(value, 20, 20); err != nil {
			t.Fatal(err)
		}
	}
	if err := doc.RemovePage(1); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddTextPage("D", 20, 20); err != nil {
		t.Fatal(err)
	}
	for index, expected := range []string{"A", "C", "D"} {
		text, err := doc.PageText(index)
		if err != nil || len(text) != 1 || text[0] != expected {
			t.Fatalf("page %d: %v, %v", index, text, err)
		}
	}
	if err := doc.RemovePage(-1); err == nil {
		t.Fatal("expected negative page index error")
	}
	if err := doc.RemovePage(3); err == nil {
		t.Fatal("expected out-of-range page index error")
	}
}

func TestValidatesPageActionDestination(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("source", 30, 20); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddTextPage("target", 30, 20); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) == 0 || objects[0].ID == nil {
		t.Fatalf("page objects: %v, %v", objects, err)
	}
	if err := doc.AddPageObjectGotoAction(0, *objects[0].ID, Destination{PageID: pages[1].ID, Kind: "Fit"}); err != nil {
		t.Fatal(err)
	}
	if issues, err := doc.ValidateReferences(); err != nil || len(issues) != 0 {
		t.Fatalf("valid destination: %v, %v", issues, err)
	}
	pageXML, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	needle := `PageID="` + pages[1].ID + `"`
	if !strings.Contains(string(pageXML), needle) {
		t.Fatal("page action destination missing")
	}
	if err := doc.SetEntry(pages[0].Path, []byte(strings.Replace(string(pageXML), needle, `PageID="999"`, 1))); err != nil {
		t.Fatal(err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	for _, issue := range issues {
		if issue.Code == "navigation_page_reference" && issue.Path == pages[0].Path {
			return
		}
	}
	t.Fatalf("missing page navigation issue: %v", issues)
}

func TestValidateReferences(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("check", 210, 297); err != nil {
		t.Fatal(err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("clean references: %v, %v", issues, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	page, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	changed := strings.Replace(string(page), `Font="1"`, `Font="999"`, 1)
	if changed == string(page) {
		t.Fatal("generated page lacks font reference")
	}
	if err := doc.SetEntry(pages[0].Path, []byte(changed)); err != nil {
		t.Fatal(err)
	}
	issues, err = doc.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, issue := range issues {
		if issue.Code == "missing_font" && issue.Path == pages[0].Path {
			found = true
		}
	}
	if !found {
		t.Fatalf("missing font issue: %v", issues)
	}
	const documentPath = "Doc_0/Document.xml"
	documentXML, err := doc.Entry(documentPath)
	if err != nil {
		t.Fatal(err)
	}
	broken := strings.Replace(string(documentXML), "</ofd:Document>", `<ofd:Bookmarks><ofd:Bookmark Name="broken"><ofd:Dest Type="Fit" PageID="999"/></ofd:Bookmark></ofd:Bookmarks></ofd:Document>`, 1)
	if err := doc.SetEntry(documentPath, []byte(broken)); err != nil {
		t.Fatal(err)
	}
	issues, err = doc.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	found = false
	for _, issue := range issues {
		if issue.Code == "navigation_page_reference" {
			found = true
		}
	}
	if !found {
		t.Fatalf("missing navigation issue: %v", issues)
	}
}

func TestCreateSaveReopen(t *testing.T) {
	path := filepath.Join(t.TempDir(), "created.ofd")
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.AddTextPage("Go & Rust", 210, 297); err != nil {
		t.Fatal(err)
	}
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	doc.Close()
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	text, err := reopened.PageText(0)
	if err != nil || len(text) != 1 || text[0] != "Go & Rust" {
		t.Fatalf("page text: %q, %v", text, err)
	}
}

func TestOFDBytesRoundTripAndInvalidInput(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.AddTextPage("内存 OFD", 40, 30); err != nil {
		t.Fatal(err)
	}
	data, err := doc.ToBytes()
	if err != nil || len(data) < 2 || string(data[:2]) != "PK" {
		t.Fatalf("serialize ofd: %v, %v", len(data), err)
	}
	doc.Close()
	if _, err := doc.ToBytes(); err == nil {
		t.Fatal("expected closed document error")
	}
	reopened, err := FromBytes(data)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	text, err := reopened.PageText(0)
	if err != nil || !slices.Equal(text, []string{"内存 OFD"}) {
		t.Fatalf("reopened page text: %v, %v", text, err)
	}
	if _, err := FromBytes(nil); err == nil {
		t.Fatal("expected empty ZIP error")
	}
	if _, err := FromBytes([]byte("not a ZIP")); err == nil {
		t.Fatal("expected invalid ZIP error")
	}
}

func TestUTF16PageXMLRoundTripPreservesOriginalBytes(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("跨语言 UTF-16", 40, 30); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	path := pages[0].Path
	source, err := doc.Entry(path)
	if err != nil {
		t.Fatal(err)
	}
	raw := []byte{0xff, 0xfe}
	for _, unit := range utf16.Encode([]rune(string(source))) {
		raw = append(raw, byte(unit), byte(unit>>8))
	}
	if err := doc.SetEntry(path, raw); err != nil {
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
	if err != nil || !reflect.DeepEqual(text, []string{"跨语言 UTF-16"}) {
		t.Fatalf("page text: %v, %v", text, err)
	}
	entry, err := reopened.Entry(path)
	if err != nil || !bytes.Equal(entry, raw) {
		t.Fatalf("raw entry changed: %v", err)
	}
}

func TestEntryEditingAndErrors(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.SetEntry("Doc_0/data.bin", []byte{0, 1, 2}); err != nil {
		t.Fatal(err)
	}
	data, err := doc.Entry("Doc_0/data.bin")
	if err != nil || len(data) != 3 || data[2] != 2 {
		t.Fatalf("entry: %v, %v", data, err)
	}
	if err := doc.SetEntry("Doc_0/empty.bin", nil); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetEntry("scratch.bin", []byte("remove")); err != nil {
		t.Fatal(err)
	}
	removed, err := doc.RemoveEntry("scratch.bin")
	if err != nil || !removed {
		t.Fatalf("remove entry: %v, %v", removed, err)
	}
	removed, err = doc.RemoveEntry("scratch.bin")
	if err != nil || removed {
		t.Fatalf("remove missing entry: %v, %v", removed, err)
	}
	if _, err := doc.RemoveEntry("../outside"); err == nil {
		t.Fatal("accepted invalid entry path")
	}
	if _, err := doc.Entry("missing.bin"); err == nil {
		t.Fatal("expected missing entry error")
	}
	if err := doc.SetEntry("bad.xml", []byte("<broken>")); err == nil {
		t.Fatal("expected XML error")
	}
	if _, err := doc.PageText(-1); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.PageText(0); err == nil {
		t.Fatal("expected out-of-range error")
	}
	if err := doc.AddTextPage("bad", -1, 297); err == nil {
		t.Fatal("expected dimension error")
	}
	if _, err := Open(filepath.Join(t.TempDir(), "missing.ofd")); err == nil {
		t.Fatal("expected open error")
	}
	doc.Close()
	if _, err := doc.PageCount(); err == nil {
		t.Fatal("expected closed document error")
	}
	if _, err := doc.Entry("OFD.xml"); err == nil {
		t.Fatal("expected closed document error")
	}
	if _, err := doc.Entries(); err == nil {
		t.Fatal("expected closed document error")
	}
	if _, err := doc.Attachments(); err == nil {
		t.Fatal("expected closed document error")
	}
	if _, err := doc.Fonts(); err == nil {
		t.Fatal("expected closed document error")
	}
	if _, err := doc.Signatures(); err == nil {
		t.Fatal("expected closed document error")
	}
	if err := doc.SetEntry("a", []byte("b")); err == nil {
		t.Fatal("expected closed document error")
	}
	if err := doc.AddTextPage("b", 210, 297); err == nil {
		t.Fatal("expected closed document error")
	}
	if err := doc.Save(filepath.Join(t.TempDir(), "closed.ofd")); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestBoundaryValidation(t *testing.T) {
	if _, err := Open("bad\x00path"); err == nil {
		t.Fatal("expected NUL path error")
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if _, err := doc.Entry("bad\x00path"); err == nil {
		t.Fatal("expected NUL entry path error")
	}
	if err := doc.SetEntry("bad\x00path", nil); err == nil {
		t.Fatal("expected NUL entry path error")
	}
	if err := doc.AddTextPage("bad\x00text", 210, 297); err != nil {
		t.Fatalf("embedded NUL text: %v", err)
	}
	if err := doc.Save("bad\x00path"); err == nil {
		t.Fatal("expected NUL output path error")
	}
	if err := doc.Save(filepath.Join(t.TempDir(), "missing", "out.ofd")); err == nil {
		t.Fatal("expected save I/O error")
	}
	if err := doc.SetEntry("Doc_0/Document.xml", []byte("<Document/>")); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.Pages(); err == nil {
		t.Fatal("expected malformed document error")
	}
	if _, err := doc.PageCount(); err == nil {
		t.Fatal("expected malformed page count error")
	}
	if _, err := doc.PageTextObjects(0); err == nil {
		t.Fatal("expected malformed text objects error")
	}
	doc.Close()
	if _, err := doc.PageText(0); err == nil {
		t.Fatal("expected closed page text error")
	}
	if _, err := doc.Pages(); err == nil {
		t.Fatal("expected closed page metadata error")
	}
	if _, err := doc.Entries(); err == nil {
		t.Fatal("expected closed entries error")
	}
	if _, err := doc.PageTextObjects(0); err == nil {
		t.Fatal("expected closed page text objects error")
	}
}

func TestNativeBufferBoundary(t *testing.T) {
	var nilDocument *Document
	nilDocument.Close()
	data, err := bytesAt(nativeBuffer{})
	if err != nil || len(data) != 0 {
		t.Fatalf("empty buffer: %v, %v", data, err)
	}
	if _, err := bytesAt(nativeBuffer{len: 1}); err == nil {
		t.Fatal("expected null data error")
	}
	value := byte(1)
	if _, err := bytesAt(nativeBuffer{data: &value, len: ^uintptr(0)}); err == nil {
		t.Fatal("expected length error")
	}
}

func TestReadsJavaAttachment(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "AddAttachment.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	items, err := doc.Attachments()
	if err != nil || len(items) != 1 || items[0].Name != "Gao" {
		t.Fatalf("attachments: %v, %v", items, err)
	}
	data, err := doc.AttachmentData(0)
	if err != nil || len(data) != 36494 {
		t.Fatalf("attachment data: %d bytes, %v", len(data), err)
	}
	if _, err := doc.AttachmentData(1); err == nil {
		t.Fatal("expected attachment index error")
	}
	if _, err := doc.AttachmentData(-1); err == nil {
		t.Fatal("expected negative index error")
	}
	doc.Close()
	if _, err := doc.Attachments(); err == nil {
		t.Fatal("expected closed document error")
	}
	if _, err := doc.AttachmentData(0); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestReadsInvoiceGraphics(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "发票示例.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	sources, err := doc.PageSources(0)
	if err != nil || len(sources) != 2 || sources[0].TemplateID == nil || *sources[0].TemplateID != "1" {
		t.Fatalf("sources: %v, %v", sources, err)
	}
	combined, err := doc.PageTextWithTemplates(0)
	if err != nil {
		t.Fatal(err)
	}
	own, err := doc.PageText(0)
	if err != nil || len(combined) <= len(own) {
		t.Fatalf("combined text: %d, own text: %d, %v", len(combined), len(own), err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) == 0 || paths[0].AbbreviatedData == nil {
		t.Fatalf("paths: %v, %v", paths, err)
	}
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 || images[0].ResourceID != "6920" || images[0].MediaPath == nil || *images[0].MediaPath != "Doc_0/Res/image_6920.png" {
		t.Fatalf("images: %v, %v", images, err)
	}
	data, err := doc.PageImageData(0, 0)
	if err != nil || len(data) != 2024 {
		t.Fatalf("image data: %d bytes, %v", len(data), err)
	}
	if _, err := doc.PagePaths(-1); err == nil {
		t.Fatal("expected negative page index")
	}
	if _, err := doc.PagePaths(1); err == nil {
		t.Fatal("expected missing page")
	}
	if _, err := doc.PageImages(-1); err == nil {
		t.Fatal("expected negative page index")
	}
	if _, err := doc.PageImages(1); err == nil {
		t.Fatal("expected missing page")
	}
	if _, err := doc.PageImageData(0, 1); err == nil {
		t.Fatal("expected missing image")
	}
	if _, err := doc.PageImageData(0, -1); err == nil {
		t.Fatal("expected negative image index")
	}
	if _, err := doc.PageSources(-1); err == nil {
		t.Fatal("expected negative page index")
	}
	if _, err := doc.PageSources(1); err == nil {
		t.Fatal("expected missing page")
	}
	if _, err := doc.PageTextWithTemplates(-1); err == nil {
		t.Fatal("expected negative page index")
	}
	if _, err := doc.PageTextWithTemplates(1); err == nil {
		t.Fatal("expected missing page")
	}
	doc.Close()
	if _, err := doc.PagePaths(0); err == nil {
		t.Fatal("expected closed document")
	}
	if _, err := doc.PageImages(0); err == nil {
		t.Fatal("expected closed document")
	}
	if _, err := doc.PageImageData(0, 0); err == nil {
		t.Fatal("expected closed document")
	}
	if _, err := doc.PageSources(0); err == nil {
		t.Fatal("expected closed document")
	}
	if _, err := doc.PageTextWithTemplates(0); err == nil {
		t.Fatal("expected closed document")
	}
}

func TestCreatesImagePage(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "发票示例.ofd")
	source, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	png, err := source.PageImageData(0, 0)
	source.Close()
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddImagePage([]byte("bad"), "png", 210, 140); err == nil {
		t.Fatal("expected invalid image error")
	}
	for _, sample := range []struct {
		format string
		data   []byte
	}{
		{"png", []byte("\x89PNG\r\n\x1a\n")},
		{"jpeg", []byte("\xff\xd8\xff")},
	} {
		if err := doc.AddImagePage(sample.data, sample.format, 210, 140); err == nil {
			t.Fatalf("expected truncated %s error", sample.format)
		}
	}
	if err := doc.AddImagePage(png, "png", 210, 140); err != nil {
		t.Fatal(err)
	}
	pageImages, err := doc.PageImages(0)
	if err != nil || len(pageImages) != 1 || !slices.Equal(pageImages[0].CTM, []float64{210, 0, 0, 140, 0, 0}) {
		t.Fatalf("image placement: %v, %v", pageImages, err)
	}
	data, err := doc.PageImageData(0, 0)
	if err != nil || len(data) != len(png) {
		t.Fatalf("image bytes: %d, %v", len(data), err)
	}
	path := filepath.Join(t.TempDir(), "image.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	doc.Close()
	if err := doc.AddImagePage(png, "png", 210, 140); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestWritesGIFImageFromJavaFixture(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-converter", "src", "test", "resources", "不规范资源路径.ofd")
	source, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	gif, err := source.PageImageData(0, 0)
	source.Close()
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddImagePage([]byte("GIF89a"), "gif", 30, 20); err == nil {
		t.Fatal("expected truncated GIF error")
	}
	if err := doc.AddImagePage(gif, "gif", 30, 20); err != nil {
		t.Fatal(err)
	}
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 || images[0].Format == nil || *images[0].Format != "GIF" {
		t.Fatalf("GIF image metadata: %v, %v", images, err)
	}
	data, err := doc.PageImageData(0, 0)
	if err != nil || !bytes.Equal(data, gif) {
		t.Fatalf("GIF image bytes: %d, %v", len(data), err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "data:image/png;base64,") {
		t.Fatalf("GIF image preview: %d, %v", len(svg), err)
	}
	path := filepath.Join(t.TempDir(), "gif.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	data, err = reopened.PageImageData(0, 0)
	if err != nil || !bytes.Equal(data, gif) {
		t.Fatalf("saved GIF image bytes: %d, %v", len(data), err)
	}
}

func TestCreatesJPEG2000ImagePage(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "red-32x32.jp2")
	jp2, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddImagePage(jp2[:20], "jp2", 32, 32); err == nil {
		t.Fatal("expected truncated JPEG2000 error")
	}
	if err := doc.AddImagePage(jp2, "jp2", 32, 32); err != nil {
		t.Fatal(err)
	}
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 || images[0].Format == nil || *images[0].Format != "JPEG2000" {
		t.Fatalf("JPEG2000 image: %v, %v", images, err)
	}
	data, err := doc.PageImageData(0, 0)
	if err != nil || !bytes.Equal(data, jp2) {
		t.Fatalf("image bytes: %d, %v", len(data), err)
	}
	preview, err := doc.PagePNG(0, 25.4)
	if err != nil || !bytes.HasPrefix(preview, []byte("\x89PNG")) {
		t.Fatalf("PNG preview: %d, %v", len(preview), err)
	}
	path := filepath.Join(t.TempDir(), "jpeg2000.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	data, err = reopened.PageImageData(0, 0)
	if err != nil || !bytes.Equal(data, jp2) {
		t.Fatalf("reopened image bytes: %d, %v", len(data), err)
	}
}

func TestImportsPDFJPXDecodeImage(t *testing.T) {
	fixtures := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	pdf, err := os.ReadFile(filepath.Join(fixtures, "red-32x32-jpx.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	jp2, err := os.ReadFile(filepath.Join(fixtures, "red-32x32.jp2"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := FromPDF(pdf)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 || images[0].Format == nil || *images[0].Format != "JPEG2000" {
		t.Fatalf("imported image: %v, %v", images, err)
	}
	data, err := doc.PageImageData(0, 0)
	if err != nil || !bytes.Equal(data, jp2) {
		t.Fatalf("imported bytes: %d, %v", len(data), err)
	}
	preview, err := doc.PagePNG(0, 25.4)
	if err != nil || !bytes.HasPrefix(preview, []byte("\x89PNG")) {
		t.Fatalf("preview: %d, %v", len(preview), err)
	}
	preblendedPDF, err := os.ReadFile(filepath.Join(fixtures, "red-premultiplied-jpx.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	preblended, err := FromPDF(preblendedPDF)
	if err != nil {
		t.Fatal(err)
	}
	defer preblended.Close()
	images, err = preblended.PageImages(0)
	if err != nil || len(images) != 1 || images[0].Format == nil || *images[0].Format != "PNG" {
		t.Fatalf("preblended image: %v, %v", images, err)
	}
	data, err = preblended.PageImageData(0, 0)
	if err != nil || !bytes.HasPrefix(data, []byte("\x89PNG")) {
		t.Fatalf("preblended bytes: %d, %v", len(data), err)
	}
	maskedPDF, err := os.ReadFile(filepath.Join(fixtures, "red-blue-jpx-mask.pdf"))
	if err != nil {
		t.Fatal(err)
	}
	masked, err := FromPDF(maskedPDF)
	if err != nil {
		t.Fatal(err)
	}
	defer masked.Close()
	images, err = masked.PageImages(0)
	if err != nil || len(images) != 1 || images[0].Format == nil || *images[0].Format != "PNG" {
		t.Fatalf("masked JPX image: %v, %v", images, err)
	}
	data, err = masked.PageImageData(0, 0)
	if err != nil || !bytes.HasPrefix(data, []byte("\x89PNG")) {
		t.Fatalf("masked JPX bytes: %d, %v", len(data), err)
	}
}

func TestAddsImageObject(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "发票示例.ofd")
	source, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	png, err := source.PageImageData(0, 0)
	source.Close()
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("before", 210, 297); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddImageObject(0, png, "png", [4]float64{10, 20, 30, 40})
	if err != nil || id == 0 {
		t.Fatalf("add image object: %d, %v", id, err)
	}
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 || images[0].ID == nil || *images[0].ID != fmt.Sprint(id) ||
		!slices.Equal(images[0].CTM, []float64{30, 0, 0, 40, 0, 0}) {
		t.Fatalf("images: %v, %v", images, err)
	}
	data, err := doc.PageImageData(0, 0)
	if err != nil || len(data) != len(png) {
		t.Fatalf("image data: %d, %v", len(data), err)
	}
	if _, err := doc.AddImageObject(-1, png, "png", [4]float64{0, 0, 10, 10}); err == nil {
		t.Fatal("expected negative page index")
	}
	if _, err := doc.AddImageObject(0, []byte("bad"), "png", [4]float64{0, 0, 10, 10}); err == nil {
		t.Fatal("expected invalid image")
	}
	path := filepath.Join(t.TempDir(), "image_object.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	images, err = reopened.PageImages(0)
	if err != nil || len(images) != 1 {
		t.Fatalf("reopened images: %v, %v", images, err)
	}
	doc.Close()
	if _, err := doc.AddImageObject(0, png, "png", [4]float64{0, 0, 10, 10}); err == nil {
		t.Fatal("expected closed document error")
	}
	existingPath := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "helloworld.ofd")
	existing, err := Open(existingPath)
	if err != nil {
		t.Fatal(err)
	}
	defer existing.Close()
	if _, err := existing.AddImageObject(0, png, "png", [4]float64{10, 20, 30, 40}); err != nil {
		t.Fatal(err)
	}
	existingOutput := filepath.Join(t.TempDir(), "existing_image.ofd")
	if err := existing.Save(existingOutput); err != nil {
		t.Fatal(err)
	}
	check, err := Open(existingOutput)
	if err != nil {
		t.Fatal(err)
	}
	defer check.Close()
	images, err = check.PageImages(0)
	if err != nil || len(images) != 1 {
		t.Fatalf("existing images: %v, %v", images, err)
	}
}

func TestCreatesPathPage(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddPathPage("", [4]float64{10, 10, 20, 10}, 210, 297, false, true); err == nil {
		t.Fatal("expected empty command error")
	}
	if err := doc.AddPathPage("M 0", [4]float64{10, 10, 20, 10}, 210, 297, false, true); err == nil {
		t.Fatal("expected malformed command error")
	}
	if err := doc.AddPathPage("M 0 0 L 20 10", [4]float64{10, 10, 20, 10}, 210, 297, false, true); err != nil {
		t.Fatal(err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 1 || paths[0].AbbreviatedData == nil || *paths[0].AbbreviatedData != "M 0 0 L 20 10" {
		t.Fatalf("paths: %v, %v", paths, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	pagePath := pages[0].Path
	original, err := doc.Entry(pagePath)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetEntry(pagePath, bytes.Replace(original, []byte("M 0 0 L 20 10"), []byte("X 0 0"), 1)); err != nil {
		t.Fatal(err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, issue := range issues {
		if issue.Code == "invalid_path_data" && issue.Path == pagePath {
			found = true
		}
	}
	if !found {
		t.Fatalf("missing invalid_path_data issue: %v", issues)
	}
	if err := doc.SetEntry(pagePath, original); err != nil {
		t.Fatal(err)
	}
	clip := []byte(`<ofd:Clips><ofd:Clip><ofd:Area><ofd:Path Boundary="0 0 10 10"><ofd:AbbreviatedData>X 0 0</ofd:AbbreviatedData></ofd:Path></ofd:Area></ofd:Clip></ofd:Clips>`)
	malformedClip := bytes.Replace(original, []byte("</ofd:PathObject>"), append(clip, []byte("</ofd:PathObject>")...), 1)
	if err := doc.SetEntry(pagePath, malformedClip); err != nil {
		t.Fatal(err)
	}
	issues, err = doc.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	found = false
	for _, issue := range issues {
		if issue.Code == "invalid_path_data" && strings.Contains(issue.Message, "clip Path") {
			found = true
		}
	}
	if !found {
		t.Fatalf("missing clip path issue: %v", issues)
	}
	if err := doc.SetEntry(pagePath, original); err != nil {
		t.Fatal(err)
	}
	doc.Close()
	if err := doc.AddPathPage("M 0 0", [4]float64{0, 0, 20, 10}, 210, 297, false, true); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestCreatesAndAppendsBlankPages(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	if err := doc.AppendBlankPage(0, 20, 20); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBlankPage(0, 20); err == nil {
		t.Fatal("expected invalid page dimensions")
	}
	if err := doc.AppendBlankPage(-1, 20, 20); err == nil {
		t.Fatal("expected negative document index")
	}
	text, err := doc.PageText(0)
	if err != nil || len(text) != 0 {
		t.Fatalf("blank page text: %v, %v", text, err)
	}
	path := filepath.Join(t.TempDir(), "blank.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	if count, err := reopened.PageCount(); err != nil || count != 2 {
		t.Fatalf("blank page count: %d, %v", count, err)
	}
	if issues, err := reopened.ValidateReferences(); err != nil || len(issues) != 0 {
		t.Fatalf("blank page references: %v, %v", issues, err)
	}
	javaPath := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "helloworld.ofd")
	existing, err := Open(javaPath)
	if err != nil {
		t.Fatal(err)
	}
	defer existing.Close()
	if err := existing.AppendBlankPage(0, 40, 30); err != nil {
		t.Fatal(err)
	}
	if err := existing.AddBlankPage(40, 30); err == nil {
		t.Fatal("expected existing document error")
	}
	if text, err := existing.PageText(1); err != nil || len(text) != 0 {
		t.Fatalf("existing blank page text: %v, %v", text, err)
	}
}

func TestPlainAndStyledFlowKeepJoinedEmojiTogether(t *testing.T) {
	font, err := os.ReadFile(filepath.Join("..", "..", "..", "..", "ofdrw-layout", "src", "test", "resources", "simhei-cut1.ttf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	fontID, err := doc.AddEmbeddedFont("SimHei", font)
	if err != nil {
		t.Fatal(err)
	}
	style := FlowTextStyle{PageSize: [2]float64{5, 8}, Margins: [4]float64{1, 1, 1, 1}, FontSize: 1, LineHeight: 1.5, FontID: fmt.Sprint(fontID)}
	if pages, err := doc.AddFlowedText(0, "X👩\u200d💻", style); err != nil || !slices.Equal(pages, []int{0}) {
		t.Fatalf("plain flow: %v, %v", pages, err)
	}
	if text, err := doc.PageText(0); err != nil || !slices.Equal(text, []string{"X", "👩\u200d💻"}) {
		t.Fatalf("plain flow text: %v, %v", text, err)
	}
	if pages, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "X👩\u200d💻"}}, style); err != nil || !slices.Equal(pages, []int{1}) {
		t.Fatalf("styled flow: %v, %v", pages, err)
	}
	if text, err := doc.PageText(1); err != nil || !slices.Equal(text, []string{"X", "👩\u200d💻"}) {
		t.Fatalf("styled flow text: %v, %v", text, err)
	}
	wordStyle := style
	wordStyle.PageSize = [2]float64{6, 8}
	if pages, err := doc.AddFlowedText(0, "hello world", wordStyle); err != nil || !slices.Equal(pages, []int{2}) {
		t.Fatalf("plain word flow: %v, %v", pages, err)
	}
	if text, err := doc.PageText(2); err != nil || !slices.Equal(text, []string{"hello ", "world"}) {
		t.Fatalf("plain word flow text: %v, %v", text, err)
	}
	if pages, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "hello "}, {Text: "world"}}, wordStyle); err != nil || !slices.Equal(pages, []int{3}) {
		t.Fatalf("styled word flow: %v, %v", pages, err)
	}
	if text, err := doc.PageText(3); err != nil || !slices.Equal(text, []string{"hello ", "world"}) {
		t.Fatalf("styled word flow text: %v, %v", text, err)
	}
	if pages, err := doc.AddFlowedText(0, "A\r\nB\u2028C", wordStyle); err != nil || !slices.Equal(pages, []int{4}) {
		t.Fatalf("plain hard breaks: %v, %v", pages, err)
	}
	if text, err := doc.PageText(4); err != nil || !slices.Equal(text, []string{"A", "B", "C"}) {
		t.Fatalf("plain hard break text: %v, %v", text, err)
	}
	if pages, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "A\r"}, {Text: "\nB\u2028C"}}, wordStyle); err != nil || !slices.Equal(pages, []int{5}) {
		t.Fatalf("styled hard breaks: %v, %v", pages, err)
	}
	if text, err := doc.PageText(5); err != nil || !slices.Equal(text, []string{"A", "B", "C"}) {
		t.Fatalf("styled hard break text: %v, %v", text, err)
	}
	path := filepath.Join(t.TempDir(), "emoji-flow.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	if text, err := reopened.PageText(1); err != nil || !slices.Equal(text, []string{"X", "👩\u200d💻"}) {
		t.Fatalf("reopened styled flow: %v, %v", text, err)
	}
	if text, err := reopened.PageText(3); err != nil || !slices.Equal(text, []string{"hello ", "world"}) {
		t.Fatalf("reopened word flow: %v, %v", text, err)
	}
	if text, err := reopened.PageText(5); err != nil || !slices.Equal(text, []string{"A", "B", "C"}) {
		t.Fatalf("reopened hard breaks: %v, %v", text, err)
	}
}

func TestFlowsTextAcrossPages(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := FlowTextStyle{PageSize: [2]float64{40, 22}, Margins: [4]float64{5, 5, 5, 5}, FontSize: 5, LineHeight: 6}
	indexes, err := doc.AddFlowedText(0, "A\nB\nC", style)
	if err != nil || !slices.Equal(indexes, []int{0, 1}) {
		t.Fatalf("flowed page indexes: %v, %v", indexes, err)
	}
	if text, err := doc.PageText(0); err != nil || !slices.Equal(text, []string{"A", "B"}) {
		t.Fatalf("first page text: %v, %v", text, err)
	}
	if _, err := doc.AddFlowedText(-1, "bad", style); err == nil {
		t.Fatal("expected negative document index")
	}
	if _, err := doc.AddFlowedText(0, "bad", FlowTextStyle{PageSize: [2]float64{9, 22}, Margins: [4]float64{5, 5, 5, 5}, FontSize: 5, LineHeight: 6}); err == nil {
		t.Fatal("expected invalid text area")
	}
	invalidNumber := style
	invalidNumber.FontSize = math.NaN()
	if _, err := doc.AddFlowedText(0, "bad", invalidNumber); err == nil {
		t.Fatal("expected invalid JSON number")
	}
	if count, err := doc.PageCount(); err != nil || count != 2 {
		t.Fatalf("page count after failed flow: %d, %v", count, err)
	}
	path := filepath.Join(t.TempDir(), "flowed.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	if text, err := reopened.PageText(1); err != nil || !slices.Equal(text, []string{"C"}) {
		t.Fatalf("reopened flowed text: %v, %v", text, err)
	}
	if issues, err := reopened.ValidateReferences(); err != nil || len(issues) != 0 {
		t.Fatalf("flowed page references: %v, %v", issues, err)
	}
	alignStyle := style
	alignStyle.Align = "right"
	alignStyle.FirstLineIndent = 4
	if indexes, err := doc.AddFlowedText(0, "A\nB", alignStyle); err != nil || !slices.Equal(indexes, []int{2}) {
		t.Fatalf("aligned flowed page: %v, %v", indexes, err)
	}
	objects, err := doc.PageTextObjects(2)
	if err != nil || len(objects) != 2 {
		t.Fatalf("aligned text objects: %v, %v", objects, err)
	}
	for _, object := range objects {
		if math.Abs(object.Boundary[0]+object.Boundary[2]-35) > 1e-6 {
			t.Fatalf("line does not end at right margin: %v", object.Boundary)
		}
	}
	alignStyle.Align = "justify"
	if _, err := doc.AddFlowedText(0, "bad", alignStyle); err == nil {
		t.Fatal("expected invalid alignment")
	}
	if indexes, err := doc.AddFlowedText(0, "A\n", style); err != nil || !slices.Equal(indexes, []int{3}) {
		t.Fatalf("trailing newline page: %v, %v", indexes, err)
	}
	if text, err := doc.PageText(3); err != nil || !slices.Equal(text, []string{"A"}) {
		t.Fatalf("trailing newline text: %v, %v", text, err)
	}
	defaultDoc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	if indexes, err := defaultDoc.AddFlowedText(0, "Default page", FlowTextStyle{}); err != nil || !slices.Equal(indexes, []int{0}) {
		t.Fatalf("default flowed style: %v, %v", indexes, err)
	}
	if text, err := defaultDoc.PageText(0); err != nil || !slices.Equal(text, []string{"Default page"}) {
		t.Fatalf("default flowed text: %v, %v", text, err)
	}
	defaultDoc.Close()
	if _, err := defaultDoc.AddFlowedText(0, "closed", FlowTextStyle{}); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestFlowsStyledSpansAcrossPages(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	size := 3.0
	underlineOffset := 0.5
	underlineWidth := 0.2
	red := [3]uint8{255, 0, 0}
	blue := [3]uint8{0, 0, 255}
	style := FlowTextStyle{PageSize: [2]float64{40, 16}, Margins: [4]float64{5, 5, 5, 5}, FontSize: 5, LineHeight: 6}
	indexes, err := doc.AddFlowedSpans(0, []FlowSpan{
		{Text: "AA", Bold: true, FillColor: &red, LetterSpacing: 0.5, Underline: true, UnderlineOffset: &underlineOffset, UnderlineWidth: &underlineWidth, LineBreakAfter: true},
		{Text: "B\nC", FontSize: &size, Italic: true, FillColor: &blue, KeepTogether: true},
	}, style)
	if err != nil || !slices.Equal(indexes, []int{0, 1, 2}) {
		t.Fatalf("flowed span indexes: %v, %v", indexes, err)
	}
	if text, err := doc.PageText(0); err != nil || !slices.Equal(text, []string{"AA"}) {
		t.Fatalf("first page styled text: %v, %v", text, err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || objects[0].Size == nil || *objects[0].Size != 5 {
		t.Fatalf("styled text objects: %v, %v", objects, err)
	}
	if text, err := doc.PageText(1); err != nil || !slices.Equal(text, []string{"B"}) {
		t.Fatalf("second page styled text: %v, %v", text, err)
	}
	if second, err := doc.PageTextObjects(1); err != nil || len(second) != 1 || second[0].Size == nil || *second[0].Size != 3 {
		t.Fatalf("second page styled object: %v, %v", second, err)
	}
	if text, err := doc.PageText(2); err != nil || !slices.Equal(text, []string{"C"}) {
		t.Fatalf("third page styled text: %v, %v", text, err)
	}
	if len(objects[0].Codes) != 1 || objects[0].Codes[0].DeltaX == nil {
		t.Fatalf("missing letter spacing: %v", objects[0].Codes)
	}
	if paths, err := doc.PagePaths(0); err != nil || len(paths) != 1 {
		t.Fatalf("underline path: %v, %v", paths, err)
	}
	if svg, err := doc.PageSVG(0); err != nil || !strings.Contains(svg, "#ff0000") {
		t.Fatalf("styled SVG: %v, %v", svg, err)
	}
	if svg, err := doc.PageSVG(1); err != nil || !strings.Contains(svg, "#0000ff") {
		t.Fatalf("second page styled SVG: %v, %v", svg, err)
	}
	size = 9
	if _, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "bad", FontSize: &size}}, style); err == nil {
		t.Fatal("expected invalid span size")
	}
	if count, err := doc.PageCount(); err != nil || count != 3 {
		t.Fatalf("page count after failed styled flow: %d, %v", count, err)
	}
	if _, err := doc.AddFlowedSpans(-1, []FlowSpan{{Text: "bad"}}, style); err == nil {
		t.Fatal("expected negative document index")
	}
	size = math.NaN()
	if _, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "bad", FontSize: &size}}, style); err == nil {
		t.Fatal("expected invalid JSON number")
	}
	if indexes, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "D\n"}}, style); err != nil || !slices.Equal(indexes, []int{3}) {
		t.Fatalf("trailing newline styled page: %v, %v", indexes, err)
	}
	if text, err := doc.PageText(3); err != nil || !slices.Equal(text, []string{"D"}) {
		t.Fatalf("trailing newline styled text: %v, %v", text, err)
	}
	defaultDoc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	if indexes, err := defaultDoc.AddFlowedSpans(0, []FlowSpan{{Text: "default"}}, FlowTextStyle{}); err != nil || !slices.Equal(indexes, []int{0}) {
		t.Fatalf("default styled flow: %v, %v", indexes, err)
	}
	defaultDoc.Close()
	if _, err := defaultDoc.AddFlowedSpans(0, []FlowSpan{{Text: "closed"}}, FlowTextStyle{}); err == nil {
		t.Fatal("expected closed document")
	}
}

func TestPageLayersFollowTypeOrder(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	path := pages[0].Path
	xml, err := doc.Entry(path)
	if err != nil {
		t.Fatal(err)
	}
	xml = bytes.Replace(xml, []byte("<ofd:Layer "), []byte("<ofd:Layer Type=\"Foreground\" "), 1)
	xml = bytes.Replace(xml, []byte("</ofd:Content>"), []byte("<ofd:Layer ID=\"80\" Type=\"Background\"/></ofd:Content>"), 1)
	if err := doc.SetEntry(path, xml); err != nil {
		t.Fatal(err)
	}
	layers, err := doc.PageLayers(0)
	if err != nil || len(layers) != 2 || layers[0].LayerType != "Background" || layers[1].LayerType != "Foreground" || layers[0].SourceLayerIndex != 1 || layers[1].SourceLayerIndex != 0 {
		t.Fatalf("page layers: %v, %v", layers, err)
	}
	if _, err := doc.PageLayers(-1); err == nil {
		t.Fatal("expected negative page index error")
	}
	if _, err := doc.PageLayers(1); err == nil {
		t.Fatal("expected missing page error")
	}
	doc.Close()
	if _, err := doc.PageLayers(0); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestFlowedCharacterIndentUsesFirstSpanFontSize(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := FlowTextStyle{
		PageSize: [2]float64{40, 22}, Margins: [4]float64{5, 5, 5, 5},
		FontSize: 5, LineHeight: 6, FirstLineIndentChars: 2,
	}
	if pages, err := doc.AddFlowedText(0, "A\nB", style); err != nil || !slices.Equal(pages, []int{0}) {
		t.Fatalf("flowed text pages: %v, %v", pages, err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 2 || objects[0].Boundary[0] != 15 || objects[1].Boundary[0] != 5 {
		t.Fatalf("flowed text positions: %v, %v", objects, err)
	}
	size := 3.0
	spacing := 0.5
	if pages, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "A\nB", FontSize: &size, LetterSpacing: spacing}}, style); err != nil || !slices.Equal(pages, []int{1}) {
		t.Fatalf("flowed span pages: %v, %v", pages, err)
	}
	objects, err = doc.PageTextObjects(1)
	if err != nil || len(objects) != 2 || objects[0].Boundary[0] != 12 || objects[1].Boundary[0] != 5 {
		t.Fatalf("flowed span positions: %v, %v", objects, err)
	}
	style.IndentEachParagraph = true
	if pages, err := doc.AddFlowedText(0, "A\nB", style); err != nil || !slices.Equal(pages, []int{2}) {
		t.Fatalf("repeated plain indent pages: %v, %v", pages, err)
	}
	objects, err = doc.PageTextObjects(2)
	if err != nil || len(objects) != 2 || objects[0].Boundary[0] != 15 || objects[1].Boundary[0] != 15 {
		t.Fatalf("repeated plain indent: %v, %v", objects, err)
	}
	if pages, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "A\nB", FontSize: &size, LetterSpacing: spacing}}, style); err != nil || !slices.Equal(pages, []int{3}) {
		t.Fatalf("repeated styled indent pages: %v, %v", pages, err)
	}
	objects, err = doc.PageTextObjects(3)
	if err != nil || len(objects) != 2 || objects[0].Boundary[0] != 12 || objects[1].Boundary[0] != 12 {
		t.Fatalf("repeated styled indent: %v, %v", objects, err)
	}
	style.FirstLineIndentChars = -1
	if _, err := doc.AddFlowedText(0, "A", style); err == nil {
		t.Fatal("expected negative indent count error")
	}
}

func TestFlowedParagraphGapAcrossPages(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := FlowTextStyle{PageSize: [2]float64{30, 22}, Margins: [4]float64{2, 2, 2, 2},
		FontSize: 4, LineHeight: 5, ParagraphGap: 3}
	if pages, err := doc.AddFlowedText(0, "A\nB\nC", style); err != nil || !slices.Equal(pages, []int{0, 1}) {
		t.Fatalf("plain paragraph pages: %v, %v", pages, err)
	}
	if pages, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "A\nB\nC"}}, style); err != nil || !slices.Equal(pages, []int{2, 3}) {
		t.Fatalf("styled paragraph pages: %v, %v", pages, err)
	}
	for _, firstPage := range []int{0, 2} {
		objects, err := doc.PageTextObjects(firstPage)
		if err != nil || len(objects) != 2 || objects[0].Boundary[1] != 2 || objects[1].Boundary[1] != 10 {
			t.Fatalf("paragraph positions: %v, %v", objects, err)
		}
		objects, err = doc.PageTextObjects(firstPage + 1)
		if err != nil || len(objects) != 1 || objects[0].Boundary[1] != 2 {
			t.Fatalf("paragraph at page top: %v, %v", objects, err)
		}
	}
	style.ParagraphGap = -1
	if _, err := doc.AddFlowedText(0, "A", style); err == nil {
		t.Fatal("expected invalid paragraph gap")
	}
	if count, err := doc.PageCount(); err != nil || count != 4 {
		t.Fatalf("invalid paragraph gap changed page count: %v, %v", count, err)
	}
}

func TestFlowedParagraphsStayTogether(t *testing.T) {
	fontPath := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-serif.ttf")
	font, err := os.ReadFile(fontPath)
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
	style := FlowTextStyle{PageSize: [2]float64{14, 18}, Margins: [4]float64{2, 2, 2, 2},
		FontSize: 4, LineHeight: 5, ParagraphGap: 2, KeepParagraphsTogether: true, FontID: fmt.Sprint(fontID)}
	if pages, err := doc.AddFlowedText(0, "A\nBBBBB", style); err != nil || !slices.Equal(pages, []int{0, 1}) {
		t.Fatalf("plain paragraph pages: %v, %v", pages, err)
	}
	if pages, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "A\nBBBBB"}}, style); err != nil || !slices.Equal(pages, []int{2, 3}) {
		t.Fatalf("styled paragraph pages: %v, %v", pages, err)
	}
	for _, firstPage := range []int{0, 2} {
		text, err := doc.PageText(firstPage)
		if err != nil || !slices.Equal(text, []string{"A"}) {
			t.Fatalf("paragraph first page: %v, %v", text, err)
		}
		objects, err := doc.PageTextObjects(firstPage + 1)
		if err != nil || len(objects) != 2 || objects[0].Boundary[1] != 2 {
			t.Fatalf("paragraph second page: %v, %v", objects, err)
		}
	}
	if _, err := doc.AddFlowedText(0, strings.Repeat("B", 40), style); err == nil {
		t.Fatal("expected overheight paragraph error")
	}
	if count, err := doc.PageCount(); err != nil || count != 4 {
		t.Fatalf("overheight paragraph changed page count: %v, %v", count, err)
	}
}

func TestLayoutWritersRejectInvalidIndexesAndNonFiniteNumbers(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	flow := DefaultFlowTextStyle()
	table := DefaultTableStyle()
	table.ColumnWidths = []float64{10}
	cells := []TableCell{{Row: 0, Column: 0, RowSpan: 1, ColumnSpan: 1, Text: "A"}}
	checks := []struct {
		name string
		run  func() error
	}{
		{"flow negative index", func() error { _, err := doc.AddFlowedText(-1, "A", flow); return err }},
		{"spans negative index", func() error { _, err := doc.AddFlowedSpans(-1, []FlowSpan{{Text: "A"}}, flow); return err }},
		{"table negative index", func() error { _, err := doc.AddTable(-1, [][]string{{"A"}}, table); return err }},
		{"cells negative index", func() error { _, err := doc.AddTableCells(-1, 1, cells, table); return err }},
		{"cells negative rows", func() error { _, err := doc.AddTableCells(0, -1, cells, table); return err }},
		{"flow nonfinite gap", func() error {
			bad := flow
			bad.ParagraphGap = math.NaN()
			_, err := doc.AddFlowedText(0, "A", bad)
			return err
		}},
		{"spans nonfinite gap", func() error {
			bad := flow
			bad.ParagraphGap = math.NaN()
			_, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "A"}}, bad)
			return err
		}},
		{"table nonfinite width", func() error {
			bad := table
			bad.ColumnWidths = []float64{math.NaN()}
			_, err := doc.AddTable(0, [][]string{{"A"}}, bad)
			return err
		}},
		{"cells nonfinite width", func() error {
			bad := table
			bad.ColumnWidths = []float64{math.NaN()}
			_, err := doc.AddTableCells(0, 1, cells, bad)
			return err
		}},
	}
	for _, check := range checks {
		if err := check.run(); err == nil {
			t.Errorf("%s: expected error", check.name)
		}
	}
	if count, err := doc.PageCount(); err != nil || count != 0 {
		t.Fatalf("invalid layout changed pages: %v, %v", count, err)
	}
	doc.Close()
	for _, check := range checks[:4] {
		if err := check.run(); err == nil {
			t.Errorf("%s: expected closed document error", check.name)
		}
	}
}

func TestFlowedTextAndSpansPreserveSpaces(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := FlowTextStyle{PageSize: [2]float64{40, 22}, Margins: [4]float64{5, 5, 5, 5}, FontSize: 5, LineHeight: 6}
	if indexes, err := doc.AddFlowedText(0, " A \n B ", style); err != nil || !slices.Equal(indexes, []int{0}) {
		t.Fatalf("plain flowed spaces: %v, %v", indexes, err)
	}
	if indexes, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: " A \n B "}}, style); err != nil || !slices.Equal(indexes, []int{1}) {
		t.Fatalf("styled flowed spaces: %v, %v", indexes, err)
	}
	for page := 0; page < 2; page++ {
		if text, err := doc.PageText(page); err != nil || !slices.Equal(text, []string{" A ", " B "}) {
			t.Fatalf("page %d spaces: %v, %v", page, text, err)
		}
	}
	path := filepath.Join(t.TempDir(), "flowed-spaces.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	if text, err := reopened.PageText(1); err != nil || !slices.Equal(text, []string{" A ", " B "}) {
		t.Fatalf("reopened spaces: %v, %v", text, err)
	}
}

func TestFlowedSpansUseVariableLineHeights(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	small := 3.0
	style := FlowTextStyle{PageSize: [2]float64{40, 20}, Margins: [4]float64{5, 5, 5, 5}, FontSize: 5, LineHeight: 6}
	indexes, err := doc.AddFlowedSpans(0, []FlowSpan{
		{Text: "A"}, {Text: "x\n", FontSize: &small},
		{Text: "B\n", FontSize: &small}, {Text: "C", FontSize: &small},
	}, style)
	if err != nil || !slices.Equal(indexes, []int{0, 1}) {
		t.Fatalf("variable line pages: %v, %v", indexes, err)
	}
	if content, err := doc.PageText(0); err != nil || !slices.Equal(content, []string{"A", "x", "B"}) {
		t.Fatalf("first page text: %v, %v", content, err)
	}
	if content, err := doc.PageText(1); err != nil || !slices.Equal(content, []string{"C"}) {
		t.Fatalf("second page text: %v, %v", content, err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 3 || objects[0].Boundary[3] != 6 || objects[1].Boundary[3] != 6 || objects[2].Boundary[1] != 11 || objects[2].Boundary[3] != 4 {
		t.Fatalf("variable line objects: %v, %v", objects, err)
	}
	path := filepath.Join(t.TempDir(), "variable-line-height.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	if content, err := reopened.PageText(1); err != nil || !slices.Equal(content, []string{"C"}) {
		t.Fatalf("reopened text: %v, %v", content, err)
	}
}

func TestFlowedTextRejectsGlyphWiderThanContentArea(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := FlowTextStyle{PageSize: [2]float64{11, 20}, Margins: [4]float64{5, 5, 5, 5}, FontSize: 5, LineHeight: 6}
	if _, err := doc.AddFlowedText(0, "中", style); err == nil {
		t.Fatal("expected overwide plain glyph error")
	}
	if _, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "中"}}, style); err == nil {
		t.Fatal("expected overwide styled glyph error")
	}
	if count, err := doc.PageCount(); err != nil || count != 0 {
		t.Fatalf("failed flow changed page count: %d, %v", count, err)
	}
}

func TestFlowedSpansWeightFillAndStroke(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	fill := false
	weight := uint16(600)
	strokeWidth := 0.2
	strokeColor := [3]uint8{20, 40, 60}
	style := FlowTextStyle{PageSize: [2]float64{40, 20}, Margins: [4]float64{5, 5, 5, 5}, FontSize: 5, LineHeight: 6}
	indexes, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "描边", Weight: &weight, Bold: true, Fill: &fill, Stroke: true, StrokeColor: &strokeColor, StrokeWidth: &strokeWidth}}, style)
	if err != nil || !slices.Equal(indexes, []int{0}) {
		t.Fatalf("outline span: %v, %v", indexes, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "font-weight=\"600\"") || !strings.Contains(svg, "fill=\"none\"") || !strings.Contains(svg, "stroke=\"#14283c\"") {
		t.Fatalf("outline SVG: %v, %v", svg, err)
	}
	invalid := uint16(650)
	if _, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "bad", Weight: &invalid}}, style); err == nil {
		t.Fatal("expected invalid weight")
	}
	if count, err := doc.PageCount(); err != nil || count != 1 {
		t.Fatalf("failed styled flow changed page count: %d, %v", count, err)
	}
	path := filepath.Join(t.TempDir(), "styled-outline.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	if text, err := reopened.PageText(0); err != nil || !slices.Equal(text, []string{"描边"}) {
		t.Fatalf("reopened styled text: %v, %v", text, err)
	}
}

func TestFlowedTextUsesEmbeddedFontPairKerning(t *testing.T) {
	fontPath := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "kerning-simhei.ttf")
	font, err := os.ReadFile(fontPath)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	fontID, err := doc.AddEmbeddedFont("Kerning Test", font)
	if err != nil {
		t.Fatal(err)
	}
	style := FlowTextStyle{PageSize: [2]float64{14.5, 20}, Margins: [4]float64{5, 5, 5, 5}, FontSize: 5, LineHeight: 6, FontID: fmt.Sprint(fontID)}
	if pages, err := doc.AddFlowedText(0, "HW", style); err != nil || !slices.Equal(pages, []int{0}) {
		t.Fatalf("plain kerning pages: %v, %v", pages, err)
	}
	if pages, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "HW", KeepTogether: true}}, style); err != nil || !slices.Equal(pages, []int{1}) {
		t.Fatalf("styled kerning pages: %v, %v", pages, err)
	}
	for page := 0; page < 2; page++ {
		objects, err := doc.PageTextObjects(page)
		if err != nil || len(objects) != 1 || objects[0].Boundary[2] != 3.75 || len(objects[0].Codes) != 1 || objects[0].Codes[0].DeltaX == nil || *objects[0].Codes[0].DeltaX != "1.25" {
			t.Fatalf("page %d kerning objects: %v, %v", page, objects, err)
		}
	}
	if pages, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "H"}, {Text: "W"}}, style); err != nil || !slices.Equal(pages, []int{2, 3}) {
		t.Fatalf("cross-span pages: %v, %v", pages, err)
	}
	path := filepath.Join(t.TempDir(), "kerning.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	if text, err := reopened.PageText(0); err != nil || !slices.Equal(text, []string{"HW"}) {
		t.Fatalf("reopened kerning text: %v, %v", text, err)
	}
}

func TestFlowedTextShapesLigaturesBeforeWrapping(t *testing.T) {
	fontPath := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-serif.ttf")
	font, err := os.ReadFile(fontPath)
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
	style := FlowTextStyle{
		PageSize: [2]float64{14.5, 22}, Margins: [4]float64{5, 5, 5, 5},
		FontSize: 5, LineHeight: 6, FontID: fmt.Sprint(fontID), Shape: true,
	}
	if pages, err := doc.AddFlowedText(0, "fifi", style); err != nil || !slices.Equal(pages, []int{0}) {
		t.Fatalf("shaped flow pages: %v, %v", pages, err)
	}
	if text, err := doc.PageText(0); err != nil || !slices.Equal(text, []string{"fi", "fi"}) {
		t.Fatalf("shaped flow text: %v, %v", text, err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 2 {
		t.Fatalf("shaped flow objects: %v, %v", objects, err)
	}
	for _, object := range objects {
		if len(object.CGTransforms) != 1 || object.CGTransforms[0].CodeCount != 2 || object.CGTransforms[0].GlyphCount != 1 {
			t.Fatalf("shaped flow mapping: %+v", object.CGTransforms)
		}
	}
	narrow := style
	narrow.PageSize = [2]float64{11, 22}
	if _, err := doc.AddFlowedText(0, "a\u0301", narrow); err == nil {
		t.Fatal("expected overwide grapheme to fail")
	}
	path := filepath.Join(t.TempDir(), "shaped-flow.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	if text, err := reopened.PageText(0); err != nil || !slices.Equal(text, []string{"fi", "fi"}) {
		t.Fatalf("reopened shaped flow: %v, %v", text, err)
	}
}

func TestFlowedRestLineIndentForPlainShapedAndStyledText(t *testing.T) {
	fontPath := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-sans.ttf")
	font, err := os.ReadFile(fontPath)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	fontID, err := doc.AddEmbeddedFont("DejaVu Sans", font)
	if err != nil {
		t.Fatal(err)
	}
	style := DefaultFlowTextStyle()
	style.PageSize = [2]float64{25, 80}
	style.Margins = [4]float64{2, 2, 2, 2}
	style.FontID = fmt.Sprint(fontID)
	style.RestLineIndent = 4
	for _, shape := range []bool{false, true} {
		style.Shape = shape
		for _, spans := range []bool{false, true} {
			var pages []int
			if spans {
				pages, err = doc.AddFlowedSpans(0, []FlowSpan{{Text: "AAAAAAAAAAAA"}}, style)
			} else {
				pages, err = doc.AddFlowedText(0, "AAAAAAAAAAAA", style)
			}
			if err != nil || len(pages) != 1 {
				t.Fatalf("flowed text: %v, %v", pages, err)
			}
			objects, err := doc.PageTextObjects(pages[0])
			if err != nil || len(objects) < 2 || len(objects[0].Boundary) != 4 || objects[0].Boundary[0] != 2 {
				t.Fatalf("first line: %v, %v", objects, err)
			}
			for _, object := range objects[1:] {
				if len(object.Boundary) != 4 || object.Boundary[0] != 6 {
					t.Fatalf("rest line: %v", object)
				}
			}
		}
	}
	style.RestLineIndent = 21
	if _, err := doc.AddFlowedText(0, "A", style); err == nil {
		t.Fatal("expected invalid rest-line indent")
	}
}

func TestFlowedTextLanguageControlsWrappingAndSpanOverride(t *testing.T) {
	fontPath := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-sans.ttf")
	font, err := os.ReadFile(fontPath)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	fontID, err := doc.AddEmbeddedFont("DejaVu Sans", font)
	if err != nil {
		t.Fatal(err)
	}
	style := DefaultFlowTextStyle()
	style.PageSize = [2]float64{8.14, 20}
	style.Margins = [4]float64{1, 1, 1, 1}
	style.FontID = fmt.Sprint(fontID)
	style.Shape = true
	style.Language = "ru"
	pages, err := doc.AddFlowedText(0, "бб", style)
	if err != nil || !slices.Equal(pages, []int{0}) {
		t.Fatalf("Russian flow: %v, %v", pages, err)
	}
	if text, err := doc.PageText(0); err != nil || !slices.Equal(text, []string{"б", "б"}) {
		t.Fatalf("Russian wrap: %v, %v", text, err)
	}
	style.Language = "sr"
	pages, err = doc.AddFlowedText(0, "бб", style)
	if err != nil || !slices.Equal(pages, []int{1}) {
		t.Fatalf("Serbian flow: %v, %v", pages, err)
	}
	if text, err := doc.PageText(1); err != nil || !slices.Equal(text, []string{"бб"}) {
		t.Fatalf("Serbian wrap: %v, %v", text, err)
	}
	style.PageSize = [2]float64{40, 20}
	style.Language = "ru"
	pages, err = doc.AddFlowedSpans(0, []FlowSpan{{Text: "б"}, {Text: "б", Language: "sr"}}, style)
	if err != nil || !slices.Equal(pages, []int{2}) {
		t.Fatalf("styled flow: %v, %v", pages, err)
	}
	objects, err := doc.PageTextObjects(2)
	if err != nil || len(objects) != 2 || reflect.DeepEqual(objects[0].CGTransforms[0].Glyphs, objects[1].CGTransforms[0].Glyphs) {
		t.Fatalf("styled language override: %v, %v", objects, err)
	}
	style.Shape = false
	if _, err := doc.AddFlowedText(0, "б", style); err == nil {
		t.Fatal("expected language without shaping to fail")
	}
}

func TestFlowedSpansShapeLigaturesAcrossPages(t *testing.T) {
	fontPath := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-serif.ttf")
	font, err := os.ReadFile(fontPath)
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
	style := FlowTextStyle{
		PageSize: [2]float64{14.5, 16}, Margins: [4]float64{5, 5, 5, 5},
		FontSize: 5, LineHeight: 6, FontID: fmt.Sprint(fontID), Shape: true,
	}
	red := [3]uint8{255, 0, 0}
	pages, err := doc.AddFlowedSpans(0, []FlowSpan{{Text: "fi", FillColor: &red, Underline: true}, {Text: "fi", Italic: true}}, style)
	if err != nil || !slices.Equal(pages, []int{0, 1}) {
		t.Fatalf("shaped spans pages: %v, %v", pages, err)
	}
	for _, page := range pages {
		objects, err := doc.PageTextObjects(page)
		if err != nil || len(objects) != 1 || len(objects[0].CGTransforms) != 1 || objects[0].CGTransforms[0].GlyphCount != 1 {
			t.Fatalf("page %d objects: %v, %v", page, objects, err)
		}
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 1 {
		t.Fatalf("underline paths: %v, %v", paths, err)
	}
}

func TestAppendsPageToExistingDocument(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "helloworld.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AppendTextPage(-1, "invalid", 210, 297); err == nil {
		t.Fatal("expected invalid document index")
	}
	if err := doc.AppendTextPage(0, "追加 & 页面", 180, 250); err != nil {
		t.Fatal(err)
	}
	text, err := doc.PageText(1)
	if err != nil || len(text) != 1 || text[0] != "追加 & 页面" {
		t.Fatalf("appended text: %v, %v", text, err)
	}
	doc.Close()
	if err := doc.AppendTextPage(0, "invalid", 210, 297); err == nil {
		t.Fatal("expected closed document")
	}
}

func TestAppendsDocumentBodies(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("first", 40, 30); err != nil {
		t.Fatal(err)
	}
	if index, err := doc.AppendDocumentBody(); err != nil || index != 1 {
		t.Fatalf("first appended body: %d, %v", index, err)
	}
	if err := doc.AppendTextPage(1, "second", 50, 40); err != nil {
		t.Fatal(err)
	}
	if index, err := doc.AppendDocumentBody(); err != nil || index != 2 {
		t.Fatalf("second appended body: %d, %v", index, err)
	}
	if err := doc.AppendBlankPage(2, 30, 30); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	if len(pages) != 3 || pages[0].DocumentIndex != 0 || pages[1].DocumentIndex != 1 || pages[2].DocumentIndex != 2 {
		t.Fatalf("pages in document bodies: %+v", pages)
	}
	if err := doc.MoveDocumentBody(0, 2); err != nil {
		t.Fatal(err)
	}
	if text, err := doc.PageText(0); err != nil || !slices.Equal(text, []string{"second"}) {
		t.Fatalf("reordered first page: %v, %v", text, err)
	}
	if text, err := doc.PageText(2); err != nil || !slices.Equal(text, []string{"first"}) {
		t.Fatalf("reordered last page: %v, %v", text, err)
	}
	if err := doc.RemoveDocumentBody(1); err != nil {
		t.Fatal(err)
	}
	if text, err := doc.PageText(1); err != nil || !slices.Equal(text, []string{"first"}) {
		t.Fatalf("page after body removal: %v, %v", text, err)
	}
	if err := doc.MoveDocumentBody(-1, 0); err == nil {
		t.Fatal("expected negative index error")
	}
	if err := doc.MoveDocumentBody(0, 2); err == nil {
		t.Fatal("expected out-of-range index error")
	}
	if err := doc.RemoveDocumentBody(-1); err == nil {
		t.Fatal("expected negative remove index error")
	}
	if err := doc.RemoveDocumentBody(2); err == nil {
		t.Fatal("expected out-of-range remove index error")
	}
	if issues, err := doc.ValidateReferences(); err != nil || len(issues) != 0 {
		t.Fatalf("document references: %+v, %v", issues, err)
	}
	path := filepath.Join(t.TempDir(), "multiple-bodies.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	if text, err := reopened.PageText(0); err != nil || !slices.Equal(text, []string{"second"}) {
		t.Fatalf("second body text: %v, %v", text, err)
	}
	if infos, err := reopened.Documents(); err != nil || len(infos) != 2 {
		t.Fatalf("document bodies: %+v, %v", infos, err)
	}
}

func TestExtractsOneDocumentBody(t *testing.T) {
	source, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer source.Close()
	if err := source.AddTextPage("first", 40, 30); err != nil {
		t.Fatal(err)
	}
	if _, err := source.AppendDocumentBody(); err != nil {
		t.Fatal(err)
	}
	if err := source.AppendTextPage(1, "selected", 40, 30); err != nil {
		t.Fatal(err)
	}
	before, err := source.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := source.ExtractDocumentBody(-1); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := source.ExtractDocumentBody(2); err == nil {
		t.Fatal("expected out-of-range index error")
	}
	extracted, err := source.ExtractDocumentBody(1)
	if err != nil {
		t.Fatal(err)
	}
	defer extracted.Close()
	if infos, err := extracted.Documents(); err != nil || len(infos) != 1 {
		t.Fatalf("extracted document bodies: %v, %v", infos, err)
	}
	if text, err := extracted.PageText(0); err != nil || !slices.Equal(text, []string{"selected"}) {
		t.Fatalf("extracted page: %v, %v", text, err)
	}
	data, err := extracted.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	reopened, err := FromBytes(data)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	if text, err := reopened.PageText(0); err != nil || !slices.Equal(text, []string{"selected"}) {
		t.Fatalf("reopened extracted page: %v, %v", text, err)
	}
	after, err := source.ToBytes()
	if err != nil || !slices.Equal(before, after) {
		t.Fatalf("source changed: %v", err)
	}
}

func TestImportsOneSelectedDocumentBody(t *testing.T) {
	source, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer source.Close()
	if err := source.AddTextPage("first", 40, 30); err != nil {
		t.Fatal(err)
	}
	if _, err := source.AppendDocumentBody(); err != nil {
		t.Fatal(err)
	}
	if err := source.AppendTextPage(1, "selected", 40, 30); err != nil {
		t.Fatal(err)
	}
	target, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer target.Close()
	if err := target.AddTextPage("target", 40, 30); err != nil {
		t.Fatal(err)
	}
	before, err := target.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := target.AppendDocumentBodyFrom(source, -1); err == nil {
		t.Fatal("expected negative source index error")
	}
	if _, err := target.AppendDocumentBodyFrom(source, 2); err == nil {
		t.Fatal("expected out-of-range source index error")
	}
	after, err := target.ToBytes()
	if err != nil || !slices.Equal(before, after) {
		t.Fatalf("invalid import mutated target: %v", err)
	}
	if index, err := target.AppendDocumentBodyFrom(source, 1); err != nil || index != 1 {
		t.Fatalf("selected import: %d, %v", index, err)
	}
	if text, err := target.PageText(1); err != nil || !slices.Equal(text, []string{"selected"}) {
		t.Fatalf("selected page text: %v, %v", text, err)
	}
	if infos, err := target.Documents(); err != nil || len(infos) != 2 {
		t.Fatalf("selected body count: %v, %v", infos, err)
	}
	if issues, err := target.ValidateReferences(); err != nil || len(issues) != 0 {
		t.Fatalf("selected references: %v, %v", issues, err)
	}
	if index, err := target.AppendDocumentBodyFrom(target, 1); err != nil || index != 2 {
		t.Fatalf("self import: %d, %v", index, err)
	}
	if text, err := target.PageText(2); err != nil || !slices.Equal(text, []string{"selected"}) {
		t.Fatalf("self imported page: %v, %v", text, err)
	}
}

func TestImportsDocumentBodiesWithResources(t *testing.T) {
	var missing *Document
	if _, err := missing.AppendDocumentBodiesFrom(missing); err == nil {
		t.Fatal("expected nil document error")
	}
	source, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer source.Close()
	if err := source.AddTextPage("source", 40, 30); err != nil {
		t.Fatal(err)
	}
	if err := source.AddAttachment("note.txt", []byte("payload")); err != nil {
		t.Fatal(err)
	}
	target, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer target.Close()
	if err := target.AddTextPage("target", 40, 30); err != nil {
		t.Fatal(err)
	}
	indexes, err := target.AppendDocumentBodiesFrom(source)
	if err != nil || !slices.Equal(indexes, []int{1}) {
		t.Fatalf("import indexes: %v, %v", indexes, err)
	}
	texts, err := target.PageText(1)
	if err != nil || !slices.Equal(texts, []string{"source"}) {
		t.Fatalf("imported text: %v, %v", texts, err)
	}
	attachments, err := target.Attachments()
	if err != nil || len(attachments) != 1 {
		t.Fatalf("imported attachments: %v, %v", attachments, err)
	}
	if issues, err := target.ValidateReferences(); err != nil || len(issues) != 0 {
		t.Fatalf("imported references: %v, %v", issues, err)
	}
	path := filepath.Join(t.TempDir(), "imported-bodies.ofd")
	if err := target.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	texts, err = reopened.PageText(1)
	if err != nil || !slices.Equal(texts, []string{"source"}) {
		t.Fatalf("saved imported text: %v, %v", texts, err)
	}
	if indexes, err := target.AppendDocumentBodiesFrom(target); err != nil || !slices.Equal(indexes, []int{2, 3}) {
		t.Fatalf("self import: %v, %v", indexes, err)
	}
	closed, err := New()
	if err != nil {
		t.Fatal(err)
	}
	closed.Close()
	if _, err := target.AppendDocumentBodiesFrom(closed); err == nil {
		t.Fatal("expected closed source error")
	}
	if _, err := closed.AppendDocumentBodiesFrom(source); err == nil {
		t.Fatal("expected closed target error")
	}
}

func TestAppendsPathPageToExistingDocument(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "helloworld.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	boundary := [4]float64{5, 5, 20, 20}
	for _, test := range []struct {
		name     string
		index    int
		commands string
	}{
		{name: "negative document index", index: -1, commands: "M 0 0 L 10 10"},
		{name: "NUL byte", index: 0, commands: "bad\x00path"},
		{name: "unknown document index", index: 1, commands: "M 0 0 L 10 10"},
		{name: "empty commands", index: 0, commands: ""},
		{name: "missing move", index: 0, commands: "L 10 10"},
	} {
		t.Run(test.name, func(t *testing.T) {
			err := doc.AppendPathPage(
				test.index,
				test.commands,
				boundary,
				80,
				60,
				false,
				true,
			)
			if err == nil {
				t.Fatal("accepted invalid path page input")
			}
		})
	}
	if err := doc.AppendPathPage(
		0,
		"M 0 0 L 10 10",
		boundary,
		80,
		60,
		false,
		true,
	); err != nil {
		t.Fatal(err)
	}
	paths, err := doc.PagePaths(1)
	if err != nil || len(paths) != 1 || paths[0].AbbreviatedData == nil || *paths[0].AbbreviatedData != "M 0 0 L 10 10" {
		t.Fatalf("appended path: %+v, %v", paths, err)
	}
	output := filepath.Join(t.TempDir(), "path-extended.ofd")
	if err := doc.Save(output); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(output)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	paths, err = reopened.PagePaths(1)
	if err != nil || len(paths) != 1 {
		t.Fatalf("round-tripped path: %+v, %v", paths, err)
	}
	if err := reopened.AppendPathPage(
		0,
		"M 0 0 L 10 10",
		boundary,
		80,
		60,
		true,
		false,
	); err != nil {
		t.Fatal(err)
	}
	if paths, err := reopened.PagePaths(2); err != nil || len(paths) != 1 {
		t.Fatalf("filled path page: %+v, %v", paths, err)
	}
	reopened.Close()
	if err := reopened.AppendPathPage(
		0,
		"M 0 0 L 10 10",
		boundary,
		80,
		60,
		true,
		false,
	); err == nil {
		t.Fatal("accepted a closed document")
	}
}

func TestAppendsImagePageToExistingDocument(t *testing.T) {
	invoice := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "发票示例.ofd")
	source, err := Open(invoice)
	if err != nil {
		t.Fatal(err)
	}
	png, err := source.PageImageData(0, 0)
	source.Close()
	if err != nil {
		t.Fatal(err)
	}
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "helloworld.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	for _, test := range []struct {
		name   string
		index  int
		data   []byte
		format string
	}{
		{name: "negative document index", index: -1, data: png, format: "png"},
		{name: "unknown document index", index: 1, data: png, format: "png"},
		{name: "invalid image", index: 0, data: []byte("bad"), format: "png"},
		{name: "empty image", index: 0, data: nil, format: "png"},
		{name: "NUL format", index: 0, data: png, format: "bad\x00format"},
	} {
		t.Run(test.name, func(t *testing.T) {
			if err := doc.AppendImagePage(
				test.index,
				test.data,
				test.format,
				50,
				40,
			); err == nil {
				t.Fatal("accepted invalid image page input")
			}
		})
	}
	if err := doc.AppendImagePage(
		0,
		png,
		"png",
		50,
		40,
	); err != nil {
		t.Fatal(err)
	}
	images, err := doc.PageImages(1)
	if err != nil || len(images) != 1 || images[0].Format == nil || *images[0].Format != "PNG" {
		t.Fatalf("appended image: %+v, %v", images, err)
	}
	written, err := doc.PageImageData(1, 0)
	if err != nil || !bytes.Equal(written, png) {
		t.Fatalf("appended image bytes: %d, %v", len(written), err)
	}
	output := filepath.Join(t.TempDir(), "image-extended.ofd")
	if err := doc.Save(output); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(output)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	written, err = reopened.PageImageData(1, 0)
	if err != nil || !bytes.Equal(written, png) {
		t.Fatalf("round-tripped image: %d, %v", len(written), err)
	}
	reopened.Close()
	if err := reopened.AppendImagePage(0, png, "png", 50, 40); err == nil {
		t.Fatal("accepted a closed document")
	}
}

func TestDocumentVersions(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "versioned.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	versions, err := doc.Versions()
	if err != nil || len(versions) != 1 {
		t.Fatalf("versions: %v, %v", versions, err)
	}
	if versions[0].ID != "v1" || !versions[0].Current || versions[0].DescriptionPath != "Versions/V1.xml" || len(versions[0].Files) != 3 {
		t.Fatalf("version metadata: %+v", versions[0])
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("version references: %v, %v", issues, err)
	}
	root, err := doc.Entry("OFD.xml")
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetEntry("OFD.xml", bytes.Replace(root, []byte(`Index="1"`), []byte(`Index="bad"`), 1)); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.Versions(); err == nil {
		t.Fatal("accepted malformed version index")
	}
	doc.Close()
	if _, err := doc.Versions(); err == nil {
		t.Fatal("accepted a closed document")
	}
}

func TestForeignNamespacePathsDoNotBreakImportOrSnapshot(t *testing.T) {
	source, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer source.Close()
	target, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer target.Close()
	if err := source.SetEntry("Doc_0/Payload.bin", []byte("payload")); err != nil {
		t.Fatal(err)
	}
	foreign := []byte(`<x:Refs xmlns:x="urn:other"><x:FileLoc>/Doc_0/Payload.bin</x:FileLoc></x:Refs>`)
	if err := source.SetEntry("Doc_0/Foreign.xml", foreign); err != nil {
		t.Fatal(err)
	}
	sourceBefore, err := source.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	targetBefore, err := target.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := target.AppendDocumentBodyFrom(source, 0); err == nil {
		t.Fatal("imported an unclassified foreign reference")
	}
	if _, err := source.AddVersionSnapshot(0, "foreign", ""); err == nil {
		t.Fatal("snapshotted an unclassified foreign reference")
	}
	sourceAfter, err := source.ToBytes()
	if err != nil || !bytes.Equal(sourceBefore, sourceAfter) {
		t.Fatalf("failed snapshot mutated source: %v", err)
	}
	targetAfter, err := target.ToBytes()
	if err != nil || !bytes.Equal(targetBefore, targetAfter) {
		t.Fatalf("failed import mutated target: %v", err)
	}

	literal := []byte(`<x:Refs xmlns:x="urn:other"><x:FileLoc>/outside/value.xml</x:FileLoc></x:Refs>`)
	if err := source.SetEntry("Doc_0/Foreign.xml", literal); err != nil {
		t.Fatal(err)
	}
	if index, err := target.AppendDocumentBodyFrom(source, 0); err != nil || index != 1 {
		t.Fatalf("literal import: %d, %v", index, err)
	}
	imported, err := target.Entry("Imported_0/Doc_0/Foreign.xml")
	if err != nil || !bytes.Equal(imported, literal) {
		t.Fatalf("literal XML changed during import: %v", err)
	}
	if _, err := source.AddVersionSnapshot(0, "foreign", ""); err != nil {
		t.Fatal(err)
	}
	snapshotted, err := source.Entry("Versions/foreign/Snapshot/Doc_0/Foreign.xml")
	if err != nil || !bytes.Equal(snapshotted, literal) {
		t.Fatalf("literal XML changed during snapshot: %v", err)
	}
}

func TestAddVersionSnapshot(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("original", 80, 50); err != nil {
		t.Fatal(err)
	}
	first, err := doc.AddVersionSnapshot(0, "v1", "First")
	if err != nil || first.ID != "v1" || first.Index != 1 || !first.Current {
		t.Fatalf("first version: %+v, %v", first, err)
	}
	firstRoot, err := doc.Entry(first.DocRoot)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.AddTextPage("later", 80, 50); err != nil {
		t.Fatal(err)
	}
	second, err := doc.AddVersionSnapshot(0, "v2", "")
	if err != nil || second.Index != 2 || !second.Current {
		t.Fatalf("second version: %+v, %v", second, err)
	}
	versions, err := doc.Versions()
	if err != nil || len(versions) != 2 || versions[0].Current {
		t.Fatalf("versions after second snapshot: %v, %v", versions, err)
	}
	unchanged, err := doc.Entry(first.DocRoot)
	if err != nil || !bytes.Equal(unchanged, firstRoot) {
		t.Fatalf("first snapshot changed: %v", err)
	}
	if _, err := doc.AddVersionSnapshot(0, "v2", ""); err == nil {
		t.Fatal("accepted duplicate version ID")
	}
	if _, err := doc.AddVersionSnapshot(-1, "v3", ""); err == nil {
		t.Fatal("accepted negative document index")
	}
	target := filepath.Join(t.TempDir(), "versioned.ofd")
	if err := doc.Save(target); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(target)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	versions, err = reopened.Versions()
	if err != nil || len(versions) != 2 {
		t.Fatalf("reopened versions: %v, %v", versions, err)
	}
	historical, err := reopened.ExtractVersionSnapshot(0)
	if err != nil {
		t.Fatal(err)
	}
	defer historical.Close()
	if text, err := historical.PageText(0); err != nil || !slices.Equal(text, []string{"original"}) {
		t.Fatalf("historical text: %v, %v", text, err)
	}
	if versions, err := historical.Versions(); err != nil || len(versions) != 0 {
		t.Fatalf("standalone historical versions: %v, %v", versions, err)
	}
	if _, err := reopened.ExtractVersionSnapshot(-1); err == nil {
		t.Fatal("accepted negative version index")
	}
	if _, err := reopened.ExtractVersionSnapshot(2); err == nil {
		t.Fatal("accepted out-of-range version index")
	}
	doc.Close()
	if _, err := doc.AddVersionSnapshot(0, "v3", ""); err == nil {
		t.Fatal("accepted a closed document")
	}
}

func TestExtractVersionWithoutListedOFDRoot(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("historical", 40, 30); err != nil {
		t.Fatal(err)
	}
	version, err := doc.AddVersionSnapshot(0, "no-root", "")
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.AddTextPage("current", 40, 30); err != nil {
		t.Fatal(err)
	}
	description, err := doc.Entry(version.DescriptionPath)
	if err != nil {
		t.Fatal(err)
	}
	pattern := regexp.MustCompile(`<ofd:File ID="f[0-9]+">Snapshot/OFD.xml</ofd:File>`)
	withoutRoot := pattern.ReplaceAll(description, nil)
	if bytes.Equal(withoutRoot, description) {
		t.Fatal("snapshot OFD.xml file was not found")
	}
	if err := doc.SetEntry(version.DescriptionPath, withoutRoot); err != nil {
		t.Fatal(err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	historical, err := doc.ExtractVersionSnapshot(0)
	if err != nil {
		t.Fatal(err)
	}
	defer historical.Close()
	text, err := historical.PageText(0)
	if err != nil || !slices.Equal(text, []string{"historical"}) {
		t.Fatalf("historical text: %v, %v", text, err)
	}
	issues, err := historical.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("historical references: %v, %v", issues, err)
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(after, before) {
		t.Fatalf("source package changed: %v", err)
	}
}

func TestDocumentMetadata(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "发票示例.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	infos, err := doc.Documents()
	if err != nil || len(infos) != 1 || infos[0].Author == nil || *infos[0].Author != "China Tax" || len(infos[0].CustomData) < 5 {
		t.Fatalf("metadata: %v, %v", infos, err)
	}
	if err := doc.SetDocumentInfoField(-1, "Title", "bad"); err == nil {
		t.Fatal("expected negative index")
	}
	if err := doc.SetDocumentInfoField(0, "DocRoot", "bad"); err == nil {
		t.Fatal("expected unsupported field")
	}
	if err := doc.SetDocumentInfoField(1, "Title", "bad"); err == nil {
		t.Fatal("expected missing document body")
	}
	if err := doc.SetDocumentInfoField(0, "Title\x00", "bad"); err == nil {
		t.Fatal("expected NUL field error")
	}
	if err := doc.SetDocumentInfoField(0, "Title", "bad\x00"); err == nil {
		t.Fatal("expected NUL value error")
	}
	if err := doc.SetDocumentInfoField(0, "Title", "Rust & OFD"); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetDocumentInfoField(0, "Cover", "missing.png"); err == nil {
		t.Fatal("expected missing cover entry")
	}
	cover, err := os.ReadFile(filepath.Join("..", "..", "..", "..", "ofdrw-layout", "src", "test", "resources", "testimg.png"))
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetEntry("Doc_0/Res/cover.png", cover); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetDocumentInfoField(0, "Cover", "/Doc_0/Res/cover.png"); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetDocumentInfoField(0, "Cover", "/DOC_0/RES/COVER.PNG"); err != nil {
		t.Fatal(err)
	}
	if issues, err := doc.ValidateReferences(); err != nil {
		t.Fatal(err)
	} else {
		for _, issue := range issues {
			if issue.Code == "cover_reference" {
				t.Fatalf("unexpected cover reference issue: %v", issue)
			}
		}
	}
	if err := doc.SetDocumentInfoField(0, "Cover", "/Doc_0/Res/cover.png"); err != nil {
		t.Fatal(err)
	}
	infos, err = doc.Documents()
	if err != nil || infos[0].Title == nil || *infos[0].Title != "Rust & OFD" || infos[0].Cover == nil || *infos[0].Cover != "/Doc_0/Res/cover.png" {
		t.Fatalf("edited metadata: %v, %v", infos, err)
	}
	if err := doc.SetEntry("OFD.xml", []byte("<OFD><DocBody><DocRoot>Doc_0/Document.xml</DocRoot></DocBody></OFD>")); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.Documents(); err == nil {
		t.Fatal("expected missing DocInfo error")
	}
	doc.Close()
	if _, err := doc.Documents(); err == nil {
		t.Fatal("expected closed document")
	}
	if err := doc.SetDocumentInfoField(0, "Title", "bad"); err == nil {
		t.Fatal("expected closed document")
	}
}

func TestDocumentPermissionsRoundTrip(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("permissions", 80, 40); err != nil {
		t.Fatal(err)
	}
	permissions, err := doc.DocumentPermissions(0)
	if err != nil || permissions != DefaultDocumentPermissions() {
		t.Fatalf("default permissions: %+v, %v", permissions, err)
	}
	if _, err := doc.DocumentPermissions(-1); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.DocumentPermissions(1); err == nil {
		t.Fatal("expected missing document error")
	}
	start, end := "2024-02-29T12:00:00", "2025-01-01T00:00:00"
	permissions.Edit = false
	permissions.Print.Copies = 2
	permissions.ValidPeriod = &ValidPeriod{StartDate: &start, EndDate: &end}
	if err := doc.SetDocumentPermissions(0, permissions); err != nil {
		t.Fatal(err)
	}
	got, err := doc.DocumentPermissions(0)
	if err != nil || got.Edit || got.Print.Copies != 2 || got.ValidPeriod == nil || *got.ValidPeriod.StartDate != start {
		t.Fatalf("edited permissions: %+v, %v", got, err)
	}
	bad := permissions
	badStart := "2024-02-30T00:00:00"
	bad.ValidPeriod = &ValidPeriod{StartDate: &badStart}
	if err := doc.SetDocumentPermissions(0, bad); err == nil {
		t.Fatal("expected invalid date error")
	}
	path := filepath.Join(t.TempDir(), "permissions.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	got, err = reopened.DocumentPermissions(0)
	if err != nil || got.Edit || got.Print.Copies != 2 || got.ValidPeriod == nil || *got.ValidPeriod.EndDate != end {
		t.Fatalf("reopened permissions: %+v, %v", got, err)
	}
}

func TestViewPreferencesRoundTrip(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	preferences, err := doc.ViewPreferences(0)
	if err != nil || preferences != DefaultViewPreferences() {
		t.Fatalf("default view preferences: %+v, %v", preferences, err)
	}
	if _, err := doc.ViewPreferences(-1); err == nil {
		t.Fatal("expected negative index error")
	}
	mode := "FitWidth"
	preferences.PageMode = "UseOutlines"
	preferences.PageLayout = "TwoColumnR"
	preferences.HideToolbar = true
	preferences.ZoomMode = &mode
	if err := doc.SetViewPreferences(0, preferences); err != nil {
		t.Fatal(err)
	}
	got, err := doc.ViewPreferences(0)
	if err != nil || got.PageMode != "UseOutlines" || got.ZoomMode == nil || *got.ZoomMode != mode {
		t.Fatalf("edited view preferences: %+v, %v", got, err)
	}
	bad := preferences
	zoom := 1.25
	bad.Zoom = &zoom
	if err := doc.SetViewPreferences(0, bad); err == nil {
		t.Fatal("expected conflicting zoom error")
	}
	path := filepath.Join(t.TempDir(), "view-preferences.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	got, err = reopened.ViewPreferences(0)
	if err != nil || got.PageMode != "UseOutlines" || got.ZoomMode == nil || *got.ZoomMode != mode {
		t.Fatalf("reopened view preferences: %+v, %v", got, err)
	}
}

func TestDocumentActionsRoundTrip(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("target", 80, 40); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	destination := Destination{PageID: pages[0].ID, Kind: "Fit"}
	if err := doc.AddBookmark(0, "chapter", destination); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddAttachment("note.txt", []byte("note")); err != nil {
		t.Fatal(err)
	}
	attachments, err := doc.Attachments()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.AddDocumentAction(0, DocumentActionSpec{Kind: "page", Event: "DO", Destination: &Destination{PageID: "missing", Kind: "Fit"}}); err == nil {
		t.Fatal("expected invalid destination")
	}
	if err := doc.AddDocumentAction(0, DocumentActionSpec{Kind: "uri", Event: "DO", URI: "https://example.org/?x=1&y=2"}); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddDocumentAction(0, DocumentActionSpec{Kind: "page", Event: "DO", Destination: &destination}); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddDocumentAction(0, DocumentActionSpec{Kind: "bookmark", Event: "CLICK", Name: "chapter"}); err != nil {
		t.Fatal(err)
	}
	newWindow := true
	if err := doc.AddDocumentAction(0, DocumentActionSpec{Kind: "attachment", Event: "DO", ID: attachments[0].ID, NewWindow: &newWindow}); err != nil {
		t.Fatal(err)
	}
	actions, err := doc.DocumentActions(0)
	if err != nil || len(actions) != 4 || actions[0].Kind != "URI" || actions[1].Kind != "Goto" || actions[2].Kind != "Goto" || actions[3].Kind != "GotoA" {
		t.Fatalf("document actions: %+v, %v", actions, err)
	}
	if err := doc.RemoveDocumentAction(0, 4); err == nil {
		t.Fatal("expected invalid index")
	}
	if err := doc.RemoveDocumentAction(0, 1); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "document-actions.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	actions, err = reopened.DocumentActions(0)
	if err != nil || len(actions) != 3 || actions[0].Kind != "URI" || actions[1].Bookmark == nil || *actions[1].Bookmark != "chapter" {
		t.Fatalf("reopened document actions: %+v, %v", actions, err)
	}
}

func TestPageActionsRoundTrip(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("page action", 80, 40); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.AddPageAction(0, DocumentActionSpec{Kind: "uri", Event: "CLICK", URI: "https://example.org"}); err == nil {
		t.Fatal("expected invalid page action event")
	}
	if err := doc.AddPageAction(0, DocumentActionSpec{Kind: "page", Event: "PO", Destination: &Destination{PageID: "missing", Kind: "Fit"}}); err == nil {
		t.Fatal("expected invalid destination")
	}
	if err := doc.AddPageAction(0, DocumentActionSpec{Kind: "uri", Event: "PO", URI: "https://example.org"}); err != nil {
		t.Fatal(err)
	}
	destination := Destination{PageID: pages[0].ID, Kind: "Fit"}
	if err := doc.AddPageAction(0, DocumentActionSpec{Kind: "page", Event: "PO", Destination: &destination}); err != nil {
		t.Fatal(err)
	}
	actions, err := doc.PageActions(0)
	if err != nil || len(actions) != 2 || actions[0].Kind != "URI" || actions[1].Kind != "Goto" {
		t.Fatalf("page actions: %+v, %v", actions, err)
	}
	if err := doc.RemovePageAction(0, 0); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "page-actions.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	actions, err = reopened.PageActions(0)
	if err != nil || len(actions) != 1 || actions[0].Event != "PO" || actions[0].Destination == nil || actions[0].Destination.PageID != pages[0].ID {
		t.Fatalf("reopened page actions: %+v, %v", actions, err)
	}
}

func TestIntegrityManifestReportsMissingAndExtraFiles(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	manifest, err := doc.IntegrityManifest()
	if err != nil || manifest != nil {
		t.Fatalf("unexpected manifest: %+v, %v", manifest, err)
	}
	if report, err := doc.CheckIntegrityManifest(); err != nil || report != nil {
		t.Fatalf("unexpected manifest report: %+v, %v", report, err)
	}
	entries, err := doc.Entries()
	if err != nil {
		t.Fatal(err)
	}
	var files strings.Builder
	for index, name := range entries {
		fmt.Fprintf(&files, `<File ID="%d" FileLoc="/%s"/>`, index+1, name)
	}
	xml := fmt.Sprintf(`<DocEntries ID="manifest"><SignedValueLoc>/signedvalue.dat</SignedValueLoc><FileList>%s</FileList></DocEntries>`, files.String())
	if err := doc.SetEntry("OFDEntries.xml", []byte(xml)); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetEntry("OFDEntries.xml", []byte(`<DocEntries><SignedValueLoc>/signedvalue.dat</SignedValueLoc><FileList/></DocEntries>`)); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.IntegrityManifest(); err == nil {
		t.Fatal("expected invalid manifest error")
	}
	if _, err := doc.CheckIntegrityManifest(); err == nil {
		t.Fatal("expected invalid manifest check error")
	}
	if err := doc.SetEntry("OFDEntries.xml", []byte(xml)); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetEntry("signedvalue.dat", []byte("opaque signature")); err != nil {
		t.Fatal(err)
	}
	manifest, err = doc.IntegrityManifest()
	if err != nil || manifest == nil || manifest.ID != "manifest" || manifest.SignedValuePath != "signedvalue.dat" {
		t.Fatalf("integrity manifest: %+v, %v", manifest, err)
	}
	report, err := doc.CheckIntegrityManifest()
	if err != nil || report == nil || !report.Complete {
		t.Fatalf("complete manifest: %+v, %v", report, err)
	}
	if err := doc.SetEntry("unexpected.bin", []byte("extra")); err != nil {
		t.Fatal(err)
	}
	report, err = doc.CheckIntegrityManifest()
	if err != nil || report == nil || report.Complete || !slices.Equal(report.ExtraFiles, []string{"unexpected.bin"}) {
		t.Fatalf("extra file report: %+v, %v", report, err)
	}
	doc.Close()
	if _, err := doc.IntegrityManifest(); err == nil {
		t.Fatal("expected closed-document error")
	}
	if _, err := doc.CheckIntegrityManifest(); err == nil {
		t.Fatal("expected closed-document error")
	}
}

func TestJavaSM2IntegritySignatureAndManifest(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-crypto", "src", "test", "resources", "hello-integrity.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	report, err := doc.VerifyIntegrityManifestSM2()
	if err != nil || report == nil || !report.ManifestComplete || !report.SignatureValid || !report.AllValid {
		t.Fatalf("valid signature: %+v, %v", report, err)
	}
	report, err = doc.VerifyIntegrityManifestSM2WithDistID("wrong-id")
	if err != nil || report == nil || report.SignatureValid || report.AllValid {
		t.Fatalf("wrong distinguishing ID: %+v, %v", report, err)
	}
	if err := doc.SetEntry("extra.bin", []byte("extra")); err != nil {
		t.Fatal(err)
	}
	report, err = doc.VerifyIntegrityManifestSM2()
	if err != nil || report == nil || report.ManifestComplete || !report.SignatureValid || report.AllValid {
		t.Fatalf("extra package file: %+v, %v", report, err)
	}
	if _, err := doc.VerifyIntegrityManifestSM2WithDistID(""); err == nil {
		t.Fatal("expected empty distinguishing ID error")
	}
	doc.Close()
	if _, err := doc.VerifyIntegrityManifestSM2(); err == nil {
		t.Fatal("expected closed-document error")
	}
	unsigned, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer unsigned.Close()
	if report, err := unsigned.VerifyIntegrityManifestSM2(); err != nil || report != nil {
		t.Fatalf("unsigned document: %+v, %v", report, err)
	}
}

func TestProtectIntegritySM2RoundTrip(t *testing.T) {
	fixtureDir := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	key, err := os.ReadFile(filepath.Join(fixtureDir, "user-private-key.bin"))
	if err != nil {
		t.Fatal(err)
	}
	certificate, err := os.ReadFile(filepath.Join(fixtureDir, "user-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.ProtectIntegritySM2WithDistID(key, certificate, "custom-id"); err != nil {
		t.Fatal(err)
	}
	report, err := doc.VerifyIntegrityManifestSM2WithDistID("custom-id")
	if err != nil || report == nil || !report.AllValid {
		t.Fatalf("valid integrity protection: %+v, %v", report, err)
	}
	report, err = doc.VerifyIntegrityManifestSM2()
	if err != nil || report == nil || report.SignatureValid {
		t.Fatalf("default ID should differ: %+v, %v", report, err)
	}
	path := filepath.Join(t.TempDir(), "protected.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	report, err = reopened.VerifyIntegrityManifestSM2WithDistID("custom-id")
	if err != nil || report == nil || !report.AllValid {
		t.Fatalf("reopened integrity protection: %+v, %v", report, err)
	}
	if err := doc.ProtectIntegritySM2(key, certificate); err == nil {
		t.Fatal("expected duplicate protection error")
	}
}

func TestIntegrityVerificationWithOrderedSM2Chain(t *testing.T) {
	fixtureDir := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	key, err := os.ReadFile(filepath.Join(fixtureDir, "trust-leaf-private-key.bin"))
	if err != nil {
		t.Fatal(err)
	}
	signer, err := os.ReadFile(filepath.Join(fixtureDir, "trust-leaf-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	anchor, err := os.ReadFile(filepath.Join(fixtureDir, "trust-root-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.ProtectIntegritySM2(key, signer); err != nil {
		t.Fatal(err)
	}
	report, err := doc.VerifyIntegrityManifestSM2WithOrderedChain(signer, nil, anchor, 1_893_456_000, "", defaultSM2DistID)
	if err != nil || report == nil || !report.SignatureValid || !report.CertificateChainValid || !report.AllValid {
		t.Fatalf("trusted integrity: %+v, %v", report, err)
	}
	report, err = doc.VerifyIntegrityManifestSM2WithOrderedChain(signer, nil, anchor, 1_600_000_000, "", defaultSM2DistID)
	if err != nil || report == nil || !report.SignatureValid || report.CertificateChainValid || report.AllValid {
		t.Fatalf("expired integrity signer: %+v, %v", report, err)
	}
	if _, err := doc.VerifyIntegrityManifestSM2WithOrderedChain([]byte("invalid cert"), nil, anchor, 1_893_456_000, "", defaultSM2DistID); err == nil {
		t.Fatal("expected invalid signer certificate error")
	}
	unsigned, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer unsigned.Close()
	if report, err := unsigned.VerifyIntegrityManifestSM2WithOrderedChain(signer, nil, anchor, 1_893_456_000, "", defaultSM2DistID); err != nil || report != nil {
		t.Fatalf("unsigned document: %+v, %v", report, err)
	}
	doc.Close()
	if _, err := doc.VerifyIntegrityManifestSM2WithOrderedChain(signer, nil, anchor, 1_893_456_000, "", defaultSM2DistID); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestIntegrityCertificatePoolFindsValidPath(t *testing.T) {
	fixtureDir := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	read := func(name string) []byte {
		data, err := os.ReadFile(filepath.Join(fixtureDir, name))
		if err != nil {
			t.Fatal(err)
		}
		return data
	}
	key := read("chain-leaf-private-key.bin")
	signer := read("chain-leaf-cert.der")
	intermediate := read("chain-intermediate-cert.der")
	unrelated := read("root-cert.der")
	anchor := read("chain-root-cert.der")
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.ProtectIntegritySM2(key, signer); err != nil {
		t.Fatal(err)
	}
	report, err := doc.VerifyIntegrityManifestSM2WithCertificatePool(signer, [][]byte{unrelated, intermediate}, anchor, 1_893_456_000, "", defaultSM2DistID)
	if err != nil || report == nil || !report.AllValid {
		t.Fatalf("valid chain in candidate pool: %+v, %v", report, err)
	}
	report, err = doc.VerifyIntegrityManifestSM2WithCertificatePool(signer, [][]byte{unrelated}, anchor, 1_893_456_000, "", defaultSM2DistID)
	if err != nil || report == nil || !report.SignatureValid || report.CertificateChainValid || report.AllValid {
		t.Fatalf("missing intermediate: %+v, %v", report, err)
	}
}

func TestCustomTagsRoundTrip(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("tag target", 80, 40); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetEntry("Doc_0/Tags/schema.xsd", []byte("<schema/>")); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddCustomTag(0, "invoice", []byte("<broken>"), CustomTagOptions{}); err == nil {
		t.Fatal("expected invalid XML")
	}
	missingSchema := "missing.xsd"
	if _, err := doc.AddCustomTag(0, "invoice", []byte("<Invoice/>"), CustomTagOptions{SchemaPath: &missingSchema}); err == nil {
		t.Fatal("expected missing schema")
	}
	namespace, schemaPath := "urn:test", "Doc_0/Tags/schema.xsd"
	payload := []byte("<Invoice Ref=\"1\"/>")
	first, err := doc.AddCustomTag(0, "invoice", payload, CustomTagOptions{Namespace: &namespace, SchemaPath: &schemaPath})
	if err != nil || first.TypeID != "invoice" || first.Namespace == nil || *first.Namespace != namespace {
		t.Fatalf("first custom tag: %+v, %v", first, err)
	}
	second, err := doc.AddCustomTag(0, "audit", []byte("<Audit/>"), CustomTagOptions{})
	if err != nil || first.Path == second.Path {
		t.Fatalf("second custom tag: %+v, %v", second, err)
	}
	tags, err := doc.CustomTags()
	if err != nil || len(tags) != 2 || tags[0].Path != first.Path || tags[1].Path != second.Path {
		t.Fatalf("custom tags: %+v, %v", tags, err)
	}
	data, err := doc.CustomTagData(0)
	if err != nil || !bytes.Equal(data, payload) {
		t.Fatalf("custom tag data: %q, %v", data, err)
	}
	if _, err := doc.CustomTagData(-1); err == nil {
		t.Fatal("expected negative index")
	}
	path := filepath.Join(t.TempDir(), "custom-tags.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	tags, err = reopened.CustomTags()
	if err != nil || len(tags) != 2 || tags[0].TypeID != "invoice" {
		t.Fatalf("reopened custom tags: %+v, %v", tags, err)
	}
	if _, err := reopened.UpdateCustomTag(-1, "bad", CustomTagOptions{}); err == nil {
		t.Fatal("expected negative custom tag index")
	}
	if _, err := reopened.UpdateCustomTag(2, "bad", CustomTagOptions{}); err == nil {
		t.Fatal("expected out-of-bounds custom tag index")
	}
	updated, err := reopened.UpdateCustomTag(0, "updated & invoice", CustomTagOptions{})
	if err != nil || updated.Path != first.Path || updated.TypeID != "updated & invoice" {
		t.Fatalf("updated custom tag: %+v, %v", updated, err)
	}
	data, err = reopened.CustomTagData(0)
	if err != nil || !bytes.Equal(data, payload) {
		t.Fatalf("retained custom tag data: %q, %v", data, err)
	}
	if _, err := reopened.ReplaceCustomTagData(-1, []byte("<New/>")); err == nil {
		t.Fatal("expected negative custom tag index")
	}
	if _, err := reopened.ReplaceCustomTagData(0, []byte("<broken>")); err == nil {
		t.Fatal("expected invalid replacement XML")
	}
	replaced, err := reopened.ReplaceCustomTagData(0, []byte("<New/>"))
	if err != nil || replaced.Path == first.Path || replaced.TypeID != updated.TypeID {
		t.Fatalf("replaced custom tag: %+v, %v", replaced, err)
	}
	data, err = reopened.CustomTagData(0)
	if err != nil || !bytes.Equal(data, []byte("<New/>")) {
		t.Fatalf("replacement data: %q, %v", data, err)
	}
	if err := reopened.RemoveCustomTag(-1); err == nil {
		t.Fatal("expected negative custom tag index")
	}
	if err := reopened.RemoveCustomTag(2); err == nil {
		t.Fatal("expected out-of-bounds custom tag index")
	}
	if err := reopened.RemoveCustomTag(0); err != nil {
		t.Fatal(err)
	}
	tags, err = reopened.CustomTags()
	if err != nil || len(tags) != 1 || tags[0].TypeID != "audit" {
		t.Fatalf("remaining custom tag: %+v, %v", tags, err)
	}
	retained, err := reopened.Entry(first.Path)
	if err != nil || !bytes.Equal(retained, payload) {
		t.Fatalf("retained custom tag payload: %q, %v", retained, err)
	}
}

func TestExtensionsRoundTrip(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.SetEntry("Doc_0/Ext/payload.bin", []byte("external")); err != nil {
		t.Fatal(err)
	}
	company, version, date, propertyType := "Acme", "1.2", "2024-02-29", "string"
	refID := uint64(1)
	extension := Extension{
		DocumentIndex: 0, AppName: "Example & Co", Company: &company,
		AppVersion: &version, Date: &date, RefID: &refID,
		Properties:      []ExtensionProperty{{Name: "status", TypeName: &propertyType, Value: "ready & checked"}},
		ExtendDataPaths: []string{"Doc_0/Ext/payload.bin"},
		DataXML:         []string{"<ofd:Data xmlns:ofd=\"http://www.ofdspec.org/2016\"><Payload/></ofd:Data>"},
	}
	bad := extension
	badDate := "2025-02-29"
	bad.Date = &badDate
	if err := doc.AddExtension(bad); err == nil {
		t.Fatal("expected invalid date")
	}
	if err := doc.AddExtension(extension); err != nil {
		t.Fatal(err)
	}
	got, err := doc.Extensions()
	if err != nil || len(got) != 1 || got[0].AppName != extension.AppName || got[0].Properties[0].Value != "ready & checked" {
		t.Fatalf("extensions: %+v, %v", got, err)
	}
	path := filepath.Join(t.TempDir(), "extensions.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	got, err = reopened.Extensions()
	if err != nil || len(got) != 1 || got[0].Date == nil || *got[0].Date != date {
		t.Fatalf("reopened extensions: %+v, %v", got, err)
	}
	updated := extension
	updated.AppName = "Updated & Co"
	if err := reopened.UpdateExtension(-1, updated); err == nil {
		t.Fatal("expected negative extension index")
	}
	if err := reopened.UpdateExtension(1, updated); err == nil {
		t.Fatal("expected out-of-bounds extension index")
	}
	if err := reopened.UpdateExtension(0, updated); err != nil {
		t.Fatal(err)
	}
	got, err = reopened.Extensions()
	if err != nil || len(got) != 1 || got[0].AppName != updated.AppName {
		t.Fatalf("updated extension: %+v, %v", got, err)
	}
	if err := reopened.RemoveExtension(-1); err == nil {
		t.Fatal("expected negative extension index")
	}
	if err := reopened.RemoveExtension(1); err == nil {
		t.Fatal("expected out-of-bounds extension index")
	}
	if err := reopened.RemoveExtension(0); err != nil {
		t.Fatal(err)
	}
	got, err = reopened.Extensions()
	if err != nil || len(got) != 0 {
		t.Fatalf("remaining extensions: %+v, %v", got, err)
	}
	retained, err := reopened.Entry("Doc_0/Ext/payload.bin")
	if err != nil || !bytes.Equal(retained, []byte("external")) {
		t.Fatalf("retained extension data: %q, %v", retained, err)
	}
}

func TestMinimalExtensionAndInvalidDocumentIndex(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddExtension(Extension{DocumentIndex: -1, AppName: "Minimal"}); err == nil {
		t.Fatal("expected negative document index error")
	}
	if err := doc.AddExtension(Extension{DocumentIndex: 0, AppName: "Minimal"}); err != nil {
		t.Fatal(err)
	}
	extensions, err := doc.Extensions()
	if err != nil || len(extensions) != 1 || extensions[0].AppName != "Minimal" || len(extensions[0].Properties) != 0 {
		t.Fatalf("minimal extension: %+v, %v", extensions, err)
	}
}

func TestReadsInvoiceAnnotations(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "发票示例.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	items, err := doc.Annotations()
	if err != nil || len(items) != 2 || items[0].Type != "Watermark" || len(items[1].Text) != 1 || items[1].Text[0] != "下载次数：1   " {
		t.Fatalf("annotations: %v, %v", items, err)
	}
	if err := doc.SetEntry("Doc_0/Annots/Annotations.xml", []byte("<Wrong/>")); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.Annotations(); err == nil {
		t.Fatal("expected malformed annotation error")
	}
	doc.Close()
	if _, err := doc.Annotations(); err == nil {
		t.Fatal("expected closed document")
	}
}

func TestMissingAnnotationIndexKeepsPageReadable(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "keyword.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	items, err := doc.Annotations()
	if err != nil || len(items) != 0 {
		t.Fatalf("annotations: %v, %v", items, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "<svg") {
		t.Fatalf("page SVG: %v, %v", len(svg), err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, issue := range issues {
		if issue.Code == "annotation_reference" && strings.Contains(issue.Message, "Doc_0/Annotations.xml") {
			found = true
		}
	}
	if !found {
		t.Fatalf("missing annotation index not reported: %v", issues)
	}
}

func TestImageWithoutResourceIDKeepsPageReadable(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "path_unstd.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(1)
	if err != nil || !strings.Contains(svg, "立案决定书") || strings.Contains(svg, "<image ") {
		t.Fatalf("page SVG: %v, %v", len(svg), err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, issue := range issues {
		if issue.Code == "missing_media" && strings.Contains(issue.Message, "75") {
			found = true
		}
	}
	if !found {
		t.Fatalf("missing image reference not reported: %v", issues)
	}
}

func TestMissingImageFileKeepsPageReadable(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-converter", "src", "test", "resources", "testImageNotFound.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	issues, err := doc.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, issue := range issues {
		if issue.Code == "media_resource" && strings.Contains(issue.Message, "missing entry") {
			found = true
		}
	}
	if !found {
		t.Fatalf("missing image file not reported: %v", issues)
	}
	count, err := doc.PageCount()
	if err != nil {
		t.Fatal(err)
	}
	for pageIndex := range count {
		svg, err := doc.PageSVG(pageIndex)
		if err != nil || !strings.Contains(svg, "<svg") {
			t.Fatalf("page %d SVG: %v, %v", pageIndex, len(svg), err)
		}
	}
}

func TestMissingPublicResourceFileKeepsPageReadable(t *testing.T) {
	fixtures := []struct{ module, name string }{
		{"ofdrw-converter", "V4RideRight.ofd"},
		{"ofdrw-converter", "h.ofd"},
		{"ofdrw-layout", "Page5.ofd"},
		{"ofdrw-sign", "VPage7.ofd"},
	}
	for _, fixture := range fixtures {
		t.Run(fixture.name, func(t *testing.T) {
			path := filepath.Join("..", "..", "..", "..", fixture.module, "src", "test", "resources", fixture.name)
			doc, err := Open(path)
			if err != nil {
				t.Fatal(err)
			}
			defer doc.Close()
			issues, err := doc.ValidateReferences()
			if err != nil {
				t.Fatal(err)
			}
			found := false
			for _, issue := range issues {
				if issue.Code == "resource_reference" && strings.Contains(issue.Message, "Doc_0/PublicRes.xml") {
					found = true
				}
			}
			if !found {
				t.Fatalf("missing resource file not reported: %v", issues)
			}
			count, err := doc.PageCount()
			if err != nil {
				t.Fatal(err)
			}
			for pageIndex := range count {
				svg, err := doc.PageSVG(pageIndex)
				if err != nil || !strings.Contains(svg, "<svg") {
					t.Fatalf("page %d SVG: %v, %v", pageIndex, len(svg), err)
				}
			}
		})
	}
}

func TestPackageRootResourceBaseReadsAbsoluteMediaPath(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-converter", "src", "test", "resources", "不规范资源路径.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	data, err := doc.PageImageData(0, 0)
	if err != nil || !bytes.HasPrefix(data, []byte("GIF")) {
		t.Fatalf("GIF media: %d bytes, %v", len(data), err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "data:image/png;base64,") {
		t.Fatalf("page SVG: %d bytes, %v", len(svg), err)
	}
}

func TestImageMaskIsExposedAndRendered(t *testing.T) {
	path := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "image-mask.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 || images[0].ImageMask == nil || *images[0].ImageMask != "4" {
		t.Fatalf("image mask metadata: %v, %v", images, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("valid references: %v, %v", issues, err)
	}
	data, err := doc.PagePNG(0, 25.4)
	if err != nil {
		t.Fatal(err)
	}
	bitmap, err := png.Decode(bytes.NewReader(data))
	if err != nil {
		t.Fatal(err)
	}
	red, green, blue, _ := bitmap.At(1, 0).RGBA()
	if red < 0xf000 || green < 0xf000 || blue < 0xf000 {
		t.Fatalf("masked pixel is not white: %d %d %d", red, green, blue)
	}
}

func TestEditsPageImageMaskAtomically(t *testing.T) {
	path := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "image-mask.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.SetPageImageMask(0, "5", nil); err != nil {
		t.Fatal(err)
	}
	images, err := doc.PageImages(0)
	if err != nil || images[0].ImageMask != nil {
		t.Fatalf("cleared mask: %v, %v", images, err)
	}
	maskID := "4"
	if err := doc.SetPageImageMask(0, "5", &maskID); err != nil {
		t.Fatal(err)
	}
	images, err = doc.PageImages(0)
	if err != nil || images[0].ImageMask == nil || *images[0].ImageMask != "4" {
		t.Fatalf("restored mask: %v, %v", images, err)
	}
	before, err := doc.Entry("Doc_0/Pages/Page_0/Content.xml")
	if err != nil {
		t.Fatal(err)
	}
	missing := "999"
	if err := doc.SetPageImageMask(0, "5", &missing); err == nil {
		t.Fatal("expected unknown mask resource error")
	}
	after, err := doc.Entry("Doc_0/Pages/Page_0/Content.xml")
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed update changed page: %v", err)
	}
	if err := doc.SetPageImageMask(-1, "5", &maskID); err == nil {
		t.Fatal("expected negative page index error")
	}
}

func TestEditsPageImageSubstitutionAtomically(t *testing.T) {
	path := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "image-mask.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	alternative := "4"
	if err := doc.SetPageImageSubstitution(0, "5", &alternative); err != nil {
		t.Fatal(err)
	}
	images, err := doc.PageImages(0)
	if err != nil || images[0].Substitution == nil || *images[0].Substitution != "4" {
		t.Fatalf("substitution: %v, %v", images, err)
	}
	before, err := doc.Entry("Doc_0/Pages/Page_0/Content.xml")
	if err != nil {
		t.Fatal(err)
	}
	missing := "999"
	if err := doc.SetPageImageSubstitution(0, "5", &missing); err == nil {
		t.Fatal("expected unknown alternative resource error")
	}
	after, err := doc.Entry("Doc_0/Pages/Page_0/Content.xml")
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed update changed page: %v", err)
	}
	if err := doc.SetPageImageSubstitution(-1, "5", &alternative); err == nil {
		t.Fatal("expected negative page index error")
	}
	if err := doc.SetPageImageSubstitution(0, "5", nil); err != nil {
		t.Fatal(err)
	}
	images, err = doc.PageImages(0)
	if err != nil || images[0].Substitution != nil {
		t.Fatalf("cleared substitution: %v, %v", images, err)
	}
}

func TestExportsWithImageSubstitution(t *testing.T) {
	path := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "image-mask.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	alternative := "4"
	if err := doc.SetPageImageSubstitution(0, "5", &alternative); err != nil {
		t.Fatal(err)
	}
	options := RenderOptions{PreferImageSubstitution: true}
	primarySVG, err := doc.PageSVG(0)
	if err != nil {
		t.Fatal(err)
	}
	selectedSVG, err := doc.PageSVGWithOptions(0, options)
	if err != nil || primarySVG == selectedSVG {
		t.Fatalf("selected SVG: %v", err)
	}
	primaryPNG, err := doc.PagePNG(0, 254)
	if err != nil {
		t.Fatal(err)
	}
	selectedPNG, err := doc.PagePNGWithOptions(0, 254, options)
	if err != nil || bytes.Equal(primaryPNG, selectedPNG) {
		t.Fatalf("selected PNG: %v", err)
	}
	primaryPDF, err := doc.ToPDF()
	if err != nil {
		t.Fatal(err)
	}
	selectedPDF, err := doc.ToPDFWithOptions(options)
	if err != nil || !bytes.HasPrefix(selectedPDF, []byte("%PDF-")) || bytes.Equal(primaryPDF, selectedPDF) {
		t.Fatalf("selected PDF: %v", err)
	}
	if _, err := doc.PageSVGWithOptions(-1, options); err == nil {
		t.Fatal("expected negative page index error")
	}
	if _, err := doc.PagePNGWithOptions(-1, 96, options); err == nil {
		t.Fatal("expected negative page index error")
	}
}

func TestReplacesPagePathDataAtomically(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	red := [3]uint8{255, 0, 0}
	id, err := doc.AddPathObject(0, "M 0 0 L 20 0", [4]float64{0, 0, 20, 20}, PathStyle{Stroke: true, StrokeColor: &red})
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPagePathData(0, strconv.Itoa(id), "M 0 0 Q 10 20 20 0"); err != nil {
		t.Fatal(err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 1 || paths[0].AbbreviatedData == nil || *paths[0].AbbreviatedData != "M 0 0 Q 10 20 20 0" {
		t.Fatalf("updated path data: %+v, %v", paths, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `d="M 0 0 Q 10 20 20 0"`) || !strings.Contains(svg, `stroke="#ff0000"`) {
		t.Fatalf("updated path SVG: %v, %v", svg, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	before, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPagePathData(0, strconv.Itoa(id), "M 0 0 X 1 1"); err == nil {
		t.Fatal("expected invalid commands")
	}
	after, err := doc.Entry(pages[0].Path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid update changed page: %v", err)
	}
}

func TestSetsPagePathStrokeStyleAtomically(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddPathObject(0, "M 0 10 L 20 10", [4]float64{0, 0, 20, 20}, PathStyle{Stroke: true})
	if err != nil {
		t.Fatal(err)
	}
	dash := [2]float64{3, 2}
	offset := 1.0
	miter := 5.0
	if err := doc.SetPagePathStrokeStyle(0, strconv.Itoa(id), PathStrokeStyle{
		LineWidth: 2, Cap: "round", Join: "bevel", MiterLimit: &miter, DashPattern: &dash, DashOffset: &offset,
	}); err != nil {
		t.Fatal(err)
	}
	styles, err := doc.PageObjectStrokeStyles(0)
	if err != nil || len(styles) != 1 || styles[0].Style.LineWidth != 2 || styles[0].Style.Cap == nil || *styles[0].Style.Cap != "round" || styles[0].Style.DashPattern == nil || *styles[0].Style.DashPattern != dash {
		t.Fatalf("updated stroke style: %+v, %v", styles, err)
	}
	if err := doc.SetPagePathStrokeStyle(0, strconv.Itoa(id), PathStrokeStyle{
		LineWidth: 3, DashSequence: []float64{1, 2, 3, 4}, DashOffset: &offset,
	}); err != nil {
		t.Fatal(err)
	}
	styles, err = doc.PageObjectStrokeStyles(0)
	if err != nil || styles[0].Style.LineWidth != 3 || !reflect.DeepEqual(styles[0].Style.DashSequence, []float64{1, 2, 3, 4}) || styles[0].Style.DashPattern != nil {
		t.Fatalf("full stroke dash sequence: %+v, %v", styles, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	before, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPagePathStrokeStyle(0, strconv.Itoa(id), PathStrokeStyle{
		LineWidth: 4, DashPattern: &dash, DashSequence: []float64{3},
	}); err == nil {
		t.Fatal("expected conflicting dash definitions")
	}
	after, err := doc.Entry(pages[0].Path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid update changed page: %v", err)
	}
	bad := [2]float64{0, 2}
	if err := doc.SetPagePathStrokeStyle(0, strconv.Itoa(id), PathStrokeStyle{LineWidth: 1, DashPattern: &bad}); err == nil {
		t.Fatal("expected invalid dash pattern")
	}
	after, err = doc.Entry(pages[0].Path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid update changed page: %v", err)
	}
	if err := doc.SetPagePathStrokeStyle(0, strconv.Itoa(id), PathStrokeStyle{LineWidth: 0.4}); err != nil {
		t.Fatal(err)
	}
	styles, err = doc.PageObjectStrokeStyles(0)
	if err != nil || styles[0].Style.DashPattern != nil {
		t.Fatalf("cleared stroke style: %+v, %v", styles, err)
	}
}

func TestSetsFullPathDashSequenceAcrossBinding(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddPathObject(0, "M 0 10 L 20 10", [4]float64{0, 0, 20, 20}, PathStyle{Stroke: true})
	if err != nil {
		t.Fatal(err)
	}
	offset := 0.5
	if err := doc.SetPagePathDashSequence(0, strconv.Itoa(id), []float64{1, 2, 3, 4}, &offset); err != nil {
		t.Fatal(err)
	}
	styles, err := doc.PageObjectStrokeStyles(0)
	if err != nil || len(styles) != 1 || !reflect.DeepEqual(styles[0].Style.DashSequence, []float64{1, 2, 3, 4}) || styles[0].Style.DashOffset != offset {
		t.Fatalf("dash sequence: %+v, %v", styles, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	before, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPagePathDashSequence(0, strconv.Itoa(id), []float64{1, 0, 3}, nil); err == nil {
		t.Fatal("expected invalid dash sequence")
	}
	after, err := doc.Entry(pages[0].Path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid edit changed page: %v", err)
	}
	if err := doc.SetPagePathDashSequence(0, strconv.Itoa(id), nil, nil); err != nil {
		t.Fatal(err)
	}
	styles, err = doc.PageObjectStrokeStyles(0)
	if err != nil || styles[0].Style.DashSequence != nil {
		t.Fatalf("dash sequence not cleared: %+v, %v", styles, err)
	}
}

func TestCreatesPathAndDrawParamWithFullDashSequences(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddPathObject(0, "M 0 10 L 20 10", [4]float64{0, 0, 20, 20}, PathStyle{
		Stroke: true, DashSequence: []float64{1, 2, 3},
	})
	if err != nil {
		t.Fatal(err)
	}
	styles, err := doc.PageObjectStrokeStyles(0)
	if err != nil || !reflect.DeepEqual(styles[0].Style.DashSequence, []float64{1, 2, 3}) {
		t.Fatalf("path dash sequence: %+v, %v", styles, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	before, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	bad := [2]float64{1, 2}
	if _, err := doc.AddPathObject(0, "M 0 10 L 20 10", [4]float64{0, 0, 20, 20}, PathStyle{
		Stroke: true, DashPattern: &bad, DashSequence: []float64{3},
	}); err == nil {
		t.Fatal("expected conflicting dash definitions")
	}
	after, err := doc.Entry(pages[0].Path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid edit changed page: %v", err)
	}
	if _, err := doc.AddPathObject(0, "M 0 10 L 20 10", [4]float64{0, 0, 20, 20}, PathStyle{
		Stroke: true, DashSequence: []float64{},
	}); err == nil {
		t.Fatal("expected empty dash sequence to fail")
	}
	drawID, err := doc.AddDocumentDrawParam(0, DrawParamStyle{DashSequence: []float64{4, 1, 2, 1}})
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPagePathDashSequence(0, strconv.Itoa(id), nil, nil); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectDrawParam(0, strconv.Itoa(id), strconv.Itoa(drawID)); err != nil {
		t.Fatal(err)
	}
	styles, err = doc.PageObjectStrokeStyles(0)
	if err != nil || !reflect.DeepEqual(styles[0].Style.DashSequence, []float64{4, 1, 2, 1}) {
		t.Fatalf("inherited dash sequence: %+v, %v", styles, err)
	}
}

func TestReadsFullPathDashSequenceAcrossBinding(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddPathObject(0, "M 0 10 L 20 10", [4]float64{0, 0, 20, 20}, PathStyle{Stroke: true}); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	source, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	updated := bytes.Replace(source, []byte("<ofd:PathObject "), []byte(`<ofd:PathObject DashPattern="1 2 3 4" DashOffset="0.5" `), 1)
	if err := doc.SetEntry(pages[0].Path, updated); err != nil {
		t.Fatal(err)
	}
	styles, err := doc.PageObjectStrokeStyles(0)
	if err != nil || len(styles) != 1 || styles[0].Style.DashPattern != nil || !slices.Equal(styles[0].Style.DashSequence, []float64{1, 2, 3, 4}) {
		t.Fatalf("full dash sequence: %+v, %v", styles, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `stroke-dasharray="1 2 3 4"`) {
		t.Fatalf("full dash sequence SVG: %v", err)
	}
}

func TestWritesReusableDrawParamsAndAssignsObjects(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddPathObject(0, "M 0 0 L 20 0", [4]float64{0, 0, 20, 20}, PathStyle{Stroke: true})
	if err != nil {
		t.Fatal(err)
	}
	width := 1.5
	dashOffset := 1.0
	dashPattern := [2]float64{3, 2}
	alpha := uint8(200)
	base, err := doc.AddDocumentDrawParam(0, DrawParamStyle{
		LineWidth: &width,
		Cap:       "Round", Join: "Bevel", DashPattern: &dashPattern, DashOffset: &dashOffset,
		Stroke: &SolidPaintStyle{Color: [3]uint8{255, 0, 0}, Alpha: &alpha},
	})
	if err != nil {
		t.Fatal(err)
	}
	derived, err := doc.AddDocumentDrawParam(0, DrawParamStyle{
		Relative: strconv.Itoa(base),
		Fill:     &SolidPaintStyle{Color: [3]uint8{0, 0, 255}},
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectDrawParam(0, strconv.Itoa(id), strconv.Itoa(derived)); err != nil {
		t.Fatal(err)
	}
	paints, err := doc.PageObjectPaints(0)
	if err != nil || len(paints) != 1 || paints[0].EffectiveStrokeDrawParam == nil || *paints[0].EffectiveStrokeDrawParam != strconv.Itoa(base) || paints[0].EffectiveStroke == nil || paints[0].EffectiveStroke.Alpha != alpha || paints[0].EffectiveFillDrawParam == nil || *paints[0].EffectiveFillDrawParam != strconv.Itoa(derived) {
		t.Fatalf("reusable DrawParam: %+v, %v", paints, err)
	}
	strokes, err := doc.PageObjectStrokeStyles(0)
	if err != nil || len(strokes) != 1 || !strokes[0].StrokeEnabled || strokes[0].Style.Cap == nil || *strokes[0].Style.Cap != "round" || strokes[0].Style.Join == nil || *strokes[0].Style.Join != "bevel" || strokes[0].Style.DashPattern == nil || *strokes[0].Style.DashPattern != dashPattern || strokes[0].Style.DashOffset != dashOffset {
		t.Fatalf("effective stroke style: %+v, %v", strokes, err)
	}
	before, err := doc.Entry("Doc_0/PublicRes.xml")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddDocumentDrawParam(0, DrawParamStyle{Relative: "999999"}); err == nil {
		t.Fatal("expected invalid Relative")
	}
	after, err := doc.Entry("Doc_0/PublicRes.xml")
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed DrawParam write changed resources: %v", err)
	}
	if err := doc.SetPageObjectDrawParam(0, strconv.Itoa(id), "999999"); err == nil {
		t.Fatal("expected missing DrawParam")
	}
	if err := doc.SetPageObjectDrawParam(0, strconv.Itoa(id), ""); err != nil {
		t.Fatal(err)
	}
	paints, err = doc.PageObjectPaints(0)
	if err != nil || paints[0].EffectiveStroke != nil {
		t.Fatalf("cleared DrawParam: %+v, %v", paints, err)
	}
	strokes, err = doc.PageObjectStrokeStyles(0)
	if err != nil || len(strokes) != 1 || strokes[0].Style.DashPattern != nil {
		t.Fatalf("cleared stroke style: %+v, %v", strokes, err)
	}
}

func TestPageDrawParamIsLocalAndInheritsPublicStyle(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	if err := doc.AppendBlankPage(0, 20, 20); err != nil {
		t.Fatal(err)
	}
	first, err := doc.AddPathObject(0, "M 0 0 L 10 0 L 10 10 C", [4]float64{0, 0, 10, 10}, PathStyle{Fill: true, Stroke: false})
	if err != nil {
		t.Fatal(err)
	}
	second, err := doc.AddPathObject(1, "M 0 0 L 10 0 L 10 10 C", [4]float64{0, 0, 10, 10}, PathStyle{Fill: true, Stroke: false})
	if err != nil {
		t.Fatal(err)
	}
	public, err := doc.AddDocumentDrawParam(0, DrawParamStyle{
		Fill: &SolidPaintStyle{Color: [3]uint8{255, 0, 0}},
	})
	if err != nil {
		t.Fatal(err)
	}
	local, err := doc.AddPageDrawParam(0, DrawParamStyle{
		Relative: strconv.Itoa(public), Fill: &SolidPaintStyle{Color: [3]uint8{0, 255, 0}},
	})
	if err != nil {
		t.Fatal(err)
	}
	documentParams, err := doc.DocumentDrawParams(0)
	if err != nil || len(documentParams) != 1 || documentParams[0].ID != strconv.Itoa(public) {
		t.Fatalf("document drawing parameters: %+v, %v", documentParams, err)
	}
	pageParams, err := doc.PageDrawParams(0)
	if err != nil || len(pageParams) != 2 {
		t.Fatalf("page drawing parameters: %+v, %v", pageParams, err)
	}
	var localInfo *DrawParam
	for i := range pageParams {
		if pageParams[i].ID == strconv.Itoa(local) {
			localInfo = &pageParams[i]
		}
	}
	if localInfo == nil || localInfo.Relative == nil || *localInfo.Relative != strconv.Itoa(public) ||
		!strings.Contains(localInfo.XML, "<ofd:DrawParam") ||
		!strings.HasSuffix(localInfo.ResourcePath, fmt.Sprintf("RustPageDrawParams_%d.xml", local)) {
		t.Fatalf("page-local drawing parameter: %+v", localInfo)
	}
	otherParams, err := doc.PageDrawParams(1)
	if err != nil || len(otherParams) != 1 || otherParams[0].ID != strconv.Itoa(public) {
		t.Fatalf("other page drawing parameters: %+v, %v", otherParams, err)
	}
	if _, err := doc.PageDrawParams(-1); err == nil {
		t.Fatal("accepted negative page index for drawing parameter reader")
	}
	if err := doc.SetPageObjectDrawParam(0, strconv.Itoa(first), strconv.Itoa(local)); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectDrawParam(1, strconv.Itoa(second), strconv.Itoa(public)); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetDocumentDrawParam(0, strconv.Itoa(public), DrawParamStyle{
		Fill: &SolidPaintStyle{Color: [3]uint8{0, 0, 255}},
	}); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageDrawParam(0, strconv.Itoa(local), DrawParamStyle{
		Relative: strconv.Itoa(public), Fill: &SolidPaintStyle{Color: [3]uint8{0, 255, 0}},
	}); err != nil {
		t.Fatal(err)
	}
	otherSVG, err := doc.PageSVG(1)
	if err != nil || !strings.Contains(otherSVG, `fill="#0000ff"`) {
		t.Fatalf("updated document DrawParam: %v", err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `fill="#00ff00"`) {
		t.Fatalf("page-local fill: %v", err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageDrawParam(0, strconv.Itoa(local), DrawParamStyle{Relative: strconv.Itoa(local)}); err == nil {
		t.Fatal("accepted self-referential page DrawParam")
	}
	if err := doc.SetPageDrawParam(1, strconv.Itoa(local), DrawParamStyle{}); err == nil {
		t.Fatal("updated page-local DrawParam through another page")
	}
	if err := doc.SetDocumentDrawParam(0, strconv.Itoa(local), DrawParamStyle{}); err == nil {
		t.Fatal("updated page-local DrawParam through document resources")
	}
	if err := doc.SetPageObjectDrawParam(1, strconv.Itoa(second), strconv.Itoa(local)); err == nil {
		t.Fatal("used page-local DrawParam on another page")
	}
	if _, err := doc.AddPageDrawParam(1, DrawParamStyle{Relative: strconv.Itoa(local)}); err == nil {
		t.Fatal("inherited page-local DrawParam from another page")
	}
	if _, err := doc.AddPageDrawParam(-1, DrawParamStyle{}); err == nil {
		t.Fatal("accepted negative page index")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed page DrawParam edit mutated package: %v", err)
	}
	if err := doc.RemovePageDrawParam(0, strconv.Itoa(local)); err == nil {
		t.Fatal("removed referenced page DrawParam")
	}
	if err := doc.RemoveDocumentDrawParam(0, strconv.Itoa(public)); err == nil {
		t.Fatal("removed referenced document DrawParam")
	}
	if err := doc.SetPageObjectDrawParam(0, strconv.Itoa(first), ""); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectDrawParam(1, strconv.Itoa(second), ""); err != nil {
		t.Fatal(err)
	}
	if err := doc.RemovePageDrawParam(0, strconv.Itoa(local)); err != nil {
		t.Fatal(err)
	}
	if err := doc.RemoveDocumentDrawParam(0, strconv.Itoa(public)); err != nil {
		t.Fatal(err)
	}
	params, err := doc.PageDrawParams(0)
	if err != nil || len(params) != 0 {
		t.Fatalf("remaining page DrawParams: %+v, %v", params, err)
	}
}

func TestDrawParamGradientAndGouraudPaints(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddPathObject(0, "M 0 0 L 20 0 L 20 20 L 0 20 C", [4]float64{0, 0, 20, 20}, PathStyle{Fill: true, Stroke: false})
	if err != nil {
		t.Fatal(err)
	}
	gradient := GradientFillStyle{
		Kind: "axial", StartPoint: [2]float64{0, 0}, EndPoint: [2]float64{20, 0},
		Segments: []GradientSegment{{Color: [3]uint8{255, 0, 0}}, {Color: [3]uint8{0, 0, 255}}},
	}
	drawParam, err := doc.AddDocumentDrawParam(0, DrawParamStyle{FillGradient: &gradient})
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectDrawParam(0, strconv.Itoa(id), strconv.Itoa(drawParam)); err != nil {
		t.Fatal(err)
	}
	paints, err := doc.PageObjectPaints(0)
	if err != nil || len(paints) != 1 || paints[0].EffectiveFill == nil || paints[0].EffectiveFill.Kind != "Axial" {
		t.Fatalf("gradient DrawParam: %+v, %v", paints, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "linearGradient") {
		t.Fatalf("gradient SVG: %v, %v", svg, err)
	}
	mesh := GouraudFillStyle{Points: []GouraudControlPoint{
		{X: 0, Y: 0, Color: [3]uint8{255, 0, 0}},
		{X: 20, Y: 0, Color: [3]uint8{0, 255, 0}},
		{X: 0, Y: 20, Color: [3]uint8{0, 0, 255}},
	}}
	meshID, err := doc.AddDocumentDrawParam(0, DrawParamStyle{FillGouraud: &mesh})
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectDrawParam(0, strconv.Itoa(id), strconv.Itoa(meshID)); err != nil {
		t.Fatal(err)
	}
	paints, err = doc.PageObjectPaints(0)
	if err != nil || paints[0].EffectiveFill == nil || paints[0].EffectiveFill.Kind != "Gouraud" {
		t.Fatalf("Gouraud DrawParam: %+v, %v", paints, err)
	}
	before, err := doc.Entry("Doc_0/PublicRes.xml")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddDocumentDrawParam(0, DrawParamStyle{
		Fill: &SolidPaintStyle{Color: [3]uint8{1, 2, 3}}, FillGradient: &gradient,
	}); err == nil {
		t.Fatal("expected conflicting fill paints")
	}
	after, err := doc.Entry("Doc_0/PublicRes.xml")
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed DrawParam write changed resources: %v", err)
	}
}

func TestDrawParamPatternPaintReusesCells(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	ids := make([]int, 0, 2)
	for _, x := range []float64{0, 10} {
		id, err := doc.AddPathObject(0, "M 0 0 L 8 0 L 8 8 L 0 8 C", [4]float64{x, 0, 8, 8}, PathStyle{Fill: true, Stroke: false})
		if err != nil {
			t.Fatal(err)
		}
		ids = append(ids, id)
	}
	red := [3]uint8{255, 0, 0}
	pattern := PatternStyle{
		Width: 4, Height: 4,
		Cells: []PatternCell{{
			Kind: "path", Commands: "M 0 0 L 4 0 L 4 4 L 0 4 C",
			Boundary:  [4]float64{0, 0, 4, 4},
			PathStyle: &PathStyle{Fill: true, Stroke: false, LineWidth: 0.4, FillColor: &red},
		}},
	}
	drawParam, err := doc.AddDocumentDrawParam(0, DrawParamStyle{FillPattern: &pattern})
	if err != nil {
		t.Fatal(err)
	}
	for _, id := range ids {
		if err := doc.SetPageObjectDrawParam(0, strconv.Itoa(id), strconv.Itoa(drawParam)); err != nil {
			t.Fatal(err)
		}
	}
	paints, err := doc.PageObjectPaints(0)
	if err != nil || len(paints) != 2 {
		t.Fatalf("pattern DrawParam: %+v, %v", paints, err)
	}
	for _, paint := range paints {
		if paint.EffectiveFill == nil || paint.EffectiveFill.Kind != "Pattern" || !strings.Contains(paint.EffectiveFill.XML, "CellContent") {
			t.Fatalf("pattern paint: %+v", paint)
		}
	}
	if png, err := doc.PagePNG(0, 25.4); err != nil || len(png) == 0 {
		t.Fatalf("pattern PNG: %d bytes, %v", len(png), err)
	}
	before, err := doc.Entry("Doc_0/PublicRes.xml")
	if err != nil {
		t.Fatal(err)
	}
	invalid := PatternStyle{Width: 4, Height: 4, Cells: []PatternCell{{
		Kind: "image", ResourceID: "999999", Boundary: [4]float64{0, 0, 4, 4},
	}}}
	if _, err := doc.AddDocumentDrawParam(0, DrawParamStyle{FillPattern: &invalid}); err == nil {
		t.Fatal("expected missing image resource")
	}
	after, err := doc.Entry("Doc_0/PublicRes.xml")
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed pattern write changed resources: %v", err)
	}
}

func TestDrawParamThumbnailPattern(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "发票示例.ofd")
	source, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	png, err := source.PageImageData(0, 0)
	source.Close()
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	mediaID, err := doc.AddMultimediaResource(0, "Image", "PNG", png)
	if err != nil {
		t.Fatal(err)
	}
	pathID, err := doc.AddPathObject(0, "M 0 0 L 20 0 L 20 20 L 0 20 C", [4]float64{0, 0, 20, 20}, PathStyle{Fill: true})
	if err != nil {
		t.Fatal(err)
	}
	thumbnail := strconv.Itoa(mediaID)
	pattern := PatternStyle{Width: 4, Height: 4, Thumbnail: &thumbnail}
	drawParam, err := doc.AddDocumentDrawParam(0, DrawParamStyle{FillPattern: &pattern})
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectDrawParam(0, strconv.Itoa(pathID), strconv.Itoa(drawParam)); err != nil {
		t.Fatal(err)
	}
	resources, err := doc.Entry("Doc_0/PublicRes.xml")
	if err != nil || !bytes.Contains(resources, []byte(`Thumbnail="`+thumbnail+`"`)) {
		t.Fatalf("pattern thumbnail: %s, %v", resources, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "<image") {
		t.Fatalf("thumbnail SVG: %v, %v", svg, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
	missing := "missing"
	pattern.Thumbnail = &missing
	if _, err := doc.AddDocumentDrawParam(0, DrawParamStyle{FillPattern: &pattern}); err == nil {
		t.Fatal("expected missing thumbnail error")
	}
	after, err := doc.Entry("Doc_0/PublicRes.xml")
	if err != nil || !bytes.Equal(resources, after) {
		t.Fatalf("failed update changed resources: %v", err)
	}
}

func TestPageObjectPaintsResolveDrawParamPrecedence(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddPathObject(0, "M 0 0 L 20 0", [4]float64{0, 0, 20, 20}, PathStyle{Stroke: true})
	if err != nil {
		t.Fatal(err)
	}
	resPath := "Doc_0/PublicRes.xml"
	res, err := doc.Entry(resPath)
	if err != nil {
		t.Fatal(err)
	}
	params := `<ofd:DrawParams><ofd:DrawParam ID="100"><ofd:StrokeColor Value="255 0 0"/></ofd:DrawParam><ofd:DrawParam ID="101" Relative="100"><ofd:FillColor Value="0 0 255"/></ofd:DrawParam></ofd:DrawParams>`
	if err := doc.SetEntry(resPath, []byte(strings.Replace(string(res), "</ofd:Res>", params+"</ofd:Res>", 1))); err != nil {
		t.Fatal(err)
	}
	pagePath := "Doc_0/Pages/Page_0/Content.xml"
	page, err := doc.Entry(pagePath)
	if err != nil {
		t.Fatal(err)
	}
	needle := fmt.Sprintf(`<ofd:PathObject ID="%d"`, id)
	if err := doc.SetEntry(pagePath, []byte(strings.Replace(string(page), needle, needle+` DrawParam="101"`, 1))); err != nil {
		t.Fatal(err)
	}
	paints, err := doc.PageObjectPaints(0)
	if err != nil || len(paints) != 1 || paints[0].Stroke != nil || paints[0].EffectiveStroke == nil || paints[0].EffectiveStroke.Value == nil || *paints[0].EffectiveStroke.Value != "255 0 0" || paints[0].EffectiveStrokeDrawParam == nil || *paints[0].EffectiveStrokeDrawParam != "100" || paints[0].EffectiveFill == nil || paints[0].EffectiveFillDrawParam == nil || *paints[0].EffectiveFillDrawParam != "101" {
		t.Fatalf("inherited paints: %+v, %v", paints, err)
	}
	if err := doc.SetPageObjectSolidStroke(0, strconv.Itoa(id), SolidPaintStyle{Color: [3]uint8{0, 255, 0}}); err != nil {
		t.Fatal(err)
	}
	paints, err = doc.PageObjectPaints(0)
	if err != nil || paints[0].Stroke == nil || paints[0].EffectiveStroke == nil || paints[0].EffectiveStrokeDrawParam != nil || *paints[0].EffectiveStroke.Value != "0 255 0" {
		t.Fatalf("direct paint override: %+v, %v", paints, err)
	}
}

func TestSolidPaintReplacesComplexFillAndSetsStroke(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddPathObject(0, "M 0 0 L 20 0 L 20 20 L 0 20 C", [4]float64{0, 0, 20, 20}, PathStyle{})
	if err != nil {
		t.Fatal(err)
	}
	objectID := strconv.Itoa(id)
	gradient := GradientFillStyle{
		Kind: "axial", StartPoint: [2]float64{0, 0}, EndPoint: [2]float64{20, 0},
		Segments: []GradientSegment{{Color: [3]uint8{255, 0, 0}}, {Color: [3]uint8{0, 0, 255}}},
	}
	if err := doc.SetPageObjectGradientFill(0, objectID, gradient); err != nil {
		t.Fatal(err)
	}
	alpha := uint8(128)
	if err := doc.SetPageObjectSolidFill(0, objectID, SolidPaintStyle{Color: [3]uint8{10, 20, 30}, Alpha: &alpha}); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectSolidStroke(0, objectID, SolidPaintStyle{Color: [3]uint8{0, 255, 0}}); err != nil {
		t.Fatal(err)
	}
	paints, err := doc.PageObjectPaints(0)
	if err != nil || len(paints) != 1 || paints[0].Fill == nil || paints[0].Stroke == nil || paints[0].Fill.Kind != "Solid" || paints[0].Fill.Alpha != 128 || paints[0].Fill.Value == nil || *paints[0].Fill.Value != "10 20 30" || !paints[0].FillEnabled || !paints[0].StrokeEnabled {
		t.Fatalf("solid object paints: %+v, %v", paints, err)
	}
	if _, err := doc.PageObjectPaints(-1); err == nil {
		t.Fatal("expected negative page index error")
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `fill="#0a141e"`) || !strings.Contains(svg, `stroke="#00ff00"`) || strings.Contains(svg, "linearGradient") {
		t.Fatalf("solid paint SVG: %v, %v", svg, err)
	}
	before, err := doc.Entry("Doc_0/Pages/Page_0/Content.xml")
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectSolidFill(0, "missing", SolidPaintStyle{Color: [3]uint8{0, 0, 0}}); err == nil {
		t.Fatal("expected missing object")
	}
	after, err := doc.Entry("Doc_0/Pages/Page_0/Content.xml")
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed update changed page: %v", err)
	}
}

func TestPatternFillAndStrokePreserveDocumentIDs(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddPathObject(0, "M 0 0 L 20 0 L 20 20 L 0 20 C", [4]float64{0, 0, 20, 20}, PathStyle{})
	if err != nil {
		t.Fatal(err)
	}
	red := [3]uint8{255, 0, 0}
	step := 6.0
	style := PatternStyle{
		Width: 4, Height: 4, XStep: &step, YStep: &step, Reflect: "row_and_column",
		Cells: []PatternCell{{
			Kind: "path", Commands: "M 0 0 L 2 0 L 2 4 L 0 4 C",
			Boundary:  [4]float64{0, 0, 2, 4},
			PathStyle: &PathStyle{Fill: true, Stroke: false, LineWidth: 0.4, FillColor: &red},
		}},
	}
	if err := doc.SetPageObjectPatternFill(0, strconv.Itoa(id), style); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectPatternStroke(0, strconv.Itoa(id), style); err != nil {
		t.Fatal(err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 1 {
		t.Fatalf("page paths include pattern cells: %+v, %v", paths, err)
	}
	layers, err := doc.PageLayers(0)
	if err != nil || len(layers) != 1 || layers[0].ObjectCount != 1 {
		t.Fatalf("pattern cells counted as page objects: %+v, %v", layers, err)
	}
	paints, err := doc.PageObjectPaints(0)
	if err != nil || len(paints) != 1 || paints[0].Fill == nil || paints[0].Stroke == nil || paints[0].Fill.Kind != "Pattern" || paints[0].Stroke.Kind != "Pattern" || !strings.Contains(paints[0].Fill.XML, "CellContent") {
		t.Fatalf("pattern object paints: %+v, %v", paints, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `fill="url(#ofd-pattern-`) || !strings.Contains(svg, `stroke="url(#ofd-pattern-`) {
		t.Fatalf("pattern SVG: %v, %v", svg, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("pattern references: %v, %v", issues, err)
	}
	before, err := doc.Entry("Doc_0/Pages/Page_0/Content.xml")
	if err != nil {
		t.Fatal(err)
	}
	style.Width = 0
	if err := doc.SetPageObjectPatternFill(0, strconv.Itoa(id), style); err == nil {
		t.Fatal("expected invalid width")
	}
	after, err := doc.Entry("Doc_0/Pages/Page_0/Content.xml")
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed update changed page: %v", err)
	}
	textStyle := PatternStyle{Width: 4, Height: 4, Cells: []PatternCell{{
		Kind: "text", Text: "tile", Size: 2, Boundary: [4]float64{0, 0, 4, 4},
	}}}
	if err := doc.SetPageObjectPatternFill(0, strconv.Itoa(id), textStyle); err != nil {
		t.Fatal(err)
	}
	pageText, err := doc.PageText(0)
	if err != nil || len(pageText) != 0 {
		t.Fatalf("page text includes pattern cell: %v, %v", pageText, err)
	}
	textObjects, err := doc.PageTextObjects(0)
	if err != nil || len(textObjects) != 0 {
		t.Fatalf("page text objects include pattern cell: %+v, %v", textObjects, err)
	}
}

func TestThumbnailOnlyPattern(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "发票示例.ofd")
	source, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	png, err := source.PageImageData(0, 0)
	source.Close()
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	imageID, err := doc.AddImageObject(0, png, "png", [4]float64{0, 0, 4, 4})
	if err != nil {
		t.Fatal(err)
	}
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 || images[0].ResourceID == "" {
		t.Fatalf("page images: %+v, %v", images, err)
	}
	thumbnail := images[0].ResourceID
	if err := doc.RemovePageObject(0, strconv.Itoa(imageID)); err != nil {
		t.Fatal(err)
	}
	pathID, err := doc.AddPathObject(0, "M 0 0 L 20 0 L 20 20 L 0 20 C", [4]float64{0, 0, 20, 20}, PathStyle{})
	if err != nil {
		t.Fatal(err)
	}
	style := PatternStyle{Width: 4, Height: 4, Thumbnail: &thumbnail}
	if err := doc.SetPageObjectPatternFill(0, strconv.Itoa(pathID), style); err != nil {
		t.Fatal(err)
	}
	content, err := doc.Entry("Doc_0/Pages/Page_0/Content.xml")
	if err != nil || !bytes.Contains(content, []byte(`Thumbnail="`+thumbnail+`"`)) {
		t.Fatalf("pattern thumbnail: %s, %v", content, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "<image") {
		t.Fatalf("thumbnail SVG: %v, %v", svg, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
	missing := "missing"
	style.Thumbnail = &missing
	if err := doc.SetPageObjectPatternFill(0, strconv.Itoa(pathID), style); err == nil {
		t.Fatal("expected missing thumbnail error")
	}
	after, err := doc.Entry("Doc_0/Pages/Page_0/Content.xml")
	if err != nil || !bytes.Equal(content, after) {
		t.Fatalf("failed update changed page: %v", err)
	}
}

func TestGradientFillWritesAndRejectsInvalidGeometryAtomically(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddPathObject(0, "M 0 0 L 20 0 L 20 20 L 0 20 C", [4]float64{0, 0, 20, 20}, PathStyle{})
	if err != nil {
		t.Fatal(err)
	}
	style := GradientFillStyle{
		Kind: "axial", StartPoint: [2]float64{0, 0}, EndPoint: [2]float64{20, 0},
		Segments: []GradientSegment{{Color: [3]uint8{255, 0, 0}}, {Color: [3]uint8{0, 0, 255}}},
	}
	if err := doc.SetPageObjectGradientFill(0, strconv.Itoa(id), style); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectGradientStroke(0, strconv.Itoa(id), style); err != nil {
		t.Fatal(err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "<linearGradient") || !strings.Contains(svg, `stroke="url(#ofd-shading-`) {
		t.Fatalf("axial SVG: %v, %v", svg, err)
	}
	page := "Doc_0/Pages/Page_0/Content.xml"
	before, err := doc.Entry(page)
	if err != nil {
		t.Fatal(err)
	}
	style.EndPoint = style.StartPoint
	if err := doc.SetPageObjectGradientFill(0, strconv.Itoa(id), style); err == nil {
		t.Fatal("expected zero axis error")
	}
	if err := doc.SetPageObjectGradientStroke(0, strconv.Itoa(id), style); err == nil {
		t.Fatal("expected zero stroke axis error")
	}
	after, err := doc.Entry(page)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed update changed page: %v", err)
	}
	style.Kind = "radial"
	radius := 10.0
	style.EndRadius = &radius
	style.StartPoint = [2]float64{10, 10}
	style.EndPoint = style.StartPoint
	if err := doc.SetPageObjectGradientFill(0, strconv.Itoa(id), style); err != nil {
		t.Fatal(err)
	}
	svg, err = doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "<radialGradient") {
		t.Fatalf("radial SVG: %v, %v", svg, err)
	}
}

func TestGouraudFillWritesAndRejectsInvalidMeshAtomically(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddPathObject(0, "M 0 0 L 20 0 L 20 20 L 0 20 C", [4]float64{0, 0, 20, 20}, PathStyle{Stroke: false})
	if err != nil {
		t.Fatal(err)
	}
	columns := 2
	style := GouraudFillStyle{Points: []GouraudControlPoint{
		{X: 0, Y: 0, Color: [3]uint8{255, 0, 0}},
		{X: 20, Y: 0, Color: [3]uint8{0, 255, 0}},
		{X: 0, Y: 20, Color: [3]uint8{0, 0, 255}},
		{X: 20, Y: 20, Color: [3]uint8{255, 255, 0}},
	}, VerticesPerRow: &columns}
	if err := doc.SetPageObjectGouraudFill(0, strconv.Itoa(id), style); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectGouraudStroke(0, strconv.Itoa(id), style); err != nil {
		t.Fatal(err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "ofd-gouraud-") || !strings.Contains(svg, `stroke="url(#ofd-gouraud-`) {
		t.Fatalf("Gouraud SVG: %v, %v", svg, err)
	}
	page := "Doc_0/Pages/Page_0/Content.xml"
	before, err := doc.Entry(page)
	if err != nil {
		t.Fatal(err)
	}
	invalid := 3
	style.VerticesPerRow = &invalid
	if err := doc.SetPageObjectGouraudFill(0, strconv.Itoa(id), style); err == nil {
		t.Fatal("expected invalid point grid")
	}
	if err := doc.SetPageObjectGouraudStroke(0, strconv.Itoa(id), style); err == nil {
		t.Fatal("expected invalid stroke point grid")
	}
	after, err := doc.Entry(page)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed update changed page: %v", err)
	}
}

func TestImageBorderWritesAndRendersAtomically(t *testing.T) {
	path := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "image-mask.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	style := DefaultImageBorderStyle()
	style.LineWidth = 0.4
	style.Color = [3]uint8{0, 255, 0}
	style.DashPattern = []float64{1, 2, 3}
	if err := doc.SetPageImageBorder(0, "5", &style); err != nil {
		t.Fatal(err)
	}
	images, err := doc.PageImages(0)
	if err != nil || images[0].Border == nil || images[0].Border.LineWidth != 0.4 ||
		images[0].Border.ColorValue == nil || *images[0].Border.ColorValue != "0 255 0" {
		t.Fatalf("border metadata: %v, %v", images, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `stroke="#00ff00"`) {
		t.Fatalf("border SVG: %v, %v", svg, err)
	}
	before, err := doc.Entry("Doc_0/Pages/Page_0/Content.xml")
	if err != nil {
		t.Fatal(err)
	}
	invalid := style
	invalid.DashPattern = []float64{1, 0}
	if err := doc.SetPageImageBorder(0, "5", &invalid); err == nil {
		t.Fatal("expected invalid dash pattern error")
	}
	after, err := doc.Entry("Doc_0/Pages/Page_0/Content.xml")
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed update changed page: %v", err)
	}
	if err := doc.SetPageImageBorder(-1, "5", &style); err == nil {
		t.Fatal("expected negative page index error")
	}
	if err := doc.SetPageImageBorder(0, "bad\x00id", &style); err == nil {
		t.Fatal("expected invalid object ID error")
	}
	invalid.LineWidth = math.NaN()
	if err := doc.SetPageImageBorder(0, "5", &invalid); err == nil {
		t.Fatal("expected invalid JSON number error")
	}
	if err := doc.SetPageImageBorder(0, "5", nil); err != nil {
		t.Fatal(err)
	}
	images, err = doc.PageImages(0)
	if err != nil || images[0].Border != nil {
		t.Fatalf("cleared border metadata: %v, %v", images, err)
	}
	svg, err = doc.PageSVG(0)
	if err != nil || strings.Contains(svg, `stroke="#00ff00"`) {
		t.Fatalf("cleared border SVG: %v, %v", svg, err)
	}
	closed, err := New()
	if err != nil {
		t.Fatal(err)
	}
	closed.Close()
	if err := closed.SetPageImageBorder(0, "5", &style); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestSignedPageRejectsObjectAttributeChanges(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-sign", "src", "test", "resources", "signedFile.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	pagePath := "Doc_0/Pages/Page_0/Content.xml"
	before, err := doc.Entry(pagePath)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageObjectVisible(0, "62", false); err == nil {
		t.Fatal("expected signed page rejection")
	}
	if err := doc.SetPageObjectAlpha(0, "62", 128); err == nil {
		t.Fatal("expected signed page rejection")
	}
	if err := doc.SetPageObjectBoundary(0, "62", [4]float64{1, 1, 10, 10}); err == nil {
		t.Fatal("expected signed page rejection")
	}
	ctm := [6]float64{1, 0, 0, 1, 2, 3}
	if err := doc.SetPageObjectCTM(0, "62", &ctm); err == nil {
		t.Fatal("expected signed page rejection")
	}
	_, err = doc.AddPathObject(0, "M 0 0 L 5 5", [4]float64{1, 1, 5, 5}, PathStyle{})
	if err == nil || !strings.Contains(err.Error(), "signed") {
		t.Fatalf("expected signed page rejection: %v", err)
	}
	_, err = doc.AddPageLayer(0, "Body", "")
	if err == nil || !strings.Contains(err.Error(), "signed") {
		t.Fatalf("expected signed page rejection: %v", err)
	}
	for _, err := range []error{
		doc.MovePageObjectToLayer(0, "62", "60"),
		doc.MovePageObject(0, "62", "64"),
		doc.RemovePageLayer(0, "60"),
		doc.RemovePageObject(0, "62"),
		doc.SetPageArea(0, PageArea{PhysicalBox: [4]float64{0, 0, 210, 297}}),
		doc.AppendBlankPage(0, 10, 10),
		doc.AddAttachmentToDocument(0, "x.txt", []byte("x")),
		doc.RemoveAttachment(0),
	} {
		if err == nil || !strings.Contains(err.Error(), "signed") {
			t.Fatalf("expected signed page rejection: %v", err)
		}
	}
	after, err := doc.Entry(pagePath)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("signed page changed: %v", err)
	}
}

func TestSharedSignedFilesRejectEditsAcrossBinding(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-sign", "src", "test", "resources", "signedFile.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	root, err := doc.Entry("Doc_0/Document.xml")
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetEntry("Doc_0/DocumentCopy.xml", root); err != nil {
		t.Fatal(err)
	}
	ofd, err := doc.Entry("OFD.xml")
	if err != nil {
		t.Fatal(err)
	}
	sharedBody := []byte("<ofd:DocBody><ofd:DocRoot>Doc_0/DocumentCopy.xml</ofd:DocRoot></ofd:DocBody></ofd:OFD>")
	modified := bytes.Replace(ofd, []byte("</ofd:OFD>"), sharedBody, 1)
	if bytes.Equal(modified, ofd) {
		t.Fatal("fixture has no OFD closing tag")
	}
	if err := doc.SetEntry("OFD.xml", modified); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil || len(pages) != 2 {
		t.Fatalf("shared pages: %v, %v", pages, err)
	}
	attachments, err := doc.Attachments()
	if err != nil || len(attachments) != 2 {
		t.Fatalf("shared attachments: %v, %v", attachments, err)
	}
	protected := []string{
		"Doc_0/Pages/Page_0/Content.xml",
		"Doc_0/PublicRes.xml",
		"Doc_0/Attachs/Attachments.xml",
	}
	before := make(map[string][]byte, len(protected))
	for _, name := range protected {
		before[name], err = doc.Entry(name)
		if err != nil {
			t.Fatal(err)
		}
	}
	boundary := [4]float64{0, 0, 2, 2}
	checks := []struct {
		name string
		run  func() error
	}{
		{"object visibility", func() error { return doc.SetPageObjectVisible(1, "62", false) }},
		{"object alpha", func() error { return doc.SetPageObjectAlpha(1, "62", 128) }},
		{"page area", func() error { return doc.SetPageArea(1, PageArea{PhysicalBox: [4]float64{0, 0, 210, 140}}) }},
		{"page layer", func() error { _, err := doc.AddPageLayer(1, "Body", ""); return err }},
		{"path object", func() error { _, err := doc.AddPathObject(1, "M 0 0 L 1 1", boundary, PathStyle{}); return err }},
		{"composite object", func() error {
			_, err := doc.AddCompositePathObject(1, "M 0 0 L 1 1", boundary, PathStyle{})
			return err
		}},
		{"path annotation", func() error { _, err := doc.AddPathAnnotation(1, "M 0 0 L 1 1", boundary, PathStyle{}); return err }},
		{"page color space", func() error {
			_, err := doc.AddPageColorSpace(1, ColorSpaceSpec{ColorType: "RGB", BitsPerComponent: 8})
			return err
		}},
		{"document color space", func() error {
			_, err := doc.AddDocumentColorSpace(1, ColorSpaceSpec{ColorType: "RGB", BitsPerComponent: 8})
			return err
		}},
		{"draw parameter", func() error {
			_, err := doc.AddDocumentDrawParam(1, DrawParamStyle{Stroke: &SolidPaintStyle{Color: [3]uint8{255, 0, 0}}})
			return err
		}},
		{"attachment rename", func() error { return doc.RenameAttachment(1, "changed") }},
		{"attachment metadata", func() error { return doc.SetAttachmentMetadata(1, AttachmentMetadata{}) }},
		{"attachment add", func() error { return doc.AddAttachmentToDocument(1, "new", []byte("new")) }},
		{"attachment remove", func() error { return doc.RemoveAttachment(1) }},
	}
	for _, check := range checks {
		if err := check.run(); err == nil || !strings.Contains(err.Error(), "signed") {
			t.Fatalf("%s: expected signed-file rejection, got %v", check.name, err)
		}
	}
	for _, name := range protected {
		after, err := doc.Entry(name)
		if err != nil || !bytes.Equal(after, before[name]) {
			t.Fatalf("protected entry %s changed: %v", name, err)
		}
	}
}

func TestClosedDocumentRejectsPublicAPIs(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	doc.Close()
	checks := []struct {
		name string
		call func() error
	}{
		{"page sources", func() error { _, err := doc.PageSources(0); return err }},
		{"page layers", func() error { _, err := doc.PageLayers(0); return err }},
		{"page objects", func() error { _, err := doc.PageObjects(0); return err }},
		{"object paints", func() error { _, err := doc.PageObjectPaints(0); return err }},
		{"stroke styles", func() error { _, err := doc.PageObjectStrokeStyles(0); return err }},
		{"object actions", func() error { _, err := doc.PageObjectActions(0, "1"); return err }},
		{"source actions", func() error { _, err := doc.PageSourceObjectActions(0, "page.xml", "1"); return err }},
		{"pages", func() error { _, err := doc.Pages(); return err }},
		{"references", func() error { _, err := doc.ValidateReferences(); return err }},
		{"entries", func() error { _, err := doc.Entries(); return err }},
		{"permissions", func() error { _, err := doc.DocumentPermissions(0); return err }},
		{"view preferences", func() error { _, err := doc.ViewPreferences(0); return err }},
		{"document actions", func() error { _, err := doc.DocumentActions(0); return err }},
		{"versions", func() error { _, err := doc.Versions(); return err }},
		{"bookmarks", func() error { _, err := doc.Bookmarks(); return err }},
		{"outlines", func() error { _, err := doc.Outlines(); return err }},
		{"page multimedia", func() error { _, err := doc.PageMultimediaResources(0); return err }},
		{"document multimedia", func() error { _, err := doc.DocumentMultimediaResources(0); return err }},
		{"composite objects", func() error { _, err := doc.PageCompositeObjects(0); return err }},
		{"attachments", func() error { _, err := doc.Attachments(); return err }},
		{"fonts", func() error { _, err := doc.Fonts(); return err }},
		{"page fonts", func() error { _, err := doc.PageFonts(0); return err }},
		{"page colors", func() error { _, err := doc.PageColorSpaces(0); return err }},
		{"document colors", func() error { _, err := doc.DocumentColorSpaces(0); return err }},
		{"default color", func() error { _, err := doc.DocumentDefaultColorSpace(0); return err }},
		{"page text objects", func() error { _, err := doc.PageTextObjects(0); return err }},
		{"page paths", func() error { _, err := doc.PagePaths(0); return err }},
		{"page images", func() error { _, err := doc.PageImages(0); return err }},
		{"composite images", func() error { _, err := doc.PageCompositeImages(0); return err }},
		{"remove page template", func() error { return doc.RemovePageTemplate(0, "1") }},
		{"remove document template", func() error { return doc.RemoveDocumentTemplate(0, "1") }},
		{"remove object action", func() error { return doc.RemovePageObjectAction(0, "1", 0) }},
		{"set permissions", func() error { return doc.SetDocumentPermissions(0, DefaultDocumentPermissions()) }},
		{"set view preferences", func() error { return doc.SetViewPreferences(0, DefaultViewPreferences()) }},
		{"add document action", func() error { return doc.AddDocumentAction(0, DocumentActionSpec{}) }},
		{"remove document action", func() error { return doc.RemoveDocumentAction(0, 0) }},
		{"remove bookmark", func() error { return doc.RemoveBookmark(0, "chapter") }},
		{"remove outline", func() error { return doc.RemoveOutline(0) }},
		{"set page area", func() error { return doc.SetPageArea(0, PageArea{}) }},
		{"move page", func() error { return doc.MovePage(0, 0) }},
		{"reorder pages", func() error { return doc.ReorderDocumentPages(0, []int{}) }},
		{"remove page", func() error { return doc.RemovePage(0) }},
		{"duplicate page", func() error { _, err := doc.DuplicatePage(0); return err }},
		{"duplicate page at", func() error { _, err := doc.DuplicatePageAt(0, 0); return err }},
		{"append text page", func() error { return doc.AppendTextPage(0, "x", 20, 20) }},
		{"append blank page", func() error { return doc.AppendBlankPage(0, 20, 20) }},
		{"insert text page", func() error { _, err := doc.InsertTextPage(0, 0, "x", 20, 20); return err }},
		{"insert blank page", func() error { _, err := doc.InsertBlankPage(0, 0, 20, 20); return err }},
		{"add positioned text", func() error { _, err := doc.AddTextObject(0, "x", [4]float64{0, 0, 10, 10}, 4, "1"); return err }},
		{"add shaped text", func() error { _, err := doc.AddShapedTextObject(0, "x", [4]float64{0, 0, 10, 10}, 4, "1"); return err }},
		{"add colored shaped text", func() error {
			_, err := doc.AddColoredShapedTextObjectWithLanguage(0, "x", [4]float64{0, 0, 10, 10}, 4, "1", "en", [3]uint8{1, 2, 3})
			return err
		}},
		{"set text code", func() error { return doc.SetPageTextCode(0, "1", 0, "x") }},
		{"font data", func() error { _, err := doc.FontData(0); return err }},
		{"page font data", func() error { _, err := doc.PageFontData(0, 0); return err }},
		{"add page font", func() error { _, err := doc.AddEmbeddedFontToPage(0, "x", []byte("bad")); return err }},
		{"set font attribute", func() error { return doc.SetFontAttribute(0, "Bold", nil) }},
		{"set page font attribute", func() error { return doc.SetPageFontAttribute(0, 0, "Bold", nil) }},
		{"replace font data", func() error { return doc.ReplaceFontData(0, []byte("bad")) }},
		{"replace page font data", func() error { return doc.ReplacePageFontData(0, 0, []byte("bad")) }},
		{"set object visibility", func() error { return doc.SetPageObjectVisible(0, "1", false) }},
		{"set object alpha", func() error { return doc.SetPageObjectAlpha(0, "1", 128) }},
		{"remove page layer", func() error { return doc.RemovePageLayer(0, "1") }},
		{"remove page object", func() error { return doc.RemovePageObject(0, "1") }},
		{"rename attachment", func() error { return doc.RenameAttachment(0, "x") }},
		{"remove attachment", func() error { return doc.RemoveAttachment(0) }},
		{"set attachment metadata", func() error { return doc.SetAttachmentMetadata(0, AttachmentMetadata{}) }},
		{"add attachment", func() error { return doc.AddAttachmentToDocument(0, "x", []byte("x")) }},
	}
	for _, check := range checks {
		if err := check.call(); err == nil || !strings.Contains(err.Error(), "closed") {
			t.Fatalf("%s: expected closed document error, got %v", check.name, err)
		}
	}
}

func TestReadsEmbeddedFont(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "keyword.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	fonts, err := doc.Fonts()
	if err != nil || len(fonts) != 1 || fonts[0].FontName != "宋体" || fonts[0].EmbeddedPath == nil || *fonts[0].EmbeddedPath != "Doc_0/Res/Font7.ttf" {
		t.Fatalf("fonts: %v, %v", fonts, err)
	}
	data, err := doc.FontData(0)
	if err != nil || len(data) != 244104 {
		t.Fatalf("font data: %d bytes, %v", len(data), err)
	}
	pageFonts, err := doc.PageFonts(0)
	if err != nil || len(pageFonts) == 0 || pageFonts[0].EmbeddedPath == nil || *pageFonts[0].EmbeddedPath != "Doc_0/Res/Font7.ttf" {
		t.Fatalf("page fonts: %v, %v", pageFonts, err)
	}
	pageData, err := doc.PageFontData(0, 0)
	if err != nil || !bytes.Equal(pageData, data) {
		t.Fatalf("page font data: %v", err)
	}
	if _, err := doc.PageFonts(-1); err == nil {
		t.Fatal("expected negative page index")
	}
	if _, err := doc.PageFontData(0, -1); err == nil {
		t.Fatal("expected negative page font index")
	}
	if _, err := doc.PageFontData(-1, 0); err == nil {
		t.Fatal("expected negative page index for font data")
	}
	if _, err := doc.PageFonts(999); err == nil {
		t.Fatal("expected out-of-range page index")
	}
	if _, err := doc.PageFontData(0, 999); err == nil {
		t.Fatal("expected out-of-range page font index")
	}
	if _, err := doc.FontData(-1); err == nil {
		t.Fatal("expected negative font index")
	}
	if _, err := doc.FontData(1); err == nil {
		t.Fatal("expected font index error")
	}
	doc.Close()
	if _, err := doc.Fonts(); err == nil {
		t.Fatal("expected closed document")
	}
	if _, err := doc.FontData(0); err == nil {
		t.Fatal("expected closed document")
	}
	if _, err := doc.PageFonts(0); err == nil {
		t.Fatal("expected closed document")
	}
	if _, err := doc.PageFontData(0, 0); err == nil {
		t.Fatal("expected closed document")
	}
}

func TestEmbeddedFontReferenceUsesUniqueArchiveCase(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "keyword.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	fonts, err := doc.Fonts()
	if err != nil || len(fonts) != 1 {
		t.Fatalf("fonts: %v, %v", fonts, err)
	}
	resourcePath := fonts[0].ResourcePath
	resource, err := doc.Entry(resourcePath)
	if err != nil {
		t.Fatal(err)
	}
	updated := bytes.Replace(resource, []byte("Font7.ttf"), []byte("font7.TTF"), 1)
	if bytes.Equal(updated, resource) {
		t.Fatal("fixture font reference was not found")
	}
	if err := doc.SetEntry(resourcePath, updated); err != nil {
		t.Fatal(err)
	}
	fonts, err = doc.Fonts()
	if err != nil || len(fonts) != 1 || fonts[0].EmbeddedPath == nil || *fonts[0].EmbeddedPath != "Doc_0/Res/Font7.ttf" {
		t.Fatalf("resolved fonts: %v, %v", fonts, err)
	}
	data, err := doc.FontData(0)
	if err != nil || len(data) != 244104 {
		t.Fatalf("font data: %d bytes, %v", len(data), err)
	}
}

func TestPageLocalFontMetadataAndBytes(t *testing.T) {
	fontPath := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-sans.ttf")
	font, err := os.ReadFile(fontPath)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("Hello", 30, 20); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	pagePath := pages[0].Path
	pageDir := pagePath[:strings.LastIndex(pagePath, "/")]
	if err := doc.SetEntry(pageDir+"/Res/local.ttf", font); err != nil {
		t.Fatal(err)
	}
	resource := `<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="Res"><ofd:Fonts><ofd:Font ID="1" FontName="DejaVu Sans"><ofd:FontFile>local.ttf</ofd:FontFile></ofd:Font></ofd:Fonts></ofd:Res>`
	if err := doc.SetEntry(pageDir+"/PageRes.xml", []byte(resource)); err != nil {
		t.Fatal(err)
	}
	page, err := doc.Entry(pagePath)
	if err != nil {
		t.Fatal(err)
	}
	page = bytes.Replace(page, []byte("<ofd:Content>"), []byte("<ofd:PageRes>PageRes.xml</ofd:PageRes><ofd:Content>"), 1)
	if err := doc.SetEntry(pagePath, page); err != nil {
		t.Fatal(err)
	}
	fonts, err := doc.PageFonts(0)
	if err != nil || len(fonts) == 0 || fonts[0].FontName != "DejaVu Sans" || fonts[0].ResourcePath != pageDir+"/PageRes.xml" {
		t.Fatalf("local page fonts: %v, %v", fonts, err)
	}
	family := "Page & Local"
	if err := doc.SetPageFontAttribute(0, 0, "FamilyName", &family); err != nil {
		t.Fatal(err)
	}
	fonts, err = doc.PageFonts(0)
	if err != nil || fonts[0].FamilyName == nil || *fonts[0].FamilyName != family {
		t.Fatalf("edited page font: %+v, %v", fonts, err)
	}
	documentFonts, err := doc.Fonts()
	if err != nil || documentFonts[0].FamilyName != nil && *documentFonts[0].FamilyName == family {
		t.Fatalf("public font changed: %+v, %v", documentFonts, err)
	}
	bad := "invalid"
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetPageFontAttribute(0, 0, "Charset", &bad); err == nil {
		t.Fatal("expected invalid charset")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed page font edit changed package: %v", err)
	}
	pageFont, err := doc.PageFontData(0, 0)
	if err != nil || !bytes.Equal(pageFont, font) {
		t.Fatalf("local page font data: %v", err)
	}
	replacement, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-serif.ttf"))
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.ReplacePageFontData(0, 0, replacement); err != nil {
		t.Fatal(err)
	}
	pageFont, err = doc.PageFontData(0, 0)
	if err != nil || !bytes.Equal(pageFont, replacement) {
		t.Fatalf("replaced page font data: %v", err)
	}
	before, err = doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.ReplacePageFontData(0, 0, []byte("invalid")); err == nil {
		t.Fatal("expected invalid replacement font")
	}
	if err := doc.ReplacePageFontData(0, 0, []byte{0, 1, 0, 0}); err == nil {
		t.Fatal("expected truncated replacement font")
	}
	after, err = doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed font replacement changed package: %v", err)
	}
	if _, err := doc.AddShapedTextObject(0, "fi", [4]float64{1, 1, 10, 5}, 3, "1"); err != nil {
		t.Fatalf("shape with page-local font: %v", err)
	}
	texts, err := doc.PageText(0)
	if err != nil || !slices.Contains(texts, "fi") {
		t.Fatalf("page-local shaped text: %v, %v", texts, err)
	}
}

func TestEmbedsFontOnlyInSelectedPage(t *testing.T) {
	font, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-sans.ttf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	for range 2 {
		if err := doc.AddBlankPage(40, 30); err != nil {
			t.Fatal(err)
		}
	}
	id, err := doc.AddEmbeddedFontToPage(0, "Page & Font", font)
	if err != nil {
		t.Fatal(err)
	}
	fonts, err := doc.PageFonts(0)
	if err != nil || len(fonts) == 0 || fonts[0].ID != strconv.Itoa(id) {
		t.Fatalf("page font: %+v, %v", fonts, err)
	}
	data, err := doc.PageFontData(0, 0)
	if err != nil || !bytes.Equal(data, font) {
		t.Fatalf("page font data: %v", err)
	}
	other, err := doc.PageFonts(1)
	if err != nil {
		t.Fatal(err)
	}
	for _, item := range other {
		if item.ID == strconv.Itoa(id) {
			t.Fatal("page font leaked to another page")
		}
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddEmbeddedFontToPage(0, "bad", []byte("invalid")); err == nil {
		t.Fatal("expected invalid font")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed edit changed package: %v", err)
	}
	reopened, err := FromBytes(before)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	data, err = reopened.PageFontData(0, 0)
	if err != nil || !bytes.Equal(data, font) {
		t.Fatalf("reopened page font data: %v", err)
	}
}

func TestWritesEmbeddedFont(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "keyword.ofd")
	source, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	data, err := source.FontData(0)
	source.Close()
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if _, err := doc.AddEmbeddedFont("bad", []byte("bad")); err == nil {
		t.Fatal("expected invalid font format")
	}
	id, err := doc.AddEmbeddedFont("宋体", data)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.AddTextPageWithFont("bad", -1, 210, 297); err == nil {
		t.Fatal("expected negative font ID")
	}
	if err := doc.AddTextPageWithFont("字体测试", id, 210, 297); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || objects[0].FontID == nil || *objects[0].FontID != "2" {
		t.Fatalf("text objects: %v, %v", objects, err)
	}
	fontData, err := doc.FontData(1)
	if err != nil || len(fontData) != len(data) {
		t.Fatalf("font data: %d, %v", len(fontData), err)
	}
	doc.Close()
	if _, err := doc.AddEmbeddedFont("宋体", data); err == nil {
		t.Fatal("expected closed document")
	}
	if err := doc.AddTextPageWithFont("bad", id, 210, 297); err == nil {
		t.Fatal("expected closed document")
	}
}

func TestStandaloneSVGEmbedsFontBytes(t *testing.T) {
	font, err := os.ReadFile(filepath.Join(
		"..",
		"..",
		"..",
		"crates",
		"ofdrw",
		"tests",
		"fixtures",
		"dejavu-serif.ttf",
	))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	fontID, err := doc.AddEmbeddedFont("serif, fallback", font)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.AddTextPageWithFont(
		"standalone SVG",
		fontID,
		60,
		30,
	); err != nil {
		t.Fatal(err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(svg, "@font-face") ||
		!strings.Contains(svg, "data:font/ttf;base64,") ||
		!strings.Contains(svg, `font-family="ofd-embedded-font-0"`) ||
		strings.Contains(svg, `font-family="serif, fallback"`) {
		t.Fatal("standalone SVG is missing its embedded font")
	}
}

func TestEditsFontSubstitutionAttributes(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-serif.ttf"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	id, err := doc.AddEmbeddedFont("DejaVu Serif", data)
	if err != nil {
		t.Fatal(err)
	}
	fonts, err := doc.Fonts()
	if err != nil {
		t.Fatal(err)
	}
	index := -1
	for i, font := range fonts {
		if font.ID == strconv.Itoa(id) {
			index = i
			break
		}
	}
	if index < 0 {
		t.Fatal("font missing")
	}
	family, charset, bold := "A & B", "SHIFT_JIS", "true"
	for _, update := range []struct {
		name  string
		value *string
	}{{"FamilyName", &family}, {"Charset", &charset}, {"Bold", &bold}} {
		if err := doc.SetFontAttribute(index, update.name, update.value); err != nil {
			t.Fatal(err)
		}
	}
	fonts, err = doc.Fonts()
	if err != nil || fonts[index].FamilyName == nil || *fonts[index].FamilyName != family || fonts[index].Attributes["Charset"] != "shift-jis" {
		t.Fatalf("font metadata: %+v, %v", fonts, err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	bad := "invalid"
	if err := doc.SetFontAttribute(index, "Charset", &bad); err == nil {
		t.Fatal("expected invalid charset")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed edit changed package: %v", err)
	}
	if err := doc.SetFontAttribute(index, "Bold", nil); err != nil {
		t.Fatal(err)
	}
	replacement, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-sans.ttf"))
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.ReplaceFontData(index, replacement); err != nil {
		t.Fatal(err)
	}
	if err := doc.ReplaceFontData(index, []byte("invalid")); err == nil {
		t.Fatal("expected invalid replacement font")
	}
	if err := doc.ReplaceFontData(index, []byte{0, 1, 0, 0}); err == nil {
		t.Fatal("expected truncated replacement font")
	}
	after, err = doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	reopened, err := FromBytes(after)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	fonts, err = reopened.Fonts()
	if err != nil || fonts[index].Attributes["Bold"] != "" {
		t.Fatalf("reopened font: %+v, %v", fonts, err)
	}
	fontData, err := reopened.FontData(index)
	if err != nil || !bytes.Equal(fontData, replacement) {
		t.Fatalf("reopened font data: %v", err)
	}
}

func TestEmbedsFontInExistingDocument(t *testing.T) {
	fixtureDir := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources")
	source, err := Open(filepath.Join(fixtureDir, "keyword.ofd"))
	if err != nil {
		t.Fatal(err)
	}
	data, err := source.FontData(0)
	source.Close()
	if err != nil {
		t.Fatal(err)
	}
	doc, err := Open(filepath.Join(fixtureDir, "helloworld.ofd"))
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	for _, test := range []struct {
		name  string
		index int
		font  string
		data  []byte
	}{
		{"negative index", -1, "font", data},
		{"bad format", 0, "font", []byte("bad")},
		{"invalid name", 0, "bad\x00name", data},
	} {
		t.Run(test.name, func(t *testing.T) {
			if _, err := doc.AddEmbeddedFontToDocument(test.index, test.font, test.data); err == nil {
				t.Fatal("expected error")
			}
		})
	}
	id, err := doc.AddEmbeddedFontToDocument(0, "Rust 字体", data)
	if err != nil {
		t.Fatal(err)
	}
	fonts, err := doc.Fonts()
	if err != nil || len(fonts) < 2 || fonts[len(fonts)-1].ID != fmt.Sprint(id) {
		t.Fatalf("fonts: %v, %v", fonts, err)
	}
	fontData, err := doc.FontData(len(fonts) - 1)
	if err != nil || !bytes.Equal(fontData, data) {
		t.Fatalf("font data: %d, %v", len(fontData), err)
	}
	for _, test := range []struct {
		name  string
		index int
		font  int
	}{
		{"negative index", -1, id},
		{"negative font", 0, -1},
		{"undefined font", 0, 999},
	} {
		t.Run(test.name, func(t *testing.T) {
			if err := doc.AppendTextPageWithFont(test.index, "bad", test.font, 80, 60); err == nil {
				t.Fatal("expected error")
			}
		})
	}
	if err := doc.AppendTextPageWithFont(0, "selected font", id, 80, 60); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(1)
	if err != nil || len(objects) != 1 || objects[0].FontID == nil || *objects[0].FontID != fmt.Sprint(id) {
		t.Fatalf("text objects: %v, %v", objects, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
	doc.Close()
	if _, err := doc.AddEmbeddedFontToDocument(0, "font", data); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestCreatesAttachment(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddAttachment("note.txt", []byte("hello")); err != nil {
		t.Fatal(err)
	}
	format := "text/plain"
	created := "2024-02-29T12:30:59"
	modified := "2024-03-01T08:00:00"
	usage := "source & <record>"
	visible := false
	if err := doc.SetAttachmentMetadata(0, AttachmentMetadata{
		Format: &format, CreationDate: &created, ModDate: &modified,
		Visible: &visible, Usage: &usage,
	}); err != nil {
		t.Fatal(err)
	}
	invalid := "2023-02-29T12:30:59"
	if err := doc.SetAttachmentMetadata(0, AttachmentMetadata{CreationDate: &invalid}); err == nil {
		t.Fatal("expected invalid attachment date")
	}
	items, err := doc.Attachments()
	if err != nil || len(items) != 1 || items[0].Name != "note.txt" || items[0].Format == nil || *items[0].Format != format || items[0].CreationDate == nil || *items[0].CreationDate != created || items[0].ModDate == nil || *items[0].ModDate != modified || items[0].Visible || items[0].Usage != usage {
		t.Fatalf("attachments: %v, %v", items, err)
	}
	oldPath := items[0].Path
	if err := doc.ReplaceAttachmentData(0, []byte("updated")); err != nil {
		t.Fatal(err)
	}
	if err := doc.ReplaceAttachmentData(1, []byte("bad")); err == nil {
		t.Fatal("expected attachment index error")
	}
	retained, err := doc.Entry(oldPath)
	if err != nil || string(retained) != "hello" {
		t.Fatalf("retained attachment: %q, %v", retained, err)
	}
	if err := doc.RenameAttachment(0, " "); err == nil {
		t.Fatal("expected empty attachment name rejection")
	}
	if err := doc.RenameAttachment(0, "新名称 & <附件>.txt"); err != nil {
		t.Fatal(err)
	}
	renamed, err := doc.Attachments()
	if err != nil || renamed[0].Name != "新名称 & <附件>.txt" || renamed[0].ID != items[0].ID {
		t.Fatalf("renamed attachment: %v, %v", renamed, err)
	}
	data, err := doc.AttachmentData(0)
	if err != nil || string(data) != "updated" {
		t.Fatalf("data: %q, %v", data, err)
	}
	if err := doc.RemoveAttachment(-1); err == nil {
		t.Fatal("expected negative attachment index error")
	}
	if err := doc.RemoveAttachment(0); err != nil {
		t.Fatal(err)
	}
	items, err = doc.Attachments()
	if err != nil || len(items) != 0 {
		t.Fatalf("attachments after removal: %v, %v", items, err)
	}
	if err := doc.AddAttachment("", nil); err == nil {
		t.Fatal("expected empty-name error")
	}
	doc.Close()
	if err := doc.AddAttachment("x", nil); err == nil {
		t.Fatal("expected closed error")
	}
}

func TestAddsAttachmentToExistingDocument(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "AddAttachment.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	for _, test := range []struct {
		name           string
		index          int
		attachmentName string
	}{
		{"negative index", -1, "note"},
		{"empty name", 0, ""},
		{"NUL name", 0, "bad\x00name"},
	} {
		t.Run(test.name, func(t *testing.T) {
			if err := doc.AddAttachmentToDocument(test.index, test.attachmentName, []byte("data")); err == nil {
				t.Fatal("expected invalid attachment error")
			}
		})
	}
	if err := doc.AddAttachmentToDocument(0, "Rust note", []byte("new data")); err != nil {
		t.Fatal(err)
	}
	items, err := doc.Attachments()
	if err != nil || len(items) != 2 || items[0].Name != "Gao" || items[1].Name != "Rust note" {
		t.Fatalf("attachments: %v, %v", items, err)
	}
	data, err := doc.AttachmentData(1)
	if err != nil || string(data) != "new data" {
		t.Fatalf("data: %q, %v", data, err)
	}
	if err := doc.RemoveAttachment(1); err != nil {
		t.Fatal(err)
	}
	items, err = doc.Attachments()
	if err != nil || len(items) != 1 || items[0].Name != "Gao" {
		t.Fatalf("attachments after removal: %v, %v", items, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
}

func TestAttachmentReferencesUseUniqueArchiveCase(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "AddAttachment.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	const root = "Doc_0/Document.xml"
	documentXML, err := doc.Entry(root)
	if err != nil {
		t.Fatal(err)
	}
	documentXML = bytes.Replace(documentXML, []byte("Attachments.xml"), []byte("ATTACHMENTS.XML"), 1)
	if err := doc.SetEntry(root, documentXML); err != nil {
		t.Fatal(err)
	}
	const listPath = "Doc_0/Attachments.xml"
	listXML, err := doc.Entry(listPath)
	if err != nil {
		t.Fatal(err)
	}
	listXML = bytes.Replace(listXML, []byte("/Doc_0/Res/eg_tulip.jpg"), []byte("/DOC_0/RES/EG_TULIP.JPG"), 1)
	if err := doc.SetEntry(listPath, listXML); err != nil {
		t.Fatal(err)
	}
	attachments, err := doc.Attachments()
	if err != nil || len(attachments) != 1 || attachments[0].Path != "Doc_0/Res/eg_tulip.jpg" {
		t.Fatalf("attachments: %v, %v", attachments, err)
	}
	data, err := doc.AttachmentData(0)
	if err != nil || len(data) != 36494 {
		t.Fatalf("attachment data: %d bytes, %v", len(data), err)
	}
	if err := doc.RenameAttachment(0, "renamed"); err != nil {
		t.Fatal(err)
	}
	attachments, err = doc.Attachments()
	if err != nil || attachments[0].Name != "renamed" {
		t.Fatalf("renamed attachment: %v, %v", attachments, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %v, %v", issues, err)
	}
}

func TestReportsMissingSignedResourceAcrossBinding(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "missing-signed-resource.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	checks, err := doc.CheckSignatureReferences(0)
	if err != nil || len(checks) != 5 {
		t.Fatalf("reference checks: %v, %v", checks, err)
	}
	missing := 0
	for _, check := range checks {
		if check.Path == "Doc_0/PublicRes.xml" {
			missing++
			if check.Valid || check.Actual != "" || check.Expected == "" {
				t.Fatalf("missing reference: %+v", check)
			}
		} else if !check.Valid {
			t.Fatalf("unrelated invalid reference: %+v", check)
		}
	}
	if missing != 1 {
		t.Fatalf("missing reference count: %d", missing)
	}
	issues, err := doc.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	if !slices.ContainsFunc(issues, func(issue ValidationIssue) bool {
		return issue.Code == "signature_reference" && strings.Contains(issue.Message, "Doc_0/PublicRes.xml")
	}) {
		t.Fatalf("missing signature reference issue: %+v", issues)
	}
}

func TestReadsSignatureDescriptionAndBytes(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "SESV4SignDoc.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	signatures, err := doc.Signatures()
	if err != nil || len(signatures) != 1 || signatures[0].CheckMethod == nil || *signatures[0].CheckMethod != "SM3" {
		t.Fatalf("signatures: %v, %v", signatures, err)
	}
	if len(signatures[0].References) != 5 || signatures[0].References[0].Path != "OFD.xml" {
		t.Fatalf("references: %v", signatures[0].References)
	}
	if len(signatures[0].Stamps) != 1 || len(signatures[0].Stamps[0].Boundary) != 4 {
		t.Fatalf("stamps: %v", signatures[0].Stamps)
	}
	checks, err := doc.CheckSignatureReferences(0)
	if err != nil || len(checks) != 5 {
		t.Fatalf("reference checks: %v, %v", checks, err)
	}
	for _, check := range checks {
		if !check.Valid {
			t.Fatalf("invalid reference: %v", check)
		}
	}
	description, err := doc.CheckSignatureDescriptionHash(0)
	if err != nil || !description.Valid || description.Expected != description.Actual {
		t.Fatalf("description hash: %v, %v", description, err)
	}
	valueValid, err := doc.CheckSESSignatureValue(0)
	if err != nil || !valueValid {
		t.Fatalf("SES signature value: %v, %v", valueValid, err)
	}
	signedValue, err := doc.SignedValueData(0)
	if err != nil || len(signedValue) != 17726 {
		t.Fatalf("signed value: %d, %v", len(signedValue), err)
	}
	seal, err := doc.SealData(0)
	if err != nil || len(seal) != 17011 {
		t.Fatalf("seal: %d, %v", len(seal), err)
	}
	matched, err := doc.CheckSignatureSealMatch(0)
	if err != nil || matched == nil || !*matched {
		t.Fatalf("detached seal match: %v, %v", matched, err)
	}
	sealValid, err := doc.CheckSESSealSignature(0)
	if err != nil || !sealValid {
		t.Fatalf("electronic seal signature: %v, %v", sealValid, err)
	}
	sealCertValid, err := doc.CheckSESSealCertificateValidity(0, 1_600_000_000)
	if err != nil || !sealCertValid {
		t.Fatalf("seal issuer certificate validity: %v, %v", sealCertValid, err)
	}
	sealCertValid, err = doc.CheckSESSealCertificateValidity(0, 0)
	if err != nil || sealCertValid {
		t.Fatalf("seal issuer certificate before validity: %v, %v", sealCertValid, err)
	}
	authorized, err := doc.CheckSESSealSignerAuthorization(0)
	if err != nil || !authorized {
		t.Fatalf("seal signer authorization: %v, %v", authorized, err)
	}
	sealValidAtTime, err := doc.CheckSESSealValidity(0, 1_600_000_000)
	if err != nil || !sealValidAtTime {
		t.Fatalf("seal validity: %v, %v", sealValidAtTime, err)
	}
	sealValidAtTime, err = doc.CheckSESSealValidity(0, 0)
	if err != nil || sealValidAtTime {
		t.Fatalf("seal validity before creation: %v, %v", sealValidAtTime, err)
	}
	rootCert, err := os.ReadFile(filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "root-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	sealIssuerTrusted, err := doc.CheckSESSealIssuerOrderedSM2Chain(0, nil, rootCert, 1_600_000_000, "")
	if err != nil || sealIssuerTrusted {
		t.Fatalf("seal issuer with missing keyCertSign: %v, %v", sealIssuerTrusted, err)
	}
	trustReport, err := doc.VerifySignatureWithOrderedSM2Chains(0, SignatureTrustOptions{
		SignerAnchorDER: rootCert, SealAnchorDER: rootCert,
		AtUnixSeconds:         1_600_000_000,
		SignerSignatureDistID: defaultSM2DistID, SealSignatureDistID: defaultSM2DistID,
	})
	if err != nil || trustReport.AllValid || trustReport.SealIssuerChainValid == nil || *trustReport.SealIssuerChainValid || trustReport.Signature.SealSignerAuthorized == nil || !*trustReport.Signature.SealSignerAuthorized || trustReport.Signature.SealValidityValid == nil || !*trustReport.Signature.SealValidityValid {
		t.Fatalf("untrusted seal issuer report: %+v, %v", trustReport, err)
	}
	sealValid, err = doc.CheckSESSealSignatureWithDistID(0, "different-id")
	if err != nil || sealValid {
		t.Fatalf("electronic seal with wrong ID: %v, %v", sealValid, err)
	}
	if signatures[0].SealPath == nil {
		t.Fatal("expected detached seal path")
	}
	seal[12] ^= 1
	if err := doc.SetEntry(*signatures[0].SealPath, seal); err != nil {
		t.Fatal(err)
	}
	matched, err = doc.CheckSignatureSealMatch(0)
	if err != nil || matched == nil || *matched {
		t.Fatalf("modified detached seal match: %v, %v", matched, err)
	}
	if _, err := doc.SignedValueData(-1); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.SealData(1); err == nil {
		t.Fatal("expected out-of-range error")
	}
	if _, err := doc.CheckSignatureReferences(-1); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.CheckSignatureSealMatch(-1); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.CheckSESSealSignature(-1); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.CheckSESSealCertificateValidity(-1, 1_600_000_000); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.CheckSESSealSignerAuthorization(-1); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.CheckSESSealValidity(-1, 1_800_000_000); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.CheckSESSealIssuerOrderedSM2Chain(-1, nil, rootCert, 1_600_000_000, ""); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.SignedValueData(1); err == nil {
		t.Fatal("expected out-of-range error")
	}
	resourcePath := "Doc_0/PublicRes.xml"
	resource, err := doc.Entry(resourcePath)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetEntry(resourcePath, append(resource, '\n')); err != nil {
		t.Fatal(err)
	}
	checks, err = doc.CheckSignatureReferences(0)
	if err != nil || checks[1].Valid {
		t.Fatalf("modified reference: %v, %v", checks, err)
	}
	doc.Close()
	if _, err := doc.Signatures(); err == nil {
		t.Fatal("expected closed document error")
	}
	if _, err := doc.SignedValueData(0); err == nil {
		t.Fatal("expected closed document error")
	}
	if _, err := doc.SealData(0); err == nil {
		t.Fatal("expected closed document error")
	}
	if _, err := doc.CheckSignatureReferences(0); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestSignatureReferenceAcceptsXMLBase64Whitespace(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "SESV4SignDoc.ofd")
	doc, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	signatures, err := doc.Signatures()
	if err != nil || len(signatures) != 1 {
		t.Fatalf("signatures: %v, %v", signatures, err)
	}
	value := signatures[0].References[0].CheckValue
	wrapped := value[:20] + "\n\t" + value[20:]
	source, err := doc.Entry(signatures[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(source, []byte(value)) {
		t.Fatal("signature reference value is missing")
	}
	if err := doc.SetEntry(signatures[0].Path, bytes.Replace(source, []byte(value), []byte(wrapped), 1)); err != nil {
		t.Fatal(err)
	}
	checks, err := doc.CheckSignatureReferences(0)
	if err != nil || len(checks) != 5 {
		t.Fatalf("reference checks: %v, %v", checks, err)
	}
	for _, check := range checks {
		if !check.Valid {
			t.Fatalf("invalid reference: %+v", check)
		}
	}
	if checks[0].Expected != wrapped {
		t.Fatalf("expected digest text changed: %q", checks[0].Expected)
	}
}

func TestChecksRawDigitalSignatureWithCertificate(t *testing.T) {
	root := filepath.Join("..", "..", "..", "..")
	certificate, err := os.ReadFile(filepath.Join(root, "ofdrw-rs", "crates", "ofdrw", "tests", "fixtures", "user-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := Open(filepath.Join(root, "ofdrw-sign", "src", "test", "resources", "DigitalSign.ofd"))
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	valid, err := doc.CheckDigitalSignatureWithCertificate(0, certificate)
	if err != nil || !valid {
		t.Fatalf("digital signature: %v, %v", valid, err)
	}
	if _, err := doc.CheckDigitalSignatureWithCertificate(0, []byte("invalid")); err == nil {
		t.Fatal("expected invalid certificate error")
	}
	if _, err := doc.CheckDigitalSignatureWithCertificate(-1, certificate); err == nil {
		t.Fatal("expected negative index error")
	}
}

func TestChecksSignerCertificateValidityAtSuppliedTime(t *testing.T) {
	root := filepath.Join("..", "..", "..", "..")
	certificate, err := os.ReadFile(filepath.Join(root, "ofdrw-rs", "crates", "ofdrw", "tests", "fixtures", "user-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := Open(filepath.Join(root, "ofdrw-sign", "src", "test", "resources", "DigitalSign.ofd"))
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	valid, err := doc.CheckSignatureCertificateValidity(0, 1_600_000_000, certificate)
	if err != nil || !valid {
		t.Fatalf("certificate should be valid: %v, %v", valid, err)
	}
	valid, err = doc.CheckSignatureCertificateValidity(0, 1_700_000_000, certificate)
	if err != nil || valid {
		t.Fatalf("certificate should be expired: %v, %v", valid, err)
	}
	if _, err := doc.CheckSignatureCertificateValidity(0, 1_600_000_000, nil); err == nil {
		t.Fatal("expected missing certificate error")
	}
	if _, err := doc.CheckSignatureCertificateValidity(-1, 1_600_000_000, certificate); err == nil {
		t.Fatal("expected negative index error")
	}
}

func TestCreatesRawSM2SignatureAndVerifiesIt(t *testing.T) {
	root := filepath.Join("..", "..", "..", "..")
	fixtures := filepath.Join(root, "ofdrw-rs", "crates", "ofdrw", "tests", "fixtures")
	key, err := os.ReadFile(filepath.Join(fixtures, "user-private-key.bin"))
	if err != nil {
		t.Fatal(err)
	}
	certificate, err := os.ReadFile(filepath.Join(fixtures, "user-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("signed", 210, 297); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddDigitalSignature(nil, "2020-04-22 16:22:44"); err == nil {
		t.Fatal("expected invalid private key error")
	}
	if _, err := doc.AddDigitalSignature(key, "2020-04-22 16:22:44\x00ignored"); err == nil {
		t.Fatal("expected NUL date error")
	}
	index, err := doc.AddDigitalSignature(key, "2020-04-22 16:22:44")
	if err != nil || index != 0 {
		t.Fatalf("add signature: %d, %v", index, err)
	}
	index, err = doc.AddDigitalSignature(key, "2020-04-23 10:11:12")
	if err != nil || index != 1 {
		t.Fatalf("append signature: %d, %v", index, err)
	}
	for index := 0; index < 2; index++ {
		valid, err := doc.CheckDigitalSignatureWithCertificate(index, certificate)
		if err != nil || !valid {
			t.Fatalf("verify signature %d: %v, %v", index, valid, err)
		}
		checks, err := doc.CheckSignatureReferences(index)
		if err != nil {
			t.Fatal(err)
		}
		for _, check := range checks {
			if !check.Valid {
				t.Fatalf("invalid reference: %s", check.Path)
			}
		}
	}
	path := filepath.Join(t.TempDir(), "signed.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	valid, err := reopened.CheckDigitalSignatureWithCertificate(1, certificate)
	if err != nil || !valid {
		t.Fatalf("verify reopened signature: %v, %v", valid, err)
	}
	doc.Close()
	if _, err := doc.AddDigitalSignature(key, "2020-04-22 16:22:44"); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestCreatesSESSignatureAndAppendsRawSignature(t *testing.T) {
	root := filepath.Join("..", "..", "..", "..")
	fixtures := filepath.Join(root, "ofdrw-rs", "crates", "ofdrw", "tests", "fixtures")
	key, err := os.ReadFile(filepath.Join(fixtures, "user-private-key.bin"))
	if err != nil {
		t.Fatal(err)
	}
	certificate, err := os.ReadFile(filepath.Join(fixtures, "user-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	seal, err := os.ReadFile(filepath.Join(root, "ofdrw-gm", "src", "test", "resources", "UserV4.esl"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("signed", 210, 297); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddSESSignatureAtPage(key, certificate, seal, "2020-09-13 12:26:40", -1, [4]float64{10, 20, 40, 40}); err == nil {
		t.Fatal("expected invalid page index")
	}
	index, err := doc.AddSESSignatureAtPage(key, certificate, seal, "2020-09-13 12:26:40", 0, [4]float64{10, 20, 40, 40})
	if err != nil || index != 0 {
		t.Fatalf("add SES signature: %d, %v", index, err)
	}
	signatures, err := doc.Signatures()
	if err != nil || len(signatures) != 1 || len(signatures[0].Stamps) != 1 || signatures[0].Stamps[0].ID != "s002" {
		t.Fatalf("positioned SES stamp: %v, %v", signatures, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `data-ofd-stamp="s002"`) {
		t.Fatalf("render SES stamp: %v", err)
	}
	valid, err := doc.CheckSESSignatureValue(0)
	if err != nil || !valid {
		t.Fatalf("verify SES value: %v, %v", valid, err)
	}
	match, err := doc.CheckSignatureSealMatch(0)
	if err != nil || match == nil || !*match {
		t.Fatalf("verify seal match: %v, %v", match, err)
	}
	valid, err = doc.CheckSESSealSignerAuthorization(0)
	if err != nil || !valid {
		t.Fatalf("verify seal signer: %v, %v", valid, err)
	}
	index, err = doc.AddDigitalSignature(key, "2020-09-13 12:26:40")
	if err != nil || index != 1 {
		t.Fatalf("append raw signature: %d, %v", index, err)
	}
	path := filepath.Join(t.TempDir(), "ses.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	valid, err = reopened.CheckSESSignatureValue(0)
	if err != nil || !valid {
		t.Fatalf("verify reopened SES value: %v, %v", valid, err)
	}
	valid, err = reopened.CheckDigitalSignatureWithCertificate(1, certificate)
	if err != nil || !valid {
		t.Fatalf("verify appended raw signature: %v, %v", valid, err)
	}
}

func TestCreatesSESSealAndSignsDocument(t *testing.T) {
	root := filepath.Join("..", "..", "..", "..")
	fixtures := filepath.Join(root, "ofdrw-rs", "crates", "ofdrw", "tests", "fixtures")
	key, err := os.ReadFile(filepath.Join(fixtures, "user-private-key.bin"))
	if err != nil {
		t.Fatal(err)
	}
	certificate, err := os.ReadFile(filepath.Join(fixtures, "user-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	picture, err := os.ReadFile(filepath.Join(root, "ofdrw-layout", "src", "test", "resources", "testimg.png"))
	if err != nil {
		t.Fatal(err)
	}
	spec := SESSealSpec{
		Version: 4, VendorID: "ofdrw-rs", SealID: "go-seal", Name: "Go 印章", SealType: 3,
		AuthorizedSignerCertificates: [][]byte{certificate},
		CreatedAt:                    1_600_000_000, ValidFrom: 1_600_000_000, ValidUntil: 1_900_000_000,
		PictureType: "PNG", PictureWidthMM: 40, PictureHeightMM: 40,
	}
	seal, err := CreateSESSeal(key, certificate, picture, spec)
	if err != nil || len(seal) == 0 || seal[0] != 0x30 {
		t.Fatalf("create SES seal: %v", err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("signed", 210, 297); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddSESSignature(key, certificate, seal, "2020-09-13 12:26:40"); err != nil {
		t.Fatal(err)
	}
	valid, err := doc.CheckSESSealSignature(0)
	if err != nil || !valid {
		t.Fatalf("verify generated seal: %v, %v", valid, err)
	}
	valid, err = doc.CheckSESSignatureValue(0)
	if err != nil || !valid {
		t.Fatalf("verify generated signature: %v, %v", valid, err)
	}
}

func TestVerifiesSESSignerAndSealIssuerCertificatePools(t *testing.T) {
	fixtures := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	read := func(name string) []byte {
		t.Helper()
		data, err := os.ReadFile(filepath.Join(fixtures, name))
		if err != nil {
			t.Fatal(err)
		}
		return data
	}
	key := read("chain-leaf-private-key.bin")
	certificate := read("chain-leaf-cert.der")
	intermediate := read("chain-intermediate-cert.der")
	anchor := read("chain-root-cert.der")
	unrelated := read("root-cert.der")
	picture, err := os.ReadFile(filepath.Join("..", "..", "..", "..", "ofdrw-layout", "src", "test", "resources", "testimg.png"))
	if err != nil {
		t.Fatal(err)
	}
	seal, err := CreateSESSeal(key, certificate, picture, SESSealSpec{
		Version: 4, VendorID: "ofdrw-rs", SealID: "go-chain-seal", Name: "Go chain seal", SealType: 3,
		AuthorizedSignerCertificates: [][]byte{certificate},
		CreatedAt:                    1_893_456_000, ValidFrom: 1_893_456_000, ValidUntil: 1_950_000_000,
		PictureType: "PNG", PictureWidthMM: 20, PictureHeightMM: 20,
	})
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("trusted SES", 210, 297); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddSESSignature(key, certificate, seal, "2030-01-01 00:00:00"); err != nil {
		t.Fatal(err)
	}
	options := SignatureTrustOptions{
		SignerIntermediatesDER: [][]byte{unrelated, intermediate}, SignerAnchorDER: anchor,
		SealIntermediatesDER: [][]byte{unrelated, intermediate}, SealAnchorDER: anchor,
		AtUnixSeconds:         1_893_456_000,
		SignerSignatureDistID: defaultSM2DistID, SealSignatureDistID: defaultSM2DistID,
	}
	report, err := doc.VerifySignatureWithCertificatePools(0, options)
	if err != nil || !report.AllValid || report.SealIssuerChainValid == nil || !*report.SealIssuerChainValid {
		t.Fatalf("valid SES trust report: %+v, %v", report, err)
	}
	options.SealIntermediatesDER = [][]byte{unrelated}
	report, err = doc.VerifySignatureWithCertificatePools(0, options)
	if err != nil || !report.Signature.AllValid || report.SealIssuerChainValid == nil || *report.SealIssuerChainValid || report.AllValid {
		t.Fatalf("missing seal issuer intermediate: %+v, %v", report, err)
	}
}

func TestRidingStampClipIsExposedAndRendered(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-converter", "src", "test", "resources", "V4RideRight.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	signatures, err := doc.Signatures()
	if err != nil || len(signatures) != 1 || len(signatures[0].Stamps) != 5 {
		t.Fatalf("riding stamp metadata: %v, %v", signatures, err)
	}
	if !slices.Equal(signatures[0].Stamps[0].Clip, []float64{0, 0, 8, 40}) {
		t.Fatalf("first riding stamp clip: %v", signatures[0].Stamps[0].Clip)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `clip-path="url(#ofd-stamp-clip-`) {
		t.Fatalf("riding stamp preview: %v", err)
	}
}

func TestChecksDirectSM2TrustAnchor(t *testing.T) {
	fixtures := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	key, err := os.ReadFile(filepath.Join(fixtures, "trust-leaf-private-key.bin"))
	if err != nil {
		t.Fatal(err)
	}
	signer, err := os.ReadFile(filepath.Join(fixtures, "trust-leaf-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	anchor, err := os.ReadFile(filepath.Join(fixtures, "trust-root-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if _, err := doc.AddDigitalSignature(key, "2030-01-01 00:00:00"); err != nil {
		t.Fatal(err)
	}
	valid, err := doc.CheckSignatureDirectTrustAnchor(0, signer, anchor, 1_893_456_000, "")
	if err != nil || !valid {
		t.Fatalf("valid trust anchor: %v, %v", valid, err)
	}
	valid, err = doc.CheckSignatureDirectTrustAnchor(0, signer, anchor, 1_893_456_000, "1234567812345678")
	if err != nil || valid {
		t.Fatalf("wrong SM2 identifier: %v, %v", valid, err)
	}
	if _, err := doc.CheckSignatureDirectTrustAnchor(0, signer, []byte("invalid"), 1_893_456_000, ""); err == nil {
		t.Fatal("expected malformed anchor error")
	}
	if _, err := doc.CheckSignatureDirectTrustAnchor(-1, signer, anchor, 1_893_456_000, ""); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.CheckSignatureDirectTrustAnchor(0, signer, anchor, 1_893_456_000, "bad\x00id"); err == nil {
		t.Fatal("expected NUL identifier error")
	}
}

func TestReportsCompleteSignatureVerification(t *testing.T) {
	fixtures := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	key, err := os.ReadFile(filepath.Join(fixtures, "trust-leaf-private-key.bin"))
	if err != nil {
		t.Fatal(err)
	}
	signer, err := os.ReadFile(filepath.Join(fixtures, "trust-leaf-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	anchor, err := os.ReadFile(filepath.Join(fixtures, "trust-root-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("protected", 210, 297); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddDigitalSignature(key, "2030-01-01 00:00:00"); err != nil {
		t.Fatal(err)
	}
	report, err := doc.VerifySignatureWithDirectTrustAnchor(0, signer, anchor, 1_893_456_000, "")
	if err != nil || !report.AllValid || !report.ReferencesValid || report.DescriptionHash != nil {
		t.Fatalf("valid signature report: %+v, %v", report, err)
	}
	path := "Doc_0/Pages/Page_0/Content.xml"
	page, err := doc.Entry(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetEntry(path, []byte(strings.Replace(string(page), "protected", "changed", 1))); err != nil {
		t.Fatal(err)
	}
	report, err = doc.VerifySignatureWithDirectTrustAnchor(0, signer, anchor, 1_893_456_000, "")
	if err != nil || report.AllValid || report.ReferencesValid || !report.SignatureValueValid || !report.DirectTrustAnchorValid {
		t.Fatalf("tampered file report: %+v, %v", report, err)
	}
	if _, err := doc.VerifySignatureWithDirectTrustAnchor(0, signer, []byte("bad"), 1_893_456_000, ""); err == nil {
		t.Fatal("expected invalid anchor error")
	}
	if _, err := doc.VerifySignatureWithDirectTrustAnchor(-1, signer, anchor, 1_893_456_000, ""); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.VerifySignatureWithDirectTrustAnchor(0, signer, anchor, 1_893_456_000, "bad\x00id"); err == nil {
		t.Fatal("expected NUL identifier error")
	}
}

func TestReportsOrderedSM2ChainVerification(t *testing.T) {
	fixtures := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	key, err := os.ReadFile(filepath.Join(fixtures, "chain-leaf-private-key.bin"))
	if err != nil {
		t.Fatal(err)
	}
	signer, err := os.ReadFile(filepath.Join(fixtures, "chain-leaf-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	intermediate, err := os.ReadFile(filepath.Join(fixtures, "chain-intermediate-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	anchor, err := os.ReadFile(filepath.Join(fixtures, "chain-root-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("chained", 210, 297); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddDigitalSignature(key, "2030-01-01 00:00:00"); err != nil {
		t.Fatal(err)
	}
	report, err := doc.VerifySignatureWithOrderedSM2Chain(0, signer, [][]byte{intermediate}, anchor, 1_893_456_000, "")
	if err != nil || !report.AllValid || !report.CertificateChainValid || !report.ReferencesValid {
		t.Fatalf("valid chain report: %+v, %v", report, err)
	}
	unrelated, err := os.ReadFile(filepath.Join(fixtures, "root-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	pooled, err := doc.VerifySignatureWithCertificatePool(0, signer, [][]byte{unrelated, intermediate}, anchor, 1_893_456_000, "", defaultSM2DistID)
	if err != nil || !pooled.AllValid {
		t.Fatalf("valid candidate pool: %+v, %v", pooled, err)
	}
	missing, err := doc.VerifySignatureWithCertificatePool(0, signer, [][]byte{unrelated}, anchor, 1_893_456_000, "", defaultSM2DistID)
	if err != nil || !missing.SignatureValueValid || missing.CertificateChainValid || missing.AllValid {
		t.Fatalf("missing intermediate candidate: %+v, %v", missing, err)
	}
	trustReport, err := doc.VerifySignatureWithOrderedSM2Chains(0, SignatureTrustOptions{
		SignerCertificateDER: signer, SignerIntermediatesDER: [][]byte{intermediate},
		SignerAnchorDER: anchor, AtUnixSeconds: 1_893_456_000,
		SignerSignatureDistID: defaultSM2DistID, SealSignatureDistID: defaultSM2DistID,
	})
	if err != nil || !trustReport.AllValid || trustReport.SealIssuerChainValid != nil || !trustReport.Signature.AllValid {
		t.Fatalf("raw trust report: %+v, %v", trustReport, err)
	}
	pooledTrust, err := doc.VerifySignatureWithCertificatePools(0, SignatureTrustOptions{
		SignerCertificateDER: signer, SignerIntermediatesDER: [][]byte{unrelated, intermediate},
		SignerAnchorDER: anchor, AtUnixSeconds: 1_893_456_000,
		SignerSignatureDistID: defaultSM2DistID, SealSignatureDistID: defaultSM2DistID,
	})
	if err != nil || !pooledTrust.AllValid || pooledTrust.SealIssuerChainValid != nil {
		t.Fatalf("raw candidate-pool trust report: %+v, %v", pooledTrust, err)
	}
	withoutIntermediate, err := doc.VerifySignatureWithOrderedSM2Chain(0, signer, nil, anchor, 1_893_456_000, "")
	if err != nil || withoutIntermediate.CertificateChainValid || withoutIntermediate.AllValid {
		t.Fatalf("missing intermediate report: %+v, %v", withoutIntermediate, err)
	}
	path := "Doc_0/Pages/Page_0/Content.xml"
	page, err := doc.Entry(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetEntry(path, []byte(strings.Replace(string(page), "chained", "changed", 1))); err != nil {
		t.Fatal(err)
	}
	report, err = doc.VerifySignatureWithOrderedSM2Chain(0, signer, [][]byte{intermediate}, anchor, 1_893_456_000, "")
	if err != nil || report.AllValid || report.ReferencesValid || !report.CertificateChainValid {
		t.Fatalf("tampered file report: %+v, %v", report, err)
	}
	if _, err := doc.VerifySignatureWithOrderedSM2Chain(0, signer, [][]byte{[]byte("bad")}, anchor, 1_893_456_000, ""); err == nil {
		t.Fatal("expected invalid intermediate error")
	}
	if _, err := doc.VerifySignatureWithOrderedSM2Chain(-1, signer, [][]byte{intermediate}, anchor, 1_893_456_000, ""); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.VerifySignatureWithOrderedSM2Chain(0, signer, [][]byte{intermediate}, anchor, 1_893_456_000, "bad\x00id"); err == nil {
		t.Fatal("expected NUL identifier error")
	}
}

func TestCustomSM2SignatureDistinguishingID(t *testing.T) {
	fixtures := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures")
	key, err := os.ReadFile(filepath.Join(fixtures, "chain-leaf-private-key.bin"))
	if err != nil {
		t.Fatal(err)
	}
	signer, err := os.ReadFile(filepath.Join(fixtures, "chain-leaf-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	intermediate, err := os.ReadFile(filepath.Join(fixtures, "chain-intermediate-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	anchor, err := os.ReadFile(filepath.Join(fixtures, "chain-root-cert.der"))
	if err != nil {
		t.Fatal(err)
	}
	const customID = "OFD-Rust-Interop"
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("custom ID", 210, 297); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddDigitalSignatureWithDistID(key, "2030-01-01 00:00:00", customID); err != nil {
		t.Fatal(err)
	}
	valid, err := doc.CheckDigitalSignatureWithCertificate(0, signer)
	if err != nil || valid {
		t.Fatalf("default ID verification: %v, %v", valid, err)
	}
	valid, err = doc.CheckDigitalSignatureWithCertificateAndDistID(0, signer, customID)
	if err != nil || !valid {
		t.Fatalf("custom ID verification: %v, %v", valid, err)
	}
	report, err := doc.VerifySignatureWithOrderedSM2ChainAndDistIDs(0, signer, [][]byte{intermediate}, anchor, 1_893_456_000, "", customID)
	if err != nil || !report.AllValid {
		t.Fatalf("custom ID chain report: %+v, %v", report, err)
	}
	report, err = doc.VerifySignatureWithOrderedSM2Chain(0, signer, [][]byte{intermediate}, anchor, 1_893_456_000, "")
	if err != nil || report.AllValid {
		t.Fatalf("default ID chain report: %+v, %v", report, err)
	}
	if _, err := doc.AddDigitalSignatureWithDistID(key, "2030-01-01 00:00:00", "bad\x00id"); err == nil {
		t.Fatal("expected NUL ID error")
	}
	if _, err := doc.CheckDigitalSignatureWithCertificateAndDistID(0, signer, "bad\x00id"); err == nil {
		t.Fatal("expected NUL ID error")
	}
	root := filepath.Join("..", "..", "..", "..")
	seal, err := Open(filepath.Join(root, "ofdrw-reader", "src", "test", "resources", "SESV4SignDoc.ofd"))
	if err != nil {
		t.Fatal(err)
	}
	defer seal.Close()
	valid, err = seal.CheckSESSignatureValue(0)
	if err != nil || !valid {
		t.Fatalf("default SES ID: %v, %v", valid, err)
	}
	valid, err = seal.CheckSESSignatureValueWithDistID(0, customID)
	if err != nil || valid {
		t.Fatalf("custom SES ID: %v, %v", valid, err)
	}
}

func TestReadsBookmarksAndOutlines(t *testing.T) {
	root := filepath.Join("..", "..", "..", "..")
	bookDoc, err := Open(filepath.Join(root, "ofdrw-layout", "src", "test", "resources", "Page5.ofd"))
	if err != nil {
		t.Fatal(err)
	}
	bookmarks, err := bookDoc.Bookmarks()
	if err != nil || len(bookmarks) != 1 || bookmarks[0].Name != "新建书签" || bookmarks[0].Destination.Right == nil || *bookmarks[0].Destination.Right != 167.873 {
		t.Fatalf("bookmarks: %v, %v", bookmarks, err)
	}
	bookXML, err := bookDoc.Entry("Doc_0/Document.xml")
	if err != nil {
		t.Fatal(err)
	}
	if err := bookDoc.SetEntry("Doc_0/Document.xml", []byte(strings.Replace(string(bookXML), "Name=\"新建书签\"", "", 1))); err != nil {
		t.Fatal(err)
	}
	if _, err := bookDoc.Bookmarks(); err == nil {
		t.Fatal("expected malformed bookmark error")
	}
	bookDoc.Close()
	if _, err := bookDoc.Bookmarks(); err == nil {
		t.Fatal("expected closed document error")
	}
	outlineDoc, err := Open(filepath.Join(root, "ofdrw-converter", "src", "test", "resources", "z.ofd"))
	if err != nil {
		t.Fatal(err)
	}
	outlines, err := outlineDoc.Outlines()
	if err != nil || len(outlines) != 5 || outlines[0].Title != "11.4字型变换 " || outlines[0].Destination == nil || outlines[0].Destination.PageID != "10090" {
		t.Fatalf("outlines: %v, %v", outlines, err)
	}
	outlineXML, err := outlineDoc.Entry("Doc_0/Document.xml")
	if err != nil {
		t.Fatal(err)
	}
	if err := outlineDoc.SetEntry("Doc_0/Document.xml", []byte(strings.Replace(string(outlineXML), "Expanded=\"true\"", "Expanded=\"invalid\"", 1))); err != nil {
		t.Fatal(err)
	}
	if _, err := outlineDoc.Outlines(); err == nil {
		t.Fatal("expected malformed outline error")
	}
	outlineDoc.Close()
	if _, err := outlineDoc.Outlines(); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestAddsBookmarkAndKeepsItAfterMorePages(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("first", 210, 297); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	left := 12.5
	dest := Destination{PageID: pages[0].ID, Left: &left}
	if err := doc.AddBookmark(0, "Start & <here>", dest); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBookmark(0, "Start & <here>", dest); err == nil {
		t.Fatal("expected duplicate bookmark error")
	}
	if err := doc.AddBookmark(-1, "bad", dest); err == nil {
		t.Fatal("expected negative document index error")
	}
	invalid := math.NaN()
	if err := doc.AddBookmark(0, "bad", Destination{PageID: pages[0].ID, Left: &invalid}); err == nil {
		t.Fatal("expected non-finite coordinate error")
	}
	if err := doc.AddTextPage("second", 210, 297); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "bookmark.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	bookmarks, err := reopened.Bookmarks()
	if err != nil || len(bookmarks) != 1 || bookmarks[0].Name != "Start & <here>" || bookmarks[0].Destination.Left == nil || *bookmarks[0].Destination.Left != left {
		t.Fatalf("bookmarks: %v, %v", bookmarks, err)
	}
	doc.Close()
	if err := doc.AddBookmark(0, "bad", dest); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestRemovesBookmarkAndOutlineSubtreeAcrossBinding(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("page", 210, 297); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	destination := Destination{PageID: pages[0].ID}
	for _, name := range []string{"Keep", "Delete"} {
		if err := doc.AddBookmark(0, name, destination); err != nil {
			t.Fatal(err)
		}
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("page objects: %v, %v", objects, err)
	}
	objectID := *objects[0].ID
	if err := doc.AddPageObjectBookmarkAction(0, objectID, "Delete"); err != nil {
		t.Fatal(err)
	}
	root, err := doc.AddOutline(0, "Parent", nil, nil, true)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddOutline(0, "Child", &root, nil, true); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddOutline(0, "Sibling", nil, nil, true); err != nil {
		t.Fatal(err)
	}
	if err := doc.RemoveBookmark(0, "Missing"); err == nil {
		t.Fatal("expected missing bookmark error")
	}
	if err := doc.RemoveOutline(3); err == nil {
		t.Fatal("expected out of bounds outline error")
	}
	if err := doc.RemoveBookmark(0, "Delete"); err == nil {
		t.Fatal("expected referenced bookmark error")
	}
	if err := doc.RemovePageObjectAction(0, objectID, 0); err != nil {
		t.Fatal(err)
	}
	if err := doc.RemoveBookmark(0, "Delete"); err != nil {
		t.Fatal(err)
	}
	if err := doc.RemoveOutline(root); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddTextPage("next", 210, 297); err != nil {
		t.Fatal(err)
	}
	file := filepath.Join(t.TempDir(), "navigation.ofd")
	if err := doc.Save(file); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(file)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	bookmarks, err := reopened.Bookmarks()
	if err != nil || len(bookmarks) != 1 || bookmarks[0].Name != "Keep" {
		t.Fatalf("remaining bookmarks: %v, %v", bookmarks, err)
	}
	outlines, err := reopened.Outlines()
	if err != nil || len(outlines) != 1 || outlines[0].Title != "Sibling" {
		t.Fatalf("remaining outlines: %v, %v", outlines, err)
	}
}

func TestAddsNestedOutlines(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("first", 210, 297); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	root, err := doc.AddOutline(0, "Root & Go", nil, &Destination{PageID: pages[0].ID}, true)
	if err != nil || root != 0 {
		t.Fatalf("root: %d, %v", root, err)
	}
	other, err := doc.AddOutline(0, "Other", nil, nil, true)
	if err != nil || other != 1 {
		t.Fatalf("other: %d, %v", other, err)
	}
	child, err := doc.AddOutline(0, "Child", &root, nil, false)
	if err != nil || child != 1 {
		t.Fatalf("child: %d, %v", child, err)
	}
	if err := doc.SetOutlineTitle(root, "章节 & Go"); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetOutlineExpanded(root, false); err != nil {
		t.Fatal(err)
	}
	items, err := doc.Outlines()
	if err != nil || len(items) != 3 || items[0].Title != "章节 & Go" || items[0].Expanded || items[1].ParentIndex == nil || *items[1].ParentIndex != 0 || items[2].Title != "Other" {
		t.Fatalf("outlines: %v, %v", items, err)
	}
	bad := -1
	if _, err := doc.AddOutline(0, "bad", &bad, nil, true); err == nil {
		t.Fatal("expected negative parent index")
	}
	bad = 9
	if _, err := doc.AddOutline(0, "bad", &bad, nil, true); err == nil {
		t.Fatal("expected invalid parent index")
	}
	if err := doc.AddTextPage("second", 210, 297); err != nil {
		t.Fatal(err)
	}
	pages, err = doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetOutlineDestination(root, Destination{PageID: pages[1].ID, Kind: "Fit"}); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBookmark(0, "索引 & Go", Destination{PageID: pages[1].ID, Kind: "Fit"}); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetOutlineBookmark(root, "索引 & Go"); err != nil {
		t.Fatal(err)
	}
	items, err = doc.Outlines()
	if err != nil || items[0].BookmarkName == nil || *items[0].BookmarkName != "索引 & Go" {
		t.Fatalf("outline bookmark: %+v, %v", items, err)
	}
	if err := doc.SetOutlineBookmark(root, "missing"); err == nil {
		t.Fatal("accepted unknown outline bookmark")
	}
	if err := doc.SetOutlineDestination(root, Destination{PageID: pages[1].ID, Kind: "Fit"}); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetOutlineDestination(root, Destination{PageID: "missing", Kind: "Fit"}); err == nil {
		t.Fatal("accepted unknown outline destination page")
	}
	path := filepath.Join(t.TempDir(), "outlines.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	items, err = reopened.Outlines()
	if err != nil || len(items) != 3 || items[0].Destination == nil || items[0].Destination.PageID != pages[1].ID {
		t.Fatalf("reopened outlines: %v, %v", items, err)
	}
	doc.Close()
	if _, err := doc.AddOutline(0, "bad", nil, nil, true); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestOutlineEditValidationPreservesTree(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("outline", 210, 297); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddOutline(0, "Original", nil, nil, true); err != nil {
		t.Fatal(err)
	}
	invalid := []struct {
		name string
		edit func() error
	}{
		{"negative title index", func() error { return doc.SetOutlineTitle(-1, "Changed") }},
		{"NUL title", func() error { return doc.SetOutlineTitle(0, "Changed\x00Title") }},
		{"negative expanded index", func() error { return doc.SetOutlineExpanded(-1, false) }},
		{"negative destination index", func() error {
			return doc.SetOutlineDestination(-1, Destination{PageID: "1"})
		}},
		{"negative bookmark index", func() error { return doc.SetOutlineBookmark(-1, "missing") }},
		{"NUL bookmark", func() error { return doc.SetOutlineBookmark(0, "bad\x00name") }},
	}
	for _, test := range invalid {
		t.Run(test.name, func(t *testing.T) {
			if err := test.edit(); err == nil {
				t.Fatal("expected input validation error")
			}
			outlines, err := doc.Outlines()
			if err != nil || len(outlines) != 1 || outlines[0].Title != "Original" || !outlines[0].Expanded || outlines[0].Destination != nil || outlines[0].BookmarkName != nil {
				t.Fatalf("invalid edit changed outline: %+v, %v", outlines, err)
			}
		})
	}
	doc.Close()
	for _, edit := range []func() error{
		func() error { return doc.SetOutlineTitle(0, "Changed") },
		func() error { return doc.SetOutlineExpanded(0, false) },
		func() error { return doc.SetOutlineDestination(0, Destination{PageID: "1"}) },
		func() error { return doc.SetOutlineBookmark(0, "missing") },
	} {
		if err := edit(); err == nil {
			t.Fatal("expected closed document error")
		}
	}
}

func TestPageAreaBoxesAndApplicationClip(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("clipped", 100, 100); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	path := pages[0].Path
	xml, err := doc.Entry(path)
	if err != nil {
		t.Fatal(err)
	}
	xml = bytes.Replace(xml, []byte("</ofd:PhysicalBox>"), []byte("</ofd:PhysicalBox><ofd:ApplicationBox>25 25 50 50</ofd:ApplicationBox><ofd:ContentBox>30 30 40 40</ofd:ContentBox><ofd:BleedBox>-5 -5 110 110</ofd:BleedBox>"), 1)
	if err := doc.SetEntry(path, xml); err != nil {
		t.Fatal(err)
	}
	pages, err = doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	if !slices.Equal(pages[0].ApplicationBox, []float64{25, 25, 50, 50}) || !slices.Equal(pages[0].ContentBox, []float64{30, 30, 40, 40}) || !slices.Equal(pages[0].BleedBox, []float64{-5, -5, 110, 110}) {
		t.Fatalf("page area boxes: %+v", pages[0])
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "clip-path=\"url(#ofd-application-clip)\"") {
		t.Fatalf("application clip: %s, %v", svg, err)
	}
	png, err := doc.PagePNG(0, 25.4)
	if err != nil || !bytes.HasPrefix(png, []byte("\x89PNG")) {
		t.Fatalf("PNG: %v", err)
	}
	pdf, err := doc.ToPDF()
	if err != nil || !bytes.HasPrefix(pdf, []byte("%PDF")) {
		t.Fatalf("PDF: %v", err)
	}
}

func TestSetPageAreaRoundTripsAndRejectsInvalidDimensions(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("area", 100, 100); err != nil {
		t.Fatal(err)
	}
	application := [4]float64{20, 20, 60, 60}
	content := [4]float64{25, 25, 50, 50}
	area := PageArea{
		PhysicalBox:    [4]float64{0, 0, 100, 100},
		ApplicationBox: &application,
		ContentBox:     &content,
	}
	if err := doc.SetPageArea(0, area); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	if !slices.Equal(pages[0].ApplicationBox, application[:]) || !slices.Equal(pages[0].ContentBox, content[:]) {
		t.Fatalf("page area boxes: %+v", pages[0])
	}
	if text, err := doc.PageText(0); err != nil || !slices.Equal(text, []string{"area"}) {
		t.Fatalf("page text after area edit: %v, %v", text, err)
	}
	if err := doc.SetPageArea(0, PageArea{PhysicalBox: [4]float64{0, 0, 0, 100}}); err == nil {
		t.Fatal("accepted a zero-width physical box")
	}
	if err := doc.SetPageArea(-1, area); err == nil {
		t.Fatal("accepted a negative page index")
	}
	output := filepath.Join(t.TempDir(), "area.ofd")
	if err := doc.Save(output); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(output)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	pages, err = reopened.Pages()
	if err != nil || !slices.Equal(pages[0].ApplicationBox, application[:]) {
		t.Fatalf("round-tripped application box: %+v, %v", pages, err)
	}
	if err := reopened.SetPageArea(0, PageArea{PhysicalBox: area.PhysicalBox}); err != nil {
		t.Fatal(err)
	}
	pages, err = reopened.Pages()
	if err != nil || pages[0].ApplicationBox != nil {
		t.Fatalf("removed application box: %+v, %v", pages, err)
	}
}

func TestDocumentPageAreaRoundTrips(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	initial, err := doc.DocumentPageArea(0)
	if err != nil || initial == nil || initial.PhysicalBox != [4]float64{0, 0, 210, 297} {
		t.Fatalf("initial document page area: %+v, %v", initial, err)
	}
	if _, err := doc.DocumentPageArea(-1); err == nil {
		t.Fatal("accepted a negative document index")
	}
	if _, err := doc.DocumentPageArea(1); err == nil {
		t.Fatal("accepted an out-of-range document index")
	}
	application := [4]float64{10, 5, 130, 50}
	area := PageArea{PhysicalBox: [4]float64{0, 0, 150, 60}, ApplicationBox: &application}
	if err := doc.SetDocumentPageArea(0, area); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetDocumentPageArea(0, PageArea{PhysicalBox: [4]float64{0, 0, -1, 60}}); err == nil {
		t.Fatal("accepted a negative-width physical box")
	}
	if err := doc.SetDocumentPageArea(-1, area); err == nil {
		t.Fatal("accepted a negative document index")
	}
	if err := doc.SetDocumentPageArea(1, area); err == nil {
		t.Fatal("accepted an out-of-range document index")
	}
	if err := doc.SetDocumentPageArea(0, PageArea{PhysicalBox: [4]float64{0, 0, math.NaN(), 60}}); err == nil {
		t.Fatal("accepted a nonfinite document page area")
	}
	path := filepath.Join(t.TempDir(), "document-page-area.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	got, err := reopened.DocumentPageArea(0)
	if err != nil || got == nil || got.PhysicalBox != area.PhysicalBox || got.ApplicationBox == nil || *got.ApplicationBox != application {
		t.Fatalf("round-tripped document page area: %+v, %v", got, err)
	}
}

func TestExportsSVGPreview(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("SVG & Go", 210, 297); err != nil {
		t.Fatal(err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "SVG &amp; Go") || !strings.HasPrefix(svg, "<svg ") {
		t.Fatalf("svg preview: %q, %v", svg, err)
	}
	if _, err := doc.PageSVG(-1); err == nil {
		t.Fatal("expected negative index error")
	}
	if _, err := doc.PageSVG(1); err == nil {
		t.Fatal("expected out-of-range error")
	}
	doc.Close()
	if _, err := doc.PageSVG(0); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestSVGPreviewRespectsJavaTextDeltas(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "发票示例.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil {
		t.Fatal(err)
	}
	match := regexp.MustCompile(`<text x="([^"]+)"[^>]*>24112000000048542163</text>`).FindStringSubmatch(svg)
	if len(match) != 2 || len(strings.Fields(match[1])) != 20 {
		t.Fatalf("invoice number glyph positions: %v", match)
	}
}

func TestJavaClippedTemplateRendersThroughNativeBinding(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "path_unstd.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "<clipPath") || !strings.Contains(svg, "</clipPath>") {
		t.Fatalf("clipped SVG preview: %v", err)
	}
	data, err := doc.PagePNG(0, 25.4)
	if err != nil || !bytes.HasPrefix(data, []byte("\x89PNG\r\n\x1a\n")) {
		t.Fatalf("clipped PNG preview: %v", err)
	}
}

func TestICCProfileConvertsPageColor(t *testing.T) {
	profilePath := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "swapped-rg.icc")
	profile, err := os.ReadFile(profilePath)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddPathPage("M 0 0 L 20 0 L 20 20 L 0 20 C", [4]float64{0, 0, 20, 20}, 20, 20, true, false); err != nil {
		t.Fatal(err)
	}
	resources, err := doc.Entry("Doc_0/PublicRes.xml")
	if err != nil {
		t.Fatal(err)
	}
	colorSpace := []byte(`<ofd:ColorSpaces><ofd:ColorSpace ID="100" Type="RGB" Profile="SWAPPED-RG.ICC"/></ofd:ColorSpaces>`)
	resources = bytes.Replace(resources, []byte("</ofd:Res>"), append(colorSpace, []byte("</ofd:Res>")...), 1)
	if err := doc.SetEntry("Doc_0/PublicRes.xml", resources); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetEntry("Doc_0/Res/swapped-rg.icc", profile); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	page, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	page = bytes.Replace(page, []byte("</ofd:PathObject>"), []byte(`<ofd:FillColor ColorSpace="100" Value="255 0 0"/></ofd:PathObject>`), 1)
	if err := doc.SetEntry(pages[0].Path, page); err != nil {
		t.Fatal(err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `fill="#00ff00"`) {
		t.Fatalf("ICC color: %v, %v", svg, err)
	}
}

func TestJavaRadialGradientAndClipRenderThroughNativeBinding(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-converter", "src", "test", "resources", "testPathClip.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "<radialGradient ") || !strings.Contains(svg, "<clipPath ") {
		t.Fatalf("gradient and clipped SVG preview: %v", err)
	}
	data, err := doc.PagePNG(0, 25.4)
	if err != nil || !bytes.HasPrefix(data, []byte("\x89PNG\r\n\x1a\n")) {
		t.Fatalf("gradient and clipped PNG preview: %v", err)
	}
}

func TestJavaPatternWatermarkRendersThroughNativeBinding(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-converter", "src", "test", "resources", "pattern类型.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `<pattern id="ofd-pattern-`) {
		t.Fatalf("pattern SVG preview: %v", err)
	}
	data, err := doc.PagePNG(0, 25.4)
	if err != nil || !bytes.HasPrefix(data, []byte("\x89PNG\r\n\x1a\n")) {
		t.Fatalf("pattern PNG preview: %v", err)
	}
	pdf, err := doc.ToPDF()
	if err != nil || !bytes.HasPrefix(pdf, []byte("%PDF")) {
		t.Fatalf("pattern PDF export: %v", err)
	}
}

func TestJavaJBIG2ImageRendersAndCanBeWritten(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-layout", "src", "test", "resources", "keyword2.ofd")
	source, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	data, err := source.PageImageData(0, 0)
	if err != nil {
		source.Close()
		t.Fatal(err)
	}
	svg, err := source.PageSVG(0)
	source.Close()
	if err != nil || !strings.Contains(svg, "data:image/png;base64,") {
		t.Fatalf("JBIG2 SVG preview: %v", err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddImagePage(data, "jb2", 20, 20); err != nil {
		t.Fatal(err)
	}
	images, err := doc.PageImages(0)
	if err != nil || len(images) != 1 || images[0].Format == nil || *images[0].Format != "GBIG2" {
		t.Fatalf("written JBIG2 resource: %v, %v", images, err)
	}
	preview, err := doc.PagePNG(0, 25.4)
	if err != nil || !bytes.HasPrefix(preview, []byte("\x89PNG\r\n\x1a\n")) {
		t.Fatalf("written JBIG2 PNG preview: %v", err)
	}
}

func TestJavaNestedOFDSealKeepsTransparentBackground(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-layout", "src", "test", "resources", "keyword2.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil {
		t.Fatal(err)
	}
	stamp := strings.Index(svg, "data-ofd-stamp=")
	if stamp < 0 {
		t.Fatal("missing OFD stamp")
	}
	const prefix = `href="data:image/png;base64,`
	encoded := strings.SplitN(svg[stamp:], prefix, 2)
	if len(encoded) != 2 {
		t.Fatal("missing rasterized OFD seal")
	}
	encoded = strings.SplitN(encoded[1], `"`, 2)
	data, err := base64.StdEncoding.DecodeString(encoded[0])
	if err != nil {
		t.Fatal(err)
	}
	image, err := png.Decode(bytes.NewReader(data))
	if err != nil {
		t.Fatal(err)
	}
	if _, _, _, alpha := image.At(image.Bounds().Min.X, image.Bounds().Min.Y).RGBA(); alpha != 0 {
		t.Fatalf("seal corner should be transparent, alpha=%d", alpha)
	}
	visible := false
	for y := image.Bounds().Min.Y; y < image.Bounds().Max.Y && !visible; y++ {
		for x := image.Bounds().Min.X; x < image.Bounds().Max.X; x++ {
			_, _, _, alpha := image.At(x, y).RGBA()
			if alpha > 0 {
				visible = true
				break
			}
		}
	}
	if !visible {
		t.Fatal("seal lost its visible content")
	}
	blank, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer blank.Close()
	if err := blank.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	preview, err := blank.PagePNG(0, 25.4)
	if err != nil {
		t.Fatal(err)
	}
	page, err := png.Decode(bytes.NewReader(preview))
	if err != nil {
		t.Fatal(err)
	}
	r, g, b, alpha := page.At(page.Bounds().Min.X, page.Bounds().Min.Y).RGBA()
	if r != 0xffff || g != 0xffff || b != 0xffff || alpha != 0xffff {
		t.Fatalf("regular blank page should be opaque white, got RGBA=%d,%d,%d,%d", r, g, b, alpha)
	}
}

func TestJavaTextWeightIsPreservedInPreview(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-layout", "src", "test", "resources", "SplitParagraph.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `font-weight="800"`) {
		t.Fatalf("text weight SVG preview: %v", err)
	}
	preview, err := doc.PagePNG(0, 25.4)
	if err != nil || !bytes.HasPrefix(preview, []byte("\x89PNG\r\n\x1a\n")) {
		t.Fatalf("text weight PNG preview: %v", err)
	}
}

func TestJavaCGTransformUsesEmbeddedFontGlyphs(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-layout", "src", "test", "resources", "ano.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `data-ofd-glyph="2591"`) {
		t.Fatalf("CGTransform SVG preview: %v", err)
	}
	preview, err := doc.PagePNG(0, 25.4)
	if err != nil || !bytes.HasPrefix(preview, []byte("\x89PNG\r\n\x1a\n")) {
		t.Fatalf("CGTransform PNG preview: %v", err)
	}
	pdf, err := doc.ToPDF()
	if err != nil || !bytes.HasPrefix(pdf, []byte("%PDF")) {
		t.Fatalf("CGTransform PDF export: %v", err)
	}
}

func TestInvisiblePathIsOmittedFromNativePreview(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddPathPage("M 0 0 L 10 0 L 10 10 L 0 10 C", [4]float64{0, 0, 10, 10}, 20, 20, true, false); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	data, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	xml := strings.Replace(string(data), "<ofd:PathObject", `<ofd:PathObject Visible="false"`, 1)
	if err := doc.SetEntry(pages[0].Path, []byte(xml)); err != nil {
		t.Fatal(err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || strings.Contains(svg, "<path d=") {
		t.Fatalf("invisible path SVG preview: %v", err)
	}
}

func TestJavaInvoiceAnnotationBlendMode(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "发票示例.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `style="mix-blend-mode:darken"`) {
		t.Fatalf("invoice annotation blend mode: %v", err)
	}
	preview, err := doc.PagePNG(0, 25.4)
	if err != nil || !bytes.HasPrefix(preview, []byte("\x89PNG\r\n\x1a\n")) {
		t.Fatalf("invoice blend mode PNG preview: %v", err)
	}
}

func TestJavaCompositeGraphicRendersThroughNativeBinding(t *testing.T) {
	path := filepath.Join("..", "..", "..", "..", "ofdrw-converter", "src", "test", "resources", "y.ofd")
	doc, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	text, err := doc.PageText(0)
	if err != nil || !slices.Contains(text, "30") {
		t.Fatalf("composite graphic text: %v, %v", text, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "ofd-composite-") || !strings.Contains(svg, ">30</text>") {
		t.Fatalf("composite graphic SVG preview: %v", err)
	}
	objects, err := doc.PageCompositeObjects(0)
	if err != nil || len(objects) == 0 || objects[0].ID == nil || objects[0].ResourceID == "" {
		t.Fatalf("composite graphic objects: %v, %v", objects, err)
	}
	if _, err := doc.PageCompositeObjects(-1); err == nil {
		t.Fatal("expected negative page index")
	}
	if _, err := doc.PageCompositeObjects(1000); err == nil {
		t.Fatal("expected missing page")
	}
	if _, err := doc.AddCompositeObject(0, "missing", [4]float64{1, 2, 10, 12}, CompositeStyle{}); err == nil {
		t.Fatal("expected missing composite resource")
	}
	if _, err := doc.AddCompositeObject(-1, objects[0].ResourceID, [4]float64{1, 2, 10, 12}, CompositeStyle{}); err == nil {
		t.Fatal("expected negative page index")
	}
	alpha := uint8(180)
	added, err := doc.AddCompositeObject(0, objects[0].ResourceID, [4]float64{1, 2, 10, 12}, CompositeStyle{Alpha: &alpha})
	if err != nil {
		t.Fatal(err)
	}
	inserted, err := doc.PageCompositeObjects(0)
	if err != nil || len(inserted) != len(objects)+1 || inserted[len(objects)].Alpha == nil || *inserted[len(objects)].Alpha != alpha {
		t.Fatalf("composite graphic objects after insertion: %v, %v", inserted, err)
	}
	if err := doc.RemovePageObject(0, fmt.Sprint(added)); err != nil {
		t.Fatal(err)
	}
	if err := doc.RemovePageObject(0, *objects[0].ID); err != nil {
		t.Fatal(err)
	}
	after, err := doc.PageCompositeObjects(0)
	if err != nil || len(after) != len(objects)-1 {
		t.Fatalf("composite graphic objects after removal: %v, %v", after, err)
	}
}

func TestCreatesCompositePathResources(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("base", 40, 40); err != nil {
		t.Fatal(err)
	}
	fill := [3]uint8{255, 0, 0}
	id, err := doc.AddCompositePathObject(0, "M 0 0 L 10 0 L 10 10 C", [4]float64{2, 3, 10, 10}, PathStyle{Fill: true, FillColor: &fill})
	if err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageCompositeObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil || *objects[0].ID != fmt.Sprint(id) {
		t.Fatalf("composite path objects: %v, %v", objects, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "fill=\"#ff0000\"") {
		t.Fatalf("composite path SVG: %v", err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("composite references: %v, %v", issues, err)
	}
	if _, err := doc.AddCompositePathObject(0, "M 0", [4]float64{2, 3, 10, 10}, PathStyle{}); err == nil {
		t.Fatal("expected invalid path error")
	}
	if _, err := doc.AddCompositePathObject(-1, "M 0 0 L 1 1", [4]float64{2, 3, 10, 10}, PathStyle{}); err == nil {
		t.Fatal("expected negative page index")
	}
	after, err := doc.PageCompositeObjects(0)
	if err != nil || len(after) != 1 {
		t.Fatalf("composite objects after rejected writes: %v, %v", after, err)
	}
}

func TestCreatesCompositeTextResource(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("base", 40, 40); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddCompositeTextObject(0, "Rust & 中", [4]float64{2, 3, 20, 8}, 5, "1")
	if err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageCompositeObjects(0)
	if err != nil || len(objects) != 1 || objects[0].ID == nil || *objects[0].ID != fmt.Sprint(id) {
		t.Fatalf("composite text objects: %v, %v", objects, err)
	}
	text, err := doc.PageText(0)
	if err != nil || !slices.Contains(text, "Rust & 中") {
		t.Fatalf("composite text: %v, %v", text, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("composite text references: %v, %v", issues, err)
	}
	if _, err := doc.AddCompositeTextObject(0, "bad", [4]float64{2, 3, 20, 8}, 5, "missing"); err == nil {
		t.Fatal("expected missing font error")
	}
	if _, err := doc.AddCompositeTextObject(-1, "bad", [4]float64{2, 3, 20, 8}, 5, ""); err == nil {
		t.Fatal("expected negative page index")
	}
	remaining, err := doc.PageCompositeObjects(0)
	if err != nil || len(remaining) != 1 {
		t.Fatalf("composite text objects after rejected writes: %v, %v", remaining, err)
	}
}

func TestRendersPNGPreview(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("PNG", 20, 20); err != nil {
		t.Fatal(err)
	}
	data, err := doc.PagePNG(0, 25.4)
	if err != nil {
		t.Fatal(err)
	}
	image, err := png.DecodeConfig(bytes.NewReader(data))
	if err != nil || image.Width != 20 || image.Height != 20 {
		t.Fatalf("PNG dimensions: %v, %v", image, err)
	}
	if _, err := doc.PagePNG(0, 0); err == nil {
		t.Fatal("expected invalid DPI error")
	}
	if _, err := doc.PagePNG(-1, 25.4); err == nil {
		t.Fatal("expected negative page index error")
	}
}

func TestPathStyleAttributesCrossNativeBinding(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddPathPage("M 1 10 L 19 10", [4]float64{0, 0, 20, 20}, 20, 20, false, true); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	data, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	xml := strings.Replace(string(data), `Stroke="true"`, `Stroke="true" LineWidth="2" Cap="Round" Join="Bevel" DashPattern="4 4"`, 1)
	xml = strings.Replace(xml, "</ofd:PathObject>", `<ofd:StrokeColor Value="255 0 0" Alpha="128"/></ofd:PathObject>`, 1)
	if err := doc.SetEntry(pages[0].Path, []byte(xml)); err != nil {
		t.Fatal(err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `stroke-dasharray="4 4"`) || !strings.Contains(svg, `stroke-linecap="round"`) || !strings.Contains(svg, `stroke-opacity="0.5019607843137255"`) {
		t.Fatalf("styled SVG preview: %v", err)
	}
	resources, err := doc.Entry("Doc_0/PublicRes.xml")
	if err != nil {
		t.Fatal(err)
	}
	res := strings.Replace(string(resources), "</ofd:Res>", `<ofd:DrawParams><ofd:DrawParam ID="10" DashPattern="2 2"><ofd:StrokeColor Value="0 255 0"/></ofd:DrawParam></ofd:DrawParams></ofd:Res>`, 1)
	if err := doc.SetEntry("Doc_0/PublicRes.xml", []byte(res)); err != nil {
		t.Fatal(err)
	}
	styled := strings.Replace(string(data), "<ofd:PathObject ID=", `<ofd:PathObject DrawParam="10" ID=`, 1)
	if err := doc.SetEntry(pages[0].Path, []byte(styled)); err != nil {
		t.Fatal(err)
	}
	svg, err = doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `stroke="#00ff00"`) || !strings.Contains(svg, `stroke-dasharray="2 2"`) {
		t.Fatalf("DrawParam SVG preview: %v", err)
	}
	localPath := pages[0].Path[:strings.LastIndex(pages[0].Path, "/")] + "/PageRes.xml"
	localRes := `<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016"><ofd:DrawParams><ofd:DrawParam ID="10"><ofd:StrokeColor Value="0 0 255"/></ofd:DrawParam></ofd:DrawParams></ofd:Res>`
	if err := doc.SetEntry(localPath, []byte(localRes)); err != nil {
		t.Fatal(err)
	}
	localPage := strings.Replace(string(data), "<ofd:Content>", "<ofd:PageRes>PageRes.xml</ofd:PageRes><ofd:Content>", 1)
	localPage = strings.Replace(localPage, "<ofd:PathObject ID=", `<ofd:PathObject DrawParam="10" ID=`, 1)
	if err := doc.SetEntry(pages[0].Path, []byte(localPage)); err != nil {
		t.Fatal(err)
	}
	svg, err = doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, `stroke="#0000ff"`) {
		t.Fatalf("page-local DrawParam SVG preview: %v", err)
	}
}

func TestExportsMultiPagePDF(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if _, err := doc.ToPDF(); err == nil {
		t.Fatal("expected empty document error")
	}
	if err := doc.AddTextPage("first", 20, 30); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddTextPage("second", 40, 50); err != nil {
		t.Fatal(err)
	}
	data, err := doc.ToPDF()
	if err != nil || !bytes.HasPrefix(data, []byte("%PDF-")) || !bytes.Contains(data, []byte("/Count 2")) {
		t.Fatalf("PDF export: %d bytes, %v", len(data), err)
	}
}

func TestAddsPositionedTextObject(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("first", 210, 297); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddTextObject(0, "定位 & Go", [4]float64{30, 40, 80, 8}, 5, "")
	if err != nil || id == 0 {
		t.Fatalf("add text object: %d, %v", id, err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 2 || objects[1].ID == nil || *objects[1].ID != fmt.Sprint(id) {
		t.Fatalf("text objects: %v, %v", objects, err)
	}
	if _, err := doc.AddTextObject(-1, "bad", [4]float64{0, 0, 10, 5}, 5, ""); err == nil {
		t.Fatal("expected negative page index error")
	}
	if _, err := doc.AddTextObject(0, "bad", [4]float64{0, 0, 10, 5}, 5, "999"); err == nil {
		t.Fatal("expected unknown font error")
	}
	if _, err := doc.AddTextObject(0, "bad", [4]float64{math.NaN(), 0, 10, 5}, 5, ""); err == nil {
		t.Fatal("expected invalid JSON number error")
	}
	if err := doc.AddTextPage("second", 210, 297); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "positioned.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	text, err := reopened.PageText(0)
	if err != nil || len(text) != 2 || text[1] != "定位 & Go" {
		t.Fatalf("page text: %v, %v", text, err)
	}
	doc.Close()
	if _, err := doc.AddTextObject(0, "bad", [4]float64{0, 0, 10, 5}, 5, ""); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestAddsColoredPositionedTextObject(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("first", 100, 100); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddColoredTextObject(0, "red", [4]float64{30, 40, 30, 8}, 5, "", [3]uint8{255, 0, 0})
	if err != nil {
		t.Fatal(err)
	}
	paints, err := doc.PageObjectPaints(0)
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, paint := range paints {
		if paint.ID != nil && *paint.ID == fmt.Sprint(id) {
			found = paint.Fill != nil && paint.Fill.Value != nil && *paint.Fill.Value == "255 0 0"
		}
	}
	if !found {
		t.Fatalf("colored text paint missing: %+v", paints)
	}
}

func TestShapedTextObjectSplitsMixedLTRScripts(t *testing.T) {
	fontPath := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-sans.ttf")
	font, err := os.ReadFile(fontPath)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	fontID, err := doc.AddEmbeddedFont("DejaVu Sans", font)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBlankPage(40, 20); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddShapedTextObject(0, "AЖB", [4]float64{5, 5, 30, 10}, 6, fmt.Sprint(fontID)); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || len(objects[0].CGTransforms) != 1 || objects[0].CGTransforms[0].CodeCount != 3 || objects[0].CGTransforms[0].GlyphCount != 3 {
		t.Fatalf("mixed-script glyph mapping: %v, %v", objects, err)
	}
}

func TestShapedTextObjectPreservesLigatureTextAndGlyphs(t *testing.T) {
	fontPath := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-serif.ttf")
	font, err := os.ReadFile(fontPath)
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
	if err := doc.AddBlankPage(40, 20); err != nil {
		t.Fatal(err)
	}
	boundary := [4]float64{5, 5, 30, 10}
	id, err := doc.AddColoredShapedTextObject(0, "AfiB", boundary, 6, fmt.Sprint(fontID), [3]uint8{12, 34, 56})
	if err != nil || id == 0 {
		t.Fatalf("add shaped text: %d, %v", id, err)
	}
	if text, err := doc.PageText(0); err != nil || !slices.Equal(text, []string{"AfiB"}) {
		t.Fatalf("shaped text extraction: %v, %v", text, err)
	}
	paints, err := doc.PageObjectPaints(0)
	if err != nil || len(paints) != 1 || paints[0].Fill == nil || paints[0].Fill.Value == nil || *paints[0].Fill.Value != "12 34 56" {
		t.Fatalf("shaped text fill: %+v, %v", paints, err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 1 || len(objects[0].CGTransforms) != 3 {
		t.Fatalf("shaped text mappings: %v, %v", objects, err)
	}
	ligature := objects[0].CGTransforms[1]
	if ligature.CodePosition != 1 || ligature.CodeCount != 2 || ligature.GlyphCount != 1 || len(ligature.Glyphs) != 1 {
		t.Fatalf("ligature mapping: %+v", ligature)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	pageXML, err := doc.Entry(pages[0].Path)
	if err != nil || !bytes.Contains(pageXML, []byte(`CodePosition="1" CodeCount="2" GlyphCount="1"`)) {
		t.Fatalf("shaped mapping: %v, %v", string(pageXML), err)
	}
	if svg, err := doc.PageSVG(0); err != nil || !strings.Contains(svg, "data-ofd-glyph=") {
		t.Fatalf("shaped SVG: %v, %v", svg, err)
	}
	if _, err := doc.AddShapedTextObject(0, "A\nB", boundary, 6, fmt.Sprint(fontID)); err == nil {
		t.Fatal("expected multiline shaping to fail")
	}
	if _, err := doc.AddShapedTextObject(0, "fi", boundary, 6, ""); err == nil {
		t.Fatal("expected missing embedded font error")
	}
	path := filepath.Join(t.TempDir(), "shaped.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	if text, err := reopened.PageText(0); err != nil || !slices.Equal(text, []string{"AfiB"}) {
		t.Fatalf("reopened shaped text: %v, %v", text, err)
	}
}

func TestShapedTextLanguageSelectsLocalizedGlyphs(t *testing.T) {
	fontPath := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-sans.ttf")
	font, err := os.ReadFile(fontPath)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	fontID, err := doc.AddEmbeddedFont("DejaVu Sans", font)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBlankPage(40, 20); err != nil {
		t.Fatal(err)
	}
	for index, language := range []string{"ru", "sr"} {
		if _, err := doc.AddShapedTextObjectWithLanguage(0, "бгдпт", [4]float64{2, 2 + float64(index)*7, 30, 6}, 5, fmt.Sprint(fontID), language); err != nil {
			t.Fatal(err)
		}
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 2 {
		t.Fatalf("localized text objects: %v, %v", objects, err)
	}
	if reflect.DeepEqual(objects[0].CGTransforms, objects[1].CGTransforms) {
		t.Fatal("expected language-specific glyph substitution")
	}
	if _, err := doc.AddColoredShapedTextObjectWithLanguage(0, "бгдпт", [4]float64{2, 14, 30, 6}, 5, fmt.Sprint(fontID), "sr", [3]uint8{12, 34, 56}); err != nil {
		t.Fatal(err)
	}
	paints, err := doc.PageObjectPaints(0)
	if err != nil || len(paints) != 3 || paints[2].Fill == nil || paints[2].Fill.Value == nil || *paints[2].Fill.Value != "12 34 56" {
		t.Fatalf("colored localized glyph paint: %+v, %v", paints, err)
	}
	if _, err := doc.AddColoredShapedTextObjectWithLanguage(0, "x", [4]float64{2, 2, 30, 6}, 5, fmt.Sprint(fontID), "", [3]uint8{1, 2, 3}); err == nil {
		t.Fatal("expected empty colored shaping language error")
	}
	if _, err := doc.AddShapedTextObjectWithLanguage(0, "бгдпт", [4]float64{2, 2, 30, 6}, 5, fmt.Sprint(fontID), "sr__invalid"); err == nil {
		t.Fatal("expected invalid language error")
	}
	if _, err := doc.AddShapedTextObjectWithLanguage(0, "бгдпт", [4]float64{2, 2, 30, 6}, 5, fmt.Sprint(fontID), ""); err == nil {
		t.Fatal("expected empty language error")
	}
}

func TestShapedRightToLeftTextRoundTrips(t *testing.T) {
	fontPath := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-sans.ttf")
	font, err := os.ReadFile(fontPath)
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	fontID, err := doc.AddEmbeddedFont("DejaVu Sans", font)
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBlankPage(40, 20); err != nil {
		t.Fatal(err)
	}
	boundary := [4]float64{5, 5, 30, 10}
	if _, err := doc.AddShapedTextObject(0, "سلام", boundary, 6, fmt.Sprint(fontID)); err != nil {
		t.Fatal(err)
	}
	if text, err := doc.PageText(0); err != nil || !slices.Equal(text, []string{"سلام"}) {
		t.Fatalf("Arabic text extraction: %v, %v", text, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	pageXML, err := doc.Entry(pages[0].Path)
	if err != nil || !bytes.Contains(pageXML, []byte(`CodePosition="0" CodeCount="4"`)) {
		t.Fatalf("Arabic glyph mapping: %v, %v", string(pageXML), err)
	}
	if svg, err := doc.PageSVG(0); err != nil || !strings.Contains(svg, "data-ofd-glyph=") {
		t.Fatalf("Arabic SVG: %v, %v", svg, err)
	}
	if _, err := doc.AddShapedTextObject(0, "AسلامB", boundary, 6, fmt.Sprint(fontID)); err != nil {
		t.Fatal(err)
	}
	if text, err := doc.PageText(0); err != nil || !slices.Equal(text, []string{"سلام", "AسلامB"}) {
		t.Fatalf("bidirectional text extraction: %v, %v", text, err)
	}
	objects, err := doc.PageTextObjects(0)
	if err != nil || len(objects) != 2 || len(objects[1].CGTransforms) != 1 {
		t.Fatalf("bidirectional mappings: %v, %v", objects, err)
	}
	if mapping := objects[1].CGTransforms[0]; mapping.CodeCount != 6 || mapping.GlyphCount != 5 || len(mapping.Glyphs) != 5 {
		t.Fatalf("bidirectional mapping: %+v", mapping)
	}
	pageXML, err = doc.Entry(pages[0].Path)
	if err != nil || !bytes.Contains(pageXML, []byte(`CodePosition="0" CodeCount="6" GlyphCount="5"`)) {
		t.Fatalf("bidirectional glyph mapping: %v, %v", string(pageXML), err)
	}
	if _, err := doc.AddShapedTextObject(0, "A\nسلام", boundary, 6, fmt.Sprint(fontID)); err == nil {
		t.Fatal("expected multiline shaping to fail")
	}
	path := filepath.Join(t.TempDir(), "arabic.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	if text, err := reopened.PageText(0); err != nil || !slices.Equal(text, []string{"سلام", "AسلامB"}) {
		t.Fatalf("reopened Arabic text: %v, %v", text, err)
	}
}

func TestDuplicatesPageObjectAtomically(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	original, err := doc.AddPathObject(0, "M 0 0 L 10 10", [4]float64{0, 0, 10, 10}, PathStyle{
		Stroke: true, DashSequence: []float64{1, 2, 3},
	})
	if err != nil {
		t.Fatal(err)
	}
	copied, err := doc.DuplicatePageObject(0, strconv.Itoa(original))
	if err != nil || copied <= original {
		t.Fatalf("copy ID: %d, %v", copied, err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 2 || paths[0].ID == nil || *paths[0].ID != strconv.Itoa(original) || paths[1].ID == nil || *paths[1].ID != strconv.Itoa(copied) {
		t.Fatalf("copied paths: %+v, %v", paths, err)
	}
	styles, err := doc.PageObjectStrokeStyles(0)
	if err != nil || len(styles) != 2 || !reflect.DeepEqual(styles[1].Style.DashSequence, []float64{1, 2, 3}) {
		t.Fatalf("copied style: %+v, %v", styles, err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	before, err := doc.Entry(pages[0].Path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := doc.DuplicatePageObject(0, "missing"); err == nil {
		t.Fatal("expected missing object error")
	}
	after, err := doc.Entry(pages[0].Path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid duplicate changed page: %v", err)
	}
}

func TestRemovesPageObject(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("keep", 210, 297); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddTextObject(0, "remove", [4]float64{20, 30, 50, 8}, 5, "")
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.RemovePageObject(0, fmt.Sprint(id)); err != nil {
		t.Fatal(err)
	}
	text, err := doc.PageText(0)
	if err != nil || len(text) != 1 || text[0] != "keep" {
		t.Fatalf("text: %v, %v", text, err)
	}
	if err := doc.RemovePageObject(-1, fmt.Sprint(id)); err == nil {
		t.Fatal("expected negative page index")
	}
	if err := doc.RemovePageObject(0, fmt.Sprint(id)); err == nil {
		t.Fatal("expected missing object error")
	}
	path := filepath.Join(t.TempDir(), "removed_object.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	text, err = reopened.PageText(0)
	if err != nil || len(text) != 1 {
		t.Fatalf("reopened text: %v, %v", text, err)
	}
	doc.Close()
	if err := doc.RemovePageObject(0, fmt.Sprint(id)); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestCreatesLayersAndMovesObjects(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	background, err := doc.AddPageLayer(0, "Background", "")
	if err != nil {
		t.Fatal(err)
	}
	foreground, err := doc.AddPageLayer(0, "Foreground", "")
	if err != nil {
		t.Fatal(err)
	}
	blue := [3]uint8{0, 0, 255}
	red := [3]uint8{255, 0, 0}
	commands := "M 0 0 L 20 0 L 20 20 L 0 20 C"
	first, err := doc.AddPathObject(0, commands, [4]float64{0, 0, 20, 20}, PathStyle{Fill: true, FillColor: &blue})
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.MovePageObjectToLayer(0, fmt.Sprint(first), fmt.Sprint(background)); err != nil {
		t.Fatal(err)
	}
	second, err := doc.AddPathObject(0, commands, [4]float64{0, 0, 20, 20}, PathStyle{Fill: true, FillColor: &red})
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.MovePageObjectToLayer(0, fmt.Sprint(second), fmt.Sprint(foreground)); err != nil {
		t.Fatal(err)
	}
	layers, err := doc.PageLayers(0)
	if err != nil || len(layers) != 3 || layers[0].ObjectCount != 1 || layers[2].ObjectCount != 1 {
		t.Fatalf("layers: %+v, %v", layers, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || strings.Index(svg, "#0000ff") >= strings.Index(svg, "#ff0000") {
		t.Fatalf("svg: %v, %v", svg, err)
	}
	if _, err := doc.AddPageLayer(0, "bad", ""); err == nil {
		t.Fatal("expected invalid type")
	}
	if _, err := doc.AddPageLayer(0, "Body", "missing"); err == nil {
		t.Fatal("expected invalid resource")
	}
	if err := doc.MovePageObjectToLayer(0, "missing", fmt.Sprint(background)); err == nil {
		t.Fatal("expected missing object")
	}
	if err := doc.MovePageObjectToLayer(0, fmt.Sprint(first), "missing"); err == nil {
		t.Fatal("expected missing layer")
	}
	if _, err := doc.AddPageLayer(-1, "Body", ""); err == nil {
		t.Fatal("expected negative page index")
	}
	if err := doc.MovePageObjectToLayer(-1, "1", "2"); err == nil {
		t.Fatal("expected negative page index")
	}
	if err := doc.RemovePageLayer(0, fmt.Sprint(foreground)); err == nil {
		t.Fatal("expected nonempty layer rejection")
	}
	empty, err := doc.AddPageLayer(0, "Foreground", "")
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.RemovePageLayer(0, fmt.Sprint(empty)); err != nil {
		t.Fatal(err)
	}
	if err := doc.RemovePageLayer(0, fmt.Sprint(empty)); err == nil {
		t.Fatal("expected missing layer")
	}
	if err := doc.RemovePageLayer(-1, fmt.Sprint(background)); err == nil {
		t.Fatal("expected negative page index")
	}
	path := filepath.Join(t.TempDir(), "layered.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	layers, err = reopened.PageLayers(0)
	if err != nil || len(layers) != 3 {
		t.Fatalf("reopened layers: %+v, %v", layers, err)
	}
	doc.Close()
	if _, err := doc.AddPageLayer(0, "Body", ""); err == nil {
		t.Fatal("expected closed document")
	}
	if err := doc.MovePageObjectToLayer(0, "1", "2"); err == nil {
		t.Fatal("expected closed document")
	}
	if err := doc.RemovePageLayer(0, fmt.Sprint(background)); err == nil {
		t.Fatal("expected closed document")
	}
}

func TestReordersPageObjects(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	commands := "M 0 0 L 20 0 L 20 20 L 0 20 C"
	boundary := [4]float64{0, 0, 20, 20}
	redColor, greenColor, blueColor := [3]uint8{255, 0, 0}, [3]uint8{0, 255, 0}, [3]uint8{0, 0, 255}
	red, err := doc.AddPathObject(0, commands, boundary, PathStyle{Fill: true, FillColor: &redColor})
	if err != nil {
		t.Fatal(err)
	}
	green, err := doc.AddPathObject(0, commands, boundary, PathStyle{Fill: true, FillColor: &greenColor})
	if err != nil {
		t.Fatal(err)
	}
	blue, err := doc.AddPathObject(0, commands, boundary, PathStyle{Fill: true, FillColor: &blueColor})
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.MovePageObject(0, fmt.Sprint(blue), fmt.Sprint(red)); err != nil {
		t.Fatal(err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || paths[0].ID == nil || *paths[0].ID != fmt.Sprint(blue) {
		t.Fatalf("reordered paths: %v, %v", paths, err)
	}
	if err := doc.MovePageObject(0, fmt.Sprint(red), ""); err != nil {
		t.Fatal(err)
	}
	paths, err = doc.PagePaths(0)
	if err != nil || paths[1].ID == nil || *paths[1].ID != fmt.Sprint(green) || paths[2].ID == nil || *paths[2].ID != fmt.Sprint(red) {
		t.Fatalf("appended paths: %v, %v", paths, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil {
		t.Fatal(err)
	}
	blueAt := strings.Index(svg, `fill="#0000ff"`)
	greenAt := strings.Index(svg, `fill="#00ff00"`)
	redAt := strings.Index(svg, `fill="#ff0000"`)
	if blueAt < 0 || greenAt <= blueAt || redAt <= greenAt {
		t.Fatal("SVG paint order does not match the reordered objects")
	}
	if err := doc.MovePageObject(0, fmt.Sprint(red), "missing"); err == nil {
		t.Fatal("expected missing target error")
	}
	if err := doc.MovePageObject(-1, fmt.Sprint(red), ""); err == nil {
		t.Fatal("expected negative page index error")
	}
	if err := doc.MovePageObject(0, "bad\x00id", ""); err == nil {
		t.Fatal("expected invalid object ID error")
	}
	if err := doc.MovePageObject(0, fmt.Sprint(red), "bad\x00id"); err == nil {
		t.Fatal("expected invalid target ID error")
	}
	foreground, err := doc.AddPageLayer(0, "Foreground", "")
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.MovePageObjectToLayer(0, fmt.Sprint(blue), fmt.Sprint(foreground)); err != nil {
		t.Fatal(err)
	}
	if err := doc.MovePageObject(0, fmt.Sprint(blue), fmt.Sprint(green)); err != nil {
		t.Fatal(err)
	}
	layers, err := doc.PageLayers(0)
	if err != nil || layers[len(layers)-1].ObjectCount != 0 {
		t.Fatalf("cross-layer move: %v, %v", layers, err)
	}
	svg, err = doc.PageSVG(0)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "reordered.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	reopenedSVG, err := reopened.PageSVG(0)
	if err != nil || reopenedSVG != svg {
		t.Fatalf("reopened SVG: %v", err)
	}
	doc.Close()
	if err := doc.MovePageObject(0, fmt.Sprint(red), ""); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestUpdatesPageObjectVisibilityAndAlpha(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddTextObject(0, "hello", [4]float64{1, 1, 15, 8}, 4, "")
	if err != nil {
		t.Fatal(err)
	}
	objectID := fmt.Sprint(id)
	if err := doc.SetPageObjectVisible(0, objectID, false); err != nil {
		t.Fatal(err)
	}
	lines, err := doc.PageText(0)
	if err != nil || len(lines) != 0 {
		t.Fatalf("hidden text: %v, %v", lines, err)
	}
	if err := doc.SetPageObjectVisible(0, objectID, true); err != nil {
		t.Fatal(err)
	}
	lines, err = doc.PageText(0)
	if err != nil || len(lines) != 1 || lines[0] != "hello" {
		t.Fatalf("visible text: %v, %v", lines, err)
	}
	if err := doc.SetPageObjectAlpha(0, objectID, 0); err != nil {
		t.Fatal(err)
	}
	lines, err = doc.PageText(0)
	if err != nil || len(lines) != 0 {
		t.Fatalf("transparent text: %v, %v", lines, err)
	}
	if err := doc.SetPageObjectAlpha(0, objectID, 128); err != nil {
		t.Fatal(err)
	}
	objects, err := doc.PageObjects(0)
	if err != nil || len(objects) != 1 || objects[0].Kind != "TextObject" || objects[0].Alpha != 128 || !objects[0].Visible {
		t.Fatalf("page objects: %v, %v", objects, err)
	}
	if _, err := doc.PageObjects(-1); err == nil {
		t.Fatal("expected negative page")
	}
	if _, err := doc.PageObjects(1); err == nil {
		t.Fatal("expected missing page")
	}
	if err := doc.SetPageObjectAlpha(0, objectID, 256); err == nil {
		t.Fatal("expected invalid alpha")
	}
	if err := doc.SetPageObjectVisible(0, "missing", false); err == nil {
		t.Fatal("expected missing object")
	}
	if err := doc.SetPageObjectVisible(-1, objectID, false); err == nil {
		t.Fatal("expected negative page")
	}
	if err := doc.SetPageObjectAlpha(0, "bad\x00id", 1); err == nil {
		t.Fatal("expected invalid ID")
	}
	doc.Close()
	if _, err := doc.PageObjects(0); err == nil {
		t.Fatal("expected closed document")
	}
	if err := doc.SetPageObjectAlpha(0, objectID, 255); err == nil {
		t.Fatal("expected closed document")
	}
}

func TestEditsPageObjectGeometry(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(30, 20); err != nil {
		t.Fatal(err)
	}
	color := [3]uint8{255, 0, 0}
	id, err := doc.AddPathObject(0, "M 0 0 L 5 0 L 5 5 L 0 5 C", [4]float64{2, 2, 5, 5}, PathStyle{Fill: true, FillColor: &color})
	if err != nil {
		t.Fatal(err)
	}
	objectID := fmt.Sprint(id)
	if err := doc.SetPageObjectBoundary(0, objectID, [4]float64{12, 2, 5, 5}); err != nil {
		t.Fatal(err)
	}
	ctm := [6]float64{1, 0, 0, 1, 5, 0}
	if err := doc.SetPageObjectCTM(0, objectID, &ctm); err != nil {
		t.Fatal(err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 1 || !slices.Equal(paths[0].Boundary, []float64{12, 2, 5, 5}) || !slices.Equal(paths[0].CTM, ctm[:]) {
		t.Fatalf("geometry: %v, %v", paths, err)
	}
	if err := doc.SetPageObjectCTM(0, objectID, nil); err != nil {
		t.Fatal(err)
	}
	paths, err = doc.PagePaths(0)
	if err != nil || paths[0].CTM != nil {
		t.Fatalf("cleared CTM: %v, %v", paths, err)
	}
	if err := doc.SetPageObjectBoundary(0, objectID, [4]float64{0, 0, 0, 5}); err == nil {
		t.Fatal("expected invalid boundary")
	}
	if err := doc.SetPageObjectCTM(0, objectID, &[6]float64{1, 0, 0, 1, math.NaN(), 0}); err == nil {
		t.Fatal("expected invalid CTM")
	}
	if err := doc.SetPageObjectBoundary(-1, objectID, [4]float64{0, 0, 5, 5}); err == nil {
		t.Fatal("expected negative page")
	}
	if err := doc.SetPageObjectCTM(0, "bad\x00id", nil); err == nil {
		t.Fatal("expected invalid ID")
	}
	doc.Close()
	if err := doc.SetPageObjectCTM(0, objectID, nil); err == nil {
		t.Fatal("expected closed document")
	}
}

func TestAddsPositionedPathObject(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("first", 210, 297); err != nil {
		t.Fatal(err)
	}
	red := [3]uint8{255, 0, 0}
	style := PathStyle{Stroke: true, StrokeColor: &red}
	id, err := doc.AddPathObject(0, "M 0 0 L 10 10", [4]float64{20, 30, 10, 10}, style)
	if err != nil || id == 0 {
		t.Fatalf("add path object: %d, %v", id, err)
	}
	paths, err := doc.PagePaths(0)
	if err != nil || len(paths) != 1 || paths[0].ID == nil || *paths[0].ID != fmt.Sprint(id) {
		t.Fatalf("paths: %v, %v", paths, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "#ff0000") {
		t.Fatalf("svg: %v, %v", svg, err)
	}
	if _, err := doc.AddPathObject(-1, "M 0 0", [4]float64{0, 0, 10, 10}, style); err == nil {
		t.Fatal("expected negative page index error")
	}
	if _, err := doc.AddPathObject(0, "M 0 0", [4]float64{math.NaN(), 0, 10, 10}, style); err == nil {
		t.Fatal("expected invalid JSON number error")
	}
	path := filepath.Join(t.TempDir(), "positioned_path.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	paths, err = reopened.PagePaths(0)
	if err != nil || len(paths) != 1 {
		t.Fatalf("reopened paths: %v, %v", paths, err)
	}
	doc.Close()
	if _, err := doc.AddPathObject(0, "M 0 0", [4]float64{0, 0, 10, 10}, style); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestWritesEvenOddPathStyles(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(20, 20); err != nil {
		t.Fatal(err)
	}
	commands := "M 0 0 L 20 0 L 20 20 L 0 20 C M 5 5 L 15 5 L 15 15 L 5 15 C"
	alpha := uint8(128)
	style := PathStyle{Fill: true, EvenOdd: true, Alpha: &alpha}
	if _, err := doc.AddPathObject(0, commands, [4]float64{0, 0, 20, 20}, style); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddCompositePathObject(0, commands, [4]float64{0, 0, 20, 20}, style); err != nil {
		t.Fatal(err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || strings.Count(svg, `fill-rule="evenodd"`) != 2 {
		t.Fatalf("even-odd SVG: %v, %v", svg, err)
	}
	if strings.Count(svg, `opacity="0.5019607843137255"`) != 2 {
		t.Fatalf("alpha SVG: %s", svg)
	}
	path := filepath.Join(t.TempDir(), "even_odd.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	svg, err = reopened.PageSVG(0)
	if err != nil || strings.Count(svg, `fill-rule="evenodd"`) != 2 {
		t.Fatalf("reopened even-odd SVG: %v, %v", svg, err)
	}
	if strings.Count(svg, `opacity="0.5019607843137255"`) != 2 {
		t.Fatalf("reopened alpha SVG: %s", svg)
	}
}

func TestWritesPathStrokeStyles(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(30, 20); err != nil {
		t.Fatal(err)
	}
	limit, offset := 5.0, 1.0
	dashes := [2]float64{3, 2}
	fillAlpha, strokeAlpha := uint8(128), uint8(64)
	style := PathStyle{Fill: true, Stroke: true, LineWidth: 2, Cap: "round", Join: "bevel", MiterLimit: &limit, DashPattern: &dashes, DashOffset: &offset, FillAlpha: &fillAlpha, StrokeAlpha: &strokeAlpha}
	commands := "M 2 10 L 28 10"
	boundary := [4]float64{0, 0, 30, 20}
	if _, err := doc.AddPathObject(0, commands, boundary, style); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddCompositePathObject(0, commands, boundary, style); err != nil {
		t.Fatal(err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil {
		t.Fatal(err)
	}
	for _, attribute := range []string{`stroke-linecap="round"`, `stroke-linejoin="bevel"`, `stroke-miterlimit="5"`, `stroke-dasharray="3 2"`, `stroke-dashoffset="1"`, `fill-opacity="0.5019607843137255"`, `stroke-opacity="0.25098039215686274"`} {
		if strings.Count(svg, attribute) != 2 {
			t.Fatalf("missing %s in SVG", attribute)
		}
	}
	bad := style
	bad.Cap = "flat"
	if _, err := doc.AddPathObject(0, commands, boundary, bad); err == nil {
		t.Fatal("expected invalid cap")
	}
	path := filepath.Join(t.TempDir(), "styled_stroke.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	reopenedSVG, err := reopened.PageSVG(0)
	if err != nil || reopenedSVG != svg {
		t.Fatalf("reopened SVG: %v", err)
	}
}

func TestAddsTextWatermarkAnnotation(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("first", 210, 297); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddTextWatermark(0, "水印 & Go", [4]float64{10, 20, 60, 10}, 5)
	if err != nil || id == 0 {
		t.Fatalf("add watermark: %d, %v", id, err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 || annotations[0].ID != fmt.Sprint(id) || annotations[0].Text[0] != "水印 & Go" {
		t.Fatalf("annotations: %v, %v", annotations, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "水印 &amp; Go") {
		t.Fatalf("watermark SVG: %v, %v", svg, err)
	}
	if _, err := doc.AddTextWatermark(-1, "bad", [4]float64{0, 0, 10, 10}, 5); err == nil {
		t.Fatal("expected negative page index")
	}
	if _, err := doc.AddTextWatermark(0, "bad", [4]float64{math.NaN(), 0, 10, 10}, 5); err == nil {
		t.Fatal("expected invalid JSON number")
	}
	if _, err := doc.AddTextWatermark(0, "", [4]float64{0, 0, 10, 10}, 5); err == nil {
		t.Fatal("expected empty watermark")
	}
	if err := doc.AddTextPage("second", 210, 297); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "watermark.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	annotations, err = reopened.Annotations()
	if err != nil || len(annotations) != 1 {
		t.Fatalf("reopened annotations: %v, %v", annotations, err)
	}
	doc.Close()
	if _, err := doc.AddTextWatermark(0, "bad", [4]float64{0, 0, 10, 10}, 5); err == nil {
		t.Fatal("expected closed document")
	}
}

func TestTextStampAnnotationRoundTrip(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("base", 80, 50); err != nil {
		t.Fatal(err)
	}
	style := DefaultTextWatermarkStyle()
	style.Color = [3]uint8{180, 20, 30}
	style.Opacity = 192
	id, err := doc.AddStyledTextStampAnnotation(0, "批准 & <同意>", [4]float64{10, 20, 60, 10}, 5, style)
	if err != nil || id == 0 {
		t.Fatalf("add text stamp: %d, %v", id, err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 || annotations[0].Type != "Stamp" || annotations[0].Text[0] != "批准 & <同意>" {
		t.Fatalf("text stamp: %v, %v", annotations, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "#b4141e") {
		t.Fatalf("text stamp SVG: %v", err)
	}
	if _, err := doc.AddTextStampAnnotation(0, "", [4]float64{10, 20, 60, 10}, 5); err == nil {
		t.Fatal("expected empty stamp rejection")
	}
	path := filepath.Join(t.TempDir(), "text-stamp.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	annotations, err = reopened.Annotations()
	if err != nil || len(annotations) != 1 || annotations[0].Type != "Stamp" {
		t.Fatalf("reopened text stamp: %v, %v", annotations, err)
	}
}

func TestStyledTextWatermarkRoundTrip(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("body", 80, 50); err != nil {
		t.Fatal(err)
	}
	style := DefaultTextWatermarkStyle()
	style.Color = [3]uint8{255, 0, 0}
	style.Opacity = 96
	style.AngleDegrees = -30
	style.Bold, style.Italic = true, true
	id, err := doc.AddStyledTextWatermark(0, "DRAFT", [4]float64{10, 10, 50, 20}, 5, style)
	if err != nil || id == 0 {
		t.Fatalf("add styled watermark: %d, %v", id, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "#ff0000") || !strings.Contains(svg, "fill-opacity=\"0.376") {
		t.Fatalf("styled watermark SVG: %v, %v", svg, err)
	}
	style.FontID = "missing"
	if _, err := doc.AddStyledTextWatermark(0, "bad", [4]float64{10, 10, 50, 20}, 5, style); err == nil {
		t.Fatal("expected missing watermark font")
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 {
		t.Fatalf("annotations after failed edit: %v, %v", annotations, err)
	}
	path := filepath.Join(t.TempDir(), "styled-watermark.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	savedSVG, err := reopened.PageSVG(0)
	if err != nil || savedSVG != svg {
		t.Fatalf("saved watermark SVG differs: %v", err)
	}
}

func TestTextWatermarkToAllPagesUsesEachPageBox(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBlankPage(60, 50); err != nil {
		t.Fatal(err)
	}
	style := DefaultTextWatermarkStyle()
	style.Color = [3]uint8{0, 0, 255}
	style.Opacity = 128
	spacing := [2]float64{20, 15}
	style.TileSpacing = &spacing
	ids, err := doc.AddTextWatermarkToAllPages("COPY", 5, style)
	if err != nil || len(ids) != 2 {
		t.Fatalf("watermark IDs: %v, %v", ids, err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 2 ||
		annotations[0].ID != fmt.Sprint(ids[0]) || annotations[1].ID != fmt.Sprint(ids[1]) ||
		!slices.Equal(annotations[0].AppearanceBox, []float64{0, 0, 40, 30}) ||
		!slices.Equal(annotations[1].AppearanceBox, []float64{0, 0, 60, 50}) ||
		len(annotations[0].Text) != 4 || len(annotations[1].Text) != 12 {
		t.Fatalf("watermark annotations: %v, %v", annotations, err)
	}
	if _, err := doc.AddTextWatermarkToAllPages("", 5, style); err == nil {
		t.Fatal("expected empty watermark error")
	}
	badSpacing := [2]float64{0, 15}
	style.TileSpacing = &badSpacing
	if _, err := doc.AddTextWatermarkToAllPages("bad", 5, style); err == nil {
		t.Fatal("expected invalid tile spacing")
	}
	if annotations, err := doc.Annotations(); err != nil || len(annotations) != 2 {
		t.Fatalf("failed edit changed annotations: %v, %v", annotations, err)
	}
	path := filepath.Join(t.TempDir(), "all-page-watermark.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	svg, err := reopened.PageSVG(1)
	if err != nil || !strings.Contains(svg, "#0000ff") {
		t.Fatalf("saved watermark SVG: %v, %v", svg, err)
	}
}

func TestImageWatermarkFromBytesAndExistingResource(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "发票示例.ofd")
	invoice, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	image, err := invoice.PageImageData(0, 0)
	invoice.Close()
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	first, err := doc.AddImageWatermarkFromBytes(0, image, "png", [4]float64{5, 5, 20, 15}, 128)
	if err != nil {
		t.Fatal(err)
	}
	resources, err := doc.DocumentMultimediaResources(0)
	if err != nil || len(resources) != 1 {
		t.Fatalf("watermark resource: %v, %v", resources, err)
	}
	second, err := doc.AddImageWatermark(1, resources[0].ID, [4]float64{5, 5, 20, 15}, 255)
	if err != nil || first == second {
		t.Fatalf("reused image watermark: %d, %v", second, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "<image") {
		t.Fatalf("image watermark SVG: %v, %v", svg, err)
	}
	if _, err := doc.AddImageWatermarkFromBytes(0, []byte("bad"), "png", [4]float64{5, 5, 20, 15}, 255); err == nil {
		t.Fatal("expected invalid image data")
	}
	if _, err := doc.AddImageWatermark(-1, resources[0].ID, [4]float64{5, 5, 20, 15}, 255); err == nil {
		t.Fatal("expected negative page index error")
	}
	if _, err := doc.AddImageWatermark(0, "missing", [4]float64{5, 5, 20, 15}, 255); err == nil {
		t.Fatal("expected missing resource error")
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 2 {
		t.Fatalf("failed edit changed annotations: %v, %v", annotations, err)
	}
	path := filepath.Join(t.TempDir(), "image-watermark.ofd")
	if err := doc.Save(path); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	svg, err = reopened.PageSVG(1)
	if err != nil || !strings.Contains(svg, "<image") {
		t.Fatalf("saved image watermark SVG: %v, %v", svg, err)
	}
	doc.Close()
	if _, err := doc.AddImageWatermark(0, resources[0].ID, [4]float64{5, 5, 20, 15}, 255); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestImageStampAnnotationFromBytesAndResource(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "发票示例.ofd")
	invoice, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	image, err := invoice.PageImageData(0, 0)
	invoice.Close()
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	for range 2 {
		if err := doc.AddBlankPage(40, 30); err != nil {
			t.Fatal(err)
		}
	}
	first, err := doc.AddImageStampAnnotationFromBytes(0, image, "png", [4]float64{5, 5, 20, 15}, 128)
	if err != nil {
		t.Fatal(err)
	}
	resources, err := doc.DocumentMultimediaResources(0)
	if err != nil || len(resources) != 1 {
		t.Fatalf("stamp resources: %v, %v", resources, err)
	}
	second, err := doc.AddImageStampAnnotation(1, resources[0].ID, [4]float64{5, 5, 20, 15}, 255)
	if err != nil || first == second {
		t.Fatalf("reused stamp: %d, %v", second, err)
	}
	marks, err := doc.Annotations()
	if err != nil || len(marks) != 2 || marks[0].Type != "Stamp" || marks[1].Type != "Stamp" {
		t.Fatalf("stamp annotations: %v, %v", marks, err)
	}
	if _, err := doc.AddImageStampAnnotationFromBytes(0, []byte("bad"), "png", [4]float64{5, 5, 20, 15}, 255); err == nil {
		t.Fatal("expected invalid image data")
	}
	file := filepath.Join(t.TempDir(), "stamp.ofd")
	if err := doc.Save(file); err != nil {
		t.Fatal(err)
	}
	reopened, err := Open(file)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	svg, err := reopened.PageSVG(1)
	if err != nil || !strings.Contains(svg, "<image") {
		t.Fatalf("saved image stamp SVG: %v, %v", svg, err)
	}
}

func TestImageWatermarkToAllPagesReusesResources(t *testing.T) {
	fixture := filepath.Join("..", "..", "..", "..", "ofdrw-reader", "src", "test", "resources", "发票示例.ofd")
	invoice, err := Open(fixture)
	if err != nil {
		t.Fatal(err)
	}
	image, err := invoice.PageImageData(0, 0)
	invoice.Close()
	if err != nil {
		t.Fatal(err)
	}
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	for range 2 {
		if err := doc.AddBlankPage(40, 30); err != nil {
			t.Fatal(err)
		}
	}
	if index, err := doc.AppendDocumentBody(); err != nil || index != 1 {
		t.Fatalf("append document body: %d, %v", index, err)
	}
	if err := doc.AppendBlankPage(1, 40, 30); err != nil {
		t.Fatal(err)
	}
	ids, err := doc.AddImageWatermarkToAllPages(image, "png", [4]float64{5, 5, 20, 15}, 128)
	if err != nil || len(ids) != 3 || ids[0] == ids[1] || ids[1] == ids[2] {
		t.Fatalf("watermark IDs: %v, %v", ids, err)
	}
	for body := range 2 {
		resources, err := doc.DocumentMultimediaResources(body)
		if err != nil || len(resources) != 1 {
			t.Fatalf("body %d resources: %v, %v", body, resources, err)
		}
	}
	if _, err := doc.AddImageWatermarkToAllPages([]byte("bad"), "png", [4]float64{5, 5, 20, 15}, 128); err == nil {
		t.Fatal("expected invalid image error")
	}
	if _, err := doc.AddImageWatermarkToAllPages(image, "png", [4]float64{5, 5, math.NaN(), 15}, 128); err == nil {
		t.Fatal("expected invalid JSON number error")
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 3 {
		t.Fatalf("failed edit changed annotations: %v, %v", annotations, err)
	}
	for page := range 3 {
		svg, err := doc.PageSVG(page)
		if err != nil || !strings.Contains(svg, "<image") {
			t.Fatalf("page %d watermark SVG: %v, %v", page, svg, err)
		}
	}
	doc.Close()
	if _, err := doc.AddImageWatermarkToAllPages(image, "png", [4]float64{5, 5, 20, 15}, 128); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestReportsMissingAnnotationFont(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddTextPage("body", 30, 20); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddTextWatermark(0, "mark", [4]float64{1, 1, 10, 5}, 4); err != nil {
		t.Fatal(err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 {
		t.Fatalf("annotations: %v, %v", annotations, err)
	}
	source := annotations[0].SourcePath
	xml, err := doc.Entry(source)
	if err != nil {
		t.Fatal(err)
	}
	font := regexp.MustCompile(`Font="[^"]+"`)
	if !font.Match(xml) {
		t.Fatal("annotation has no font")
	}
	if err := doc.SetEntry(source, font.ReplaceAll(xml, []byte(`Font="999999"`))); err != nil {
		t.Fatal(err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	for _, issue := range issues {
		if issue.Path == source && issue.Code == "missing_font" {
			return
		}
	}
	t.Fatalf("missing annotation font issue: %+v", issues)
}

func TestAddsPathAnnotationToBlankPage(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(30, 20); err != nil {
		t.Fatal(err)
	}
	red := [3]uint8{255, 0, 0}
	id, err := doc.AddPathAnnotation(0, "M 0 0 L 10 5", [4]float64{2, 3, 10, 5}, PathStyle{Stroke: true, StrokeColor: &red})
	if err != nil || id <= 0 {
		t.Fatalf("add path annotation: %d, %v", id, err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 || annotations[0].Type != "Path" {
		t.Fatalf("path annotations: %+v, %v", annotations, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "#ff0000") {
		t.Fatalf("path annotation SVG: %v, %v", svg, err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("path annotation references: %+v, %v", issues, err)
	}
	shownPDF, err := doc.ToPDF()
	if err != nil {
		t.Fatal(err)
	}
	annotationID := strconv.Itoa(id)
	if err := doc.SetAnnotationPrint(0, annotationID, false); err != nil {
		t.Fatal(err)
	}
	svg, err = doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "#ff0000") {
		t.Fatalf("nonprinting screen annotation: %v, %v", svg, err)
	}
	hiddenPDF, err := doc.ToPDF()
	if err != nil || bytes.Equal(hiddenPDF, shownPDF) {
		t.Fatalf("nonprinting PDF unchanged: %v", err)
	}
	if err := doc.SetAnnotationVisible(0, annotationID, false); err != nil {
		t.Fatal(err)
	}
	svg, err = doc.PageSVG(0)
	if err != nil || strings.Contains(svg, "data-ofd-annotation") {
		t.Fatalf("hidden annotation still visible: %v, %v", svg, err)
	}
	if err := doc.SetAnnotationVisible(0, annotationID, true); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationPrint(0, annotationID, true); err != nil {
		t.Fatal(err)
	}
	for _, flag := range []string{"NoZoom", "NoRotate", "ReadOnly"} {
		if err := doc.SetAnnotationFlag(0, annotationID, flag, true); err != nil {
			t.Fatal(err)
		}
	}
	annotations, err = doc.Annotations()
	if err != nil || annotations[0].Attributes["NoRotate"] != "true" {
		t.Fatalf("annotation flags: %+v, %v", annotations, err)
	}
	if err := doc.SetAnnotationFlag(0, annotationID, "ReadOnly", false); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationFlag(0, annotationID, "Unknown", true); err == nil {
		t.Fatal("expected unknown annotation flag error")
	}
	if err := doc.SetAnnotationFlag(-1, annotationID, "NoZoom", true); err == nil {
		t.Fatal("expected negative page index error")
	}
	if _, err := doc.AddPathAnnotation(0, "", [4]float64{2, 3, 10, 5}, PathStyle{}); err == nil {
		t.Fatal("expected empty path rejection")
	}
}

func TestAddsTextNoteAnnotation(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(30, 20); err != nil {
		t.Fatal(err)
	}
	color := [3]uint8{0, 100, 200}
	opacity := uint8(128)
	id, err := doc.AddTextNoteAnnotation(0, "审核 & <通过>", [4]float64{2, 3, 5, 5}, TextNoteOptions{
		Color: &color, Opacity: &opacity, Icon: "Comment", Open: true,
	})
	if err != nil || id == 0 {
		t.Fatalf("add text note: %d, %v", id, err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 || annotations[0].Type != "Path" || annotations[0].Subtype == nil || *annotations[0].Subtype != "Text" || annotations[0].Remark == nil || *annotations[0].Remark != "审核 & <通过>" || annotations[0].Parameters["PDFTextIcon"] != "Comment" {
		t.Fatalf("text note: %v, %v", annotations, err)
	}
	if _, err := doc.ToPDF(); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddTextNoteAnnotation(0, "bad", [4]float64{2, 3, 5, 5}, TextNoteOptions{Icon: "Unknown"}); err == nil {
		t.Fatal("expected invalid icon")
	}
}

func TestAddsMultiPathAnnotation(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	red := [3]uint8{255, 0, 0}
	blue := [3]uint8{0, 0, 255}
	paths := []AnnotationPath{
		{Commands: "M 0 0 L 10 10", Boundary: [4]float64{0, 0, 10, 10}, PathStyle: PathStyle{Stroke: true, StrokeColor: &red}},
		{Commands: "M 0 10 L 10 0", Boundary: [4]float64{10, 0, 10, 10}, PathStyle: PathStyle{Stroke: true, StrokeColor: &blue, DashSequence: []float64{2, 1}}},
	}
	id, err := doc.AddMultiPathAnnotation(0, [4]float64{3, 4, 25, 15}, paths)
	if err != nil || id == 0 {
		t.Fatalf("add multi-path annotation: %d, %v", id, err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 || annotations[0].Type != "Path" {
		t.Fatalf("annotations: %v, %v", annotations, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "#ff0000") || !strings.Contains(svg, "#0000ff") {
		t.Fatalf("multi-path SVG: %v", err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	invalid := append([]AnnotationPath{}, paths...)
	invalid = append(invalid, AnnotationPath{Commands: "bad", Boundary: [4]float64{0, 0, 10, 10}})
	if _, err := doc.AddMultiPathAnnotation(0, [4]float64{3, 4, 25, 15}, invalid); err == nil {
		t.Fatal("expected invalid path")
	}
	if _, err := doc.AddMultiPathAnnotation(0, [4]float64{3, 4, 25, 15}, nil); err == nil {
		t.Fatal("expected empty paths to fail")
	}
	if _, err := doc.AddMultiPathAnnotation(-1, [4]float64{3, 4, 25, 15}, paths); err == nil {
		t.Fatal("expected negative page index to fail")
	}
	if _, err := doc.AddMultiPathAnnotation(0, [4]float64{math.NaN(), 4, 25, 15}, paths); err == nil {
		t.Fatal("expected non-finite boundary to fail")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid edit changed document: %v", err)
	}
	doc.Close()
	if _, err := doc.AddMultiPathAnnotation(0, [4]float64{3, 4, 25, 15}, paths); err == nil {
		t.Fatal("expected closed document to fail")
	}
}

func TestAddsMixedAnnotation(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	png, err := doc.PagePNG(0, 25.4)
	if err != nil {
		t.Fatal(err)
	}
	imageID, err := doc.AddMultimediaResource(0, "Image", "PNG", png)
	if err != nil {
		t.Fatal(err)
	}
	red := [3]uint8{255, 0, 0}
	objects := []AnnotationGraphic{
		{Kind: "path", Commands: "M 0 0 L 9 9", Boundary: [4]float64{0, 0, 10, 10}, PathStyle: PathStyle{Stroke: true, StrokeColor: &red}},
		{Kind: "text", Text: "Mixed", Boundary: [4]float64{10, 0, 12, 10}, Size: 4},
		{Kind: "image", ResourceID: strconv.Itoa(imageID), Boundary: [4]float64{22, 0, 8, 8}},
	}
	id, err := doc.AddMixedAnnotation(0, [4]float64{2, 3, 32, 12}, objects)
	if err != nil || id == 0 {
		t.Fatalf("add mixed annotation: %d, %v", id, err)
	}
	appearance, err := doc.AnnotationObjects(0, strconv.Itoa(id))
	if err != nil || len(appearance) != 3 || appearance[0].Kind != "PathObject" ||
		appearance[1].Kind != "TextObject" || appearance[2].Kind != "ImageObject" {
		t.Fatalf("annotation objects: %v, %v", appearance, err)
	}
	if len(appearance[1].Text) != 1 || appearance[1].Text[0] != "Mixed" ||
		appearance[2].ResourceID == nil || *appearance[2].ResourceID != strconv.Itoa(imageID) {
		t.Fatalf("annotation object content: %v", appearance)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "Mixed") || !strings.Contains(svg, "#ff0000") {
		t.Fatalf("mixed SVG: %v", err)
	}
	stampID, err := doc.AddMixedStampAnnotation(0, [4]float64{2, 15, 32, 12}, objects)
	if err != nil || stampID == 0 {
		t.Fatalf("add mixed stamp: %d, %v", stampID, err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 2 || annotations[1].Type != "Stamp" {
		t.Fatalf("mixed stamp annotations: %v, %v", annotations, err)
	}
	if _, err := doc.AnnotationObjects(0, "missing"); err == nil {
		t.Fatal("expected missing annotation to fail")
	}
	if _, err := doc.AnnotationObjects(-1, strconv.Itoa(id)); err == nil {
		t.Fatal("expected negative page index to fail")
	}
	if _, err := doc.AnnotationObjects(0, "bad\x00id"); err == nil {
		t.Fatal("expected NUL annotation ID to fail")
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	reopened, err := FromBytes(before)
	if err != nil {
		t.Fatal(err)
	}
	reopenedObjects, err := reopened.AnnotationObjects(0, strconv.Itoa(id))
	if err != nil || !reflect.DeepEqual(reopenedObjects, appearance) {
		t.Fatalf("reopened annotation objects: %v, %v", reopenedObjects, err)
	}
	reopened.Close()
	if _, err := reopened.AnnotationObjects(0, strconv.Itoa(id)); err == nil {
		t.Fatal("expected closed document to fail")
	}
	invalid := append(append([]AnnotationGraphic{}, objects...), AnnotationGraphic{Kind: "image", ResourceID: "999999", Boundary: [4]float64{0, 0, 4, 4}})
	if _, err := doc.AddMixedAnnotation(0, [4]float64{2, 3, 32, 12}, invalid); err == nil {
		t.Fatal("expected missing image to fail")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid edit changed document: %v", err)
	}
	pathID := *appearance[0].ID
	newBoundary := [4]float64{1, 1, 9, 9}
	newCTM := [6]float64{1, 0, 0, 1, 2, 3}
	visible := false
	alpha := uint8(128)
	if err := doc.UpdateAnnotationObject(0, strconv.Itoa(id), pathID, AnnotationObjectUpdate{
		Boundary: &newBoundary, CTM: &newCTM, Visible: &visible, Alpha: &alpha,
	}); err != nil {
		t.Fatal(err)
	}
	edited, err := doc.AnnotationObjects(0, strconv.Itoa(id))
	if err != nil || !reflect.DeepEqual(edited[0].Boundary, []float64{1, 1, 9, 9}) || edited[0].Visible || edited[0].Alpha != 128 {
		t.Fatalf("edited annotation object: %v, %v", edited, err)
	}
	if err := doc.UpdateAnnotationObject(0, strconv.Itoa(id), pathID, AnnotationObjectUpdate{ClearCTM: true}); err != nil {
		t.Fatal(err)
	}
	edited, err = doc.AnnotationObjects(0, strconv.Itoa(id))
	if err != nil || edited[0].CTM != nil {
		t.Fatalf("CTM was not cleared: %v, %v", edited, err)
	}
	before, err = doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	zeroBoundary := [4]float64{0, 0, 0, 9}
	if err := doc.UpdateAnnotationObject(0, strconv.Itoa(id), pathID, AnnotationObjectUpdate{Boundary: &zeroBoundary}); err == nil {
		t.Fatal("expected invalid boundary")
	}
	if err := doc.UpdateAnnotationObject(0, strconv.Itoa(id), pathID, AnnotationObjectUpdate{CTM: &newCTM, ClearCTM: true}); err == nil {
		t.Fatal("expected conflicting CTM")
	}
	after, err = doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid annotation update changed document: %v", err)
	}
	if err := doc.RemoveAnnotationObject(0, strconv.Itoa(id), *appearance[2].ID); err != nil {
		t.Fatal(err)
	}
	edited, err = doc.AnnotationObjects(0, strconv.Itoa(id))
	if err != nil || len(edited) != 2 {
		t.Fatalf("removed annotation object: %v, %v", edited, err)
	}
	if err := doc.RemoveAnnotationObject(0, strconv.Itoa(id), *appearance[2].ID); err == nil {
		t.Fatal("expected missing object to fail")
	}
	before, err = doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	badBoundary := [4]float64{math.NaN(), 0, 9, 9}
	for _, test := range []struct {
		name string
		call func() error
	}{
		{"empty update", func() error { return doc.UpdateAnnotationObject(0, strconv.Itoa(id), pathID, AnnotationObjectUpdate{}) }},
		{"negative update page", func() error {
			return doc.UpdateAnnotationObject(-1, strconv.Itoa(id), pathID, AnnotationObjectUpdate{Alpha: &alpha})
		}},
		{"NUL annotation ID", func() error {
			return doc.UpdateAnnotationObject(0, "bad\x00id", pathID, AnnotationObjectUpdate{Alpha: &alpha})
		}},
		{"NUL object ID", func() error {
			return doc.UpdateAnnotationObject(0, strconv.Itoa(id), "bad\x00id", AnnotationObjectUpdate{Alpha: &alpha})
		}},
		{"nonfinite boundary", func() error {
			return doc.UpdateAnnotationObject(0, strconv.Itoa(id), pathID, AnnotationObjectUpdate{Boundary: &badBoundary})
		}},
		{"missing update object", func() error {
			return doc.UpdateAnnotationObject(0, strconv.Itoa(id), "missing", AnnotationObjectUpdate{Alpha: &alpha})
		}},
		{"negative remove page", func() error { return doc.RemoveAnnotationObject(-1, strconv.Itoa(id), pathID) }},
		{"NUL remove annotation ID", func() error { return doc.RemoveAnnotationObject(0, "bad\x00id", pathID) }},
		{"NUL remove object ID", func() error { return doc.RemoveAnnotationObject(0, strconv.Itoa(id), "bad\x00id") }},
	} {
		if err := test.call(); err == nil {
			t.Fatalf("expected %s to fail", test.name)
		}
	}
	after, err = doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid object edits changed document: %v", err)
	}
	doc.Close()
	if err := doc.UpdateAnnotationObject(0, strconv.Itoa(id), pathID, AnnotationObjectUpdate{Alpha: &alpha}); err == nil {
		t.Fatal("expected closed update to fail")
	}
	if err := doc.RemoveAnnotationObject(0, strconv.Itoa(id), pathID); err == nil {
		t.Fatal("expected closed remove to fail")
	}
}

func TestAppendsAnnotationObjects(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddMixedStampAnnotation(0, [4]float64{1, 1, 20, 10}, []AnnotationGraphic{
		{Kind: "text", Text: "first", Boundary: [4]float64{0, 0, 8, 5}, Size: 3},
	})
	if err != nil {
		t.Fatal(err)
	}
	annotationID := strconv.Itoa(id)
	ids, err := doc.AppendAnnotationObjects(0, annotationID, []AnnotationGraphic{
		{Kind: "path", Commands: "M 0 0 L 5 5", Boundary: [4]float64{0, 0, 5, 5}},
		{Kind: "text", Text: "second", Boundary: [4]float64{5, 0, 8, 5}, Size: 3},
	})
	if err != nil || len(ids) != 2 {
		t.Fatalf("append annotation objects: %v, %v", ids, err)
	}
	objects, err := doc.AnnotationObjects(0, annotationID)
	if err != nil || len(objects) != 3 || *objects[1].ID != ids[0] || *objects[2].ID != ids[1] {
		t.Fatalf("annotation objects after append: %v, %v", objects, err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 {
		t.Fatalf("annotations after append: %v, %v", annotations, err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	reopened, err := FromBytes(before)
	if err != nil {
		t.Fatal(err)
	}
	reopenedObjects, err := reopened.AnnotationObjects(0, annotationID)
	if err != nil || !reflect.DeepEqual(reopenedObjects, objects) {
		t.Fatalf("reopened annotation objects: %v, %v", reopenedObjects, err)
	}
	reopened.Close()
	for _, test := range []struct {
		name string
		call func() error
	}{
		{"missing image", func() error {
			_, err := doc.AppendAnnotationObjects(0, annotationID, []AnnotationGraphic{
				{Kind: "image", ResourceID: "999999", Boundary: [4]float64{0, 0, 5, 5}},
			})
			return err
		}},
		{"empty objects", func() error { _, err := doc.AppendAnnotationObjects(0, annotationID, nil); return err }},
		{"negative page", func() error { _, err := doc.AppendAnnotationObjects(-1, annotationID, nil); return err }},
		{"NUL annotation ID", func() error { _, err := doc.AppendAnnotationObjects(0, "bad\x00id", nil); return err }},
	} {
		t.Run(test.name, func(t *testing.T) {
			if err := test.call(); err == nil {
				t.Fatal("expected append to fail")
			}
		})
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed append changed document: %v", err)
	}
}

func TestCompositeAnnotationGraphics(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	red := [3]uint8{255, 0, 0}
	if _, err := doc.AddCompositePathObject(0, "M 0 0 L 8 0 L 8 8 C", [4]float64{0, 0, 8, 8},
		PathStyle{Fill: true, FillColor: &red}); err != nil {
		t.Fatal(err)
	}
	resources, err := doc.PageCompositeObjects(0)
	if err != nil || len(resources) != 1 {
		t.Fatalf("composite resources: %v, %v", resources, err)
	}
	alpha := uint8(180)
	graphic := AnnotationGraphic{
		Kind: "composite", ResourceID: resources[0].ResourceID,
		Boundary: [4]float64{0, 0, 8, 8}, CTM: &[6]float64{1, 0, 0, 1, 1, 2},
		PathStyle: PathStyle{Alpha: &alpha},
	}
	id, err := doc.AddMixedStampAnnotation(0, [4]float64{2, 2, 20, 12}, []AnnotationGraphic{graphic})
	if err != nil {
		t.Fatal(err)
	}
	annotationID := strconv.Itoa(id)
	ids, err := doc.AppendAnnotationObjects(0, annotationID, []AnnotationGraphic{graphic})
	if err != nil || len(ids) != 1 {
		t.Fatalf("append composite: %v, %v", ids, err)
	}
	objects, err := doc.AnnotationObjects(0, annotationID)
	if err != nil || len(objects) != 2 || objects[0].Kind != "CompositeObject" ||
		objects[0].ResourceID == nil || *objects[0].ResourceID != resources[0].ResourceID ||
		objects[0].Alpha != 180 || objects[1].ID == nil || *objects[1].ID != ids[0] {
		t.Fatalf("composite annotation objects: %v, %v", objects, err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AppendAnnotationObjects(0, annotationID, []AnnotationGraphic{
		{Kind: "composite", ResourceID: "999999", Boundary: [4]float64{0, 0, 8, 8}},
	}); err == nil {
		t.Fatal("expected missing composite resource to fail")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid composite changed document: %v", err)
	}
}

func TestSetAnnotationPathData(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddMixedAnnotation(0, [4]float64{2, 2, 20, 12}, []AnnotationGraphic{
		{Kind: "path", Commands: "M 0 0 L 8 8", Boundary: [4]float64{0, 0, 8, 8}},
		{Kind: "text", Text: "keep", Boundary: [4]float64{8, 0, 8, 8}, Size: 3},
	})
	if err != nil {
		t.Fatal(err)
	}
	annotationID := strconv.Itoa(id)
	objects, err := doc.AnnotationObjects(0, annotationID)
	if err != nil || len(objects) != 2 || objects[0].ID == nil || objects[1].ID == nil {
		t.Fatalf("annotation objects: %v, %v", objects, err)
	}
	if err := doc.SetAnnotationPathData(0, annotationID, *objects[0].ID, "M 0 8 L 8 0"); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationPathStrokeStyle(0, annotationID, *objects[0].ID, PathStrokeStyle{
		LineWidth: 1.25, Cap: "round", DashSequence: []float64{1, 2, 3},
	}); err != nil {
		t.Fatal(err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	reopened, err := FromBytes(before)
	if err != nil {
		t.Fatal(err)
	}
	edited, err := reopened.AnnotationObjects(0, annotationID)
	if err != nil || edited[0].Commands == nil || *edited[0].Commands != "M 0 8 L 8 0" ||
		!reflect.DeepEqual(edited[1], objects[1]) {
		t.Fatalf("edited annotation path: %v, %v", edited, err)
	}
	reopened.Close()
	for _, test := range []struct {
		name string
		call func() error
	}{
		{"invalid commands", func() error { return doc.SetAnnotationPathData(0, annotationID, *objects[0].ID, "invalid") }},
		{"text object", func() error { return doc.SetAnnotationPathData(0, annotationID, *objects[1].ID, "M 0 0 L 8 8") }},
		{"negative page", func() error { return doc.SetAnnotationPathData(-1, annotationID, *objects[0].ID, "M 0 0 L 8 8") }},
		{"NUL object ID", func() error { return doc.SetAnnotationPathData(0, annotationID, "bad\x00id", "M 0 0 L 8 8") }},
		{"invalid stroke", func() error {
			return doc.SetAnnotationPathStrokeStyle(0, annotationID, *objects[0].ID, PathStrokeStyle{LineWidth: -1})
		}},
		{"text stroke", func() error {
			return doc.SetAnnotationPathStrokeStyle(0, annotationID, *objects[1].ID, PathStrokeStyle{LineWidth: 1})
		}},
	} {
		t.Run(test.name, func(t *testing.T) {
			if err := test.call(); err == nil {
				t.Fatal("expected path edit to fail")
			}
		})
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed edit changed document: %v", err)
	}
}

func TestSetAnnotationTextCode(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddMixedAnnotation(0, [4]float64{2, 2, 25, 12}, []AnnotationGraphic{
		{Kind: "text", Text: "AB", Boundary: [4]float64{0, 0, 15, 8}, Size: 3},
		{Kind: "path", Commands: "M 0 0 L 8 8", Boundary: [4]float64{15, 0, 8, 8}},
	})
	if err != nil {
		t.Fatal(err)
	}
	annotationID := strconv.Itoa(id)
	objects, err := doc.AnnotationObjects(0, annotationID)
	if err != nil || len(objects) != 2 || objects[0].ID == nil || objects[1].ID == nil {
		t.Fatalf("annotation objects: %v, %v", objects, err)
	}
	if err := doc.SetAnnotationTextCode(0, annotationID, *objects[0].ID, 0, "A & <B>"); err != nil {
		t.Fatal(err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	reopened, err := FromBytes(before)
	if err != nil {
		t.Fatal(err)
	}
	edited, err := reopened.AnnotationObjects(0, annotationID)
	if err != nil || len(edited) != 2 || !reflect.DeepEqual(edited[0].Text, []string{"A & <B>"}) ||
		!reflect.DeepEqual(edited[1], objects[1]) {
		t.Fatalf("edited annotation text: %v, %v", edited, err)
	}
	reopened.Close()
	for _, test := range []struct {
		name string
		call func() error
	}{
		{"wrong type", func() error { return doc.SetAnnotationTextCode(0, annotationID, *objects[1].ID, 0, "wrong") }},
		{"invalid code index", func() error { return doc.SetAnnotationTextCode(0, annotationID, *objects[0].ID, 1, "wrong") }},
		{"negative code index", func() error { return doc.SetAnnotationTextCode(0, annotationID, *objects[0].ID, -1, "wrong") }},
	} {
		t.Run(test.name, func(t *testing.T) {
			if err := test.call(); err == nil {
				t.Fatal("expected text edit to fail")
			}
		})
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed text edit changed document: %v", err)
	}
}

func TestSetAnnotationTextFont(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	fontPath := filepath.Join("..", "..", "..", "crates", "ofdrw", "tests", "fixtures", "dejavu-serif.ttf")
	font, err := os.ReadFile(fontPath)
	if err != nil {
		t.Fatal(err)
	}
	fontID, err := doc.AddEmbeddedFont("DejaVu Serif", font)
	if err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddMixedAnnotation(0, [4]float64{2, 2, 20, 10}, []AnnotationGraphic{
		{Kind: "text", Text: "AB", Boundary: [4]float64{0, 0, 15, 8}, Size: 3},
	})
	if err != nil {
		t.Fatal(err)
	}
	annotationID := strconv.Itoa(id)
	objects, err := doc.AnnotationObjects(0, annotationID)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("objects: %v, %v", objects, err)
	}
	if err := doc.SetAnnotationTextFont(0, annotationID, *objects[0].ID, strconv.Itoa(fontID)); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.AnnotationObjects(0, annotationID)
	if err != nil || objects[0].FontID == nil || *objects[0].FontID != strconv.Itoa(fontID) {
		t.Fatalf("font edit: %v, %v", objects, err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationTextFont(0, annotationID, *objects[0].ID, "999999"); err == nil {
		t.Fatal("missing font accepted")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed edit changed document: %v", err)
	}
}

func TestRemoveAnnotationTextCode(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddMixedAnnotation(0, [4]float64{2, 2, 20, 10}, []AnnotationGraphic{
		{Kind: "text", Text: "AB", Boundary: [4]float64{0, 0, 15, 8}, Size: 3},
	})
	if err != nil {
		t.Fatal(err)
	}
	annotationID := strconv.Itoa(id)
	objects, err := doc.AnnotationObjects(0, annotationID)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("objects: %v, %v", objects, err)
	}
	annotations, err := doc.Annotations()
	if err != nil {
		t.Fatal(err)
	}
	path := annotations[0].SourcePath
	source, err := doc.Entry(path)
	if err != nil {
		t.Fatal(err)
	}
	modified := bytes.Replace(source, []byte("</ofd:TextObject>"),
		[]byte("<ofd:TextCode>CD</ofd:TextCode></ofd:TextObject>"), 1)
	if bytes.Equal(source, modified) {
		t.Fatal("TextObject closing tag missing")
	}
	if err := doc.SetEntry(path, modified); err != nil {
		t.Fatal(err)
	}
	if err := doc.RemoveAnnotationTextCode(0, annotationID, *objects[0].ID, 0); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.AnnotationObjects(0, annotationID)
	if err != nil || !reflect.DeepEqual(objects[0].Text, []string{"CD"}) {
		t.Fatalf("text: %v, %v", objects, err)
	}
	updated, err := doc.Entry(path)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(updated, []byte(`<ofd:TextCode X="0" Y="3">CD</ofd:TextCode>`)) {
		t.Fatalf("inherited position missing: %s", updated)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.RemoveAnnotationTextCode(0, annotationID, *objects[0].ID, 0); err == nil {
		t.Fatal("removed last text code")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed edit changed document: %v", err)
	}
}

func TestAppendAnnotationTextCode(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddMixedAnnotation(0, [4]float64{2, 2, 20, 10}, []AnnotationGraphic{
		{Kind: "text", Text: "AB", Boundary: [4]float64{0, 0, 15, 8}, Size: 3},
	})
	if err != nil {
		t.Fatal(err)
	}
	annotationID := strconv.Itoa(id)
	objects, err := doc.AnnotationObjects(0, annotationID)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("objects: %v, %v", objects, err)
	}
	if err := doc.AppendAnnotationTextCode(0, annotationID, *objects[0].ID, "C&D",
		TextCodeLayout{DeltaX: []float64{2, 2.5}}); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.AnnotationObjects(0, annotationID)
	if err != nil || !reflect.DeepEqual(objects[0].Text, []string{"AB", "C&D"}) {
		t.Fatalf("text: %v, %v", objects, err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.AppendAnnotationTextCode(0, annotationID, *objects[0].ID, "XY",
		TextCodeLayout{DeltaX: []float64{1, 2}}); err == nil {
		t.Fatal("accepted invalid advances")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed edit changed document: %v", err)
	}
}

func TestAnnotationTextEditsPreserveEmbeddedNUL(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddMixedAnnotation(0, [4]float64{2, 2, 20, 10}, []AnnotationGraphic{
		{Kind: "text", Text: "AB", Boundary: [4]float64{0, 0, 15, 8}, Size: 3},
	})
	if err != nil {
		t.Fatal(err)
	}
	annotationID := strconv.Itoa(id)
	objects, err := doc.AnnotationObjects(0, annotationID)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("objects: %v, %v", objects, err)
	}
	objectID := *objects[0].ID
	if err := doc.SetAnnotationTextCode(0, annotationID, objectID, 0, "A\x00B"); err != nil {
		t.Fatal(err)
	}
	if err := doc.AppendAnnotationTextCode(0, annotationID, objectID, "C\x00D", TextCodeLayout{}); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.AnnotationObjects(0, annotationID)
	if err != nil || !reflect.DeepEqual(objects[0].Text, []string{"A\x00B", "C\x00D"}) {
		t.Fatalf("text: %v, %v", objects, err)
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
	objects, err = reopened.AnnotationObjects(0, annotationID)
	if err != nil || !reflect.DeepEqual(objects[0].Text, []string{"A\x00B", "C\x00D"}) {
		t.Fatalf("reopened text: %v, %v", objects, err)
	}
}

func TestSetAnnotationTextCodeLayout(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddMixedAnnotation(0, [4]float64{2, 2, 20, 10}, []AnnotationGraphic{
		{Kind: "text", Text: "AB", Boundary: [4]float64{0, 0, 15, 8}, Size: 3},
	})
	if err != nil {
		t.Fatal(err)
	}
	annotationID := strconv.Itoa(id)
	objects, err := doc.AnnotationObjects(0, annotationID)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("objects: %v, %v", objects, err)
	}
	objectID := *objects[0].ID
	x, y := 12.0, 6.0
	if err := doc.AppendAnnotationTextCode(0, annotationID, objectID, "CD", TextCodeLayout{X: &x, Y: &y}); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationTextCodeLayout(0, annotationID, objectID, 1, TextCodeLayout{DeltaX: []float64{2.5}}); err != nil {
		t.Fatal(err)
	}
	annotations, err := doc.Annotations()
	if err != nil {
		t.Fatal(err)
	}
	data, err := doc.Entry(annotations[0].SourcePath)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(data, []byte(`<ofd:TextCode DeltaX="2.5">CD</ofd:TextCode>`)) {
		t.Fatalf("layout not updated: %s", data)
	}
	objects, err = doc.AnnotationObjects(0, annotationID)
	if err != nil || len(objects[0].Codes) != 2 || objects[0].Codes[1].X != nil ||
		objects[0].Codes[1].Y != nil || objects[0].Codes[1].DeltaX == nil ||
		*objects[0].Codes[1].DeltaX != "2.5" || len(objects[0].CGTransforms) != 0 {
		t.Fatalf("positioned codes: %v, %v", objects, err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationTextCodeLayout(0, annotationID, objectID, 0, TextCodeLayout{}); err == nil {
		t.Fatal("accepted first run without coordinates")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed edit changed document: %v", err)
	}
}

func TestSetAnnotationTextTypography(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddMixedAnnotation(0, [4]float64{2, 2, 20, 10}, []AnnotationGraphic{
		{Kind: "text", Text: "AB", Boundary: [4]float64{0, 0, 15, 8}, Size: 3},
	})
	if err != nil {
		t.Fatal(err)
	}
	annotationID := strconv.Itoa(id)
	objects, err := doc.AnnotationObjects(0, annotationID)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("objects: %v, %v", objects, err)
	}
	objectID := *objects[0].ID
	scale, weight, italic := 0.8, uint16(700), true
	if err := doc.SetAnnotationTextTypography(0, annotationID, objectID, TextTypographyStyle{
		Size: 4, HScale: &scale, Weight: &weight, Italic: &italic,
	}); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.AnnotationObjects(0, annotationID)
	if err != nil || objects[0].Size == nil || *objects[0].Size != 4 ||
		objects[0].HScale == nil || *objects[0].HScale != 0.8 ||
		objects[0].Weight == nil || *objects[0].Weight != 700 ||
		objects[0].Italic == nil || !*objects[0].Italic {
		t.Fatalf("size: %v, %v", objects, err)
	}
	annotations, err := doc.Annotations()
	if err != nil {
		t.Fatal(err)
	}
	data, err := doc.Entry(annotations[0].SourcePath)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(data, []byte(`Weight="700"`)) {
		t.Fatalf("weight missing: %s", data)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	invalid := uint16(750)
	if err := doc.SetAnnotationTextTypography(0, annotationID, objectID, TextTypographyStyle{
		Size: 4, Weight: &invalid,
	}); err == nil {
		t.Fatal("accepted invalid weight")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed edit changed document: %v", err)
	}
}

func TestSetAnnotationTextPaintFlags(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddMixedAnnotation(0, [4]float64{2, 2, 20, 10}, []AnnotationGraphic{
		{Kind: "text", Text: "AB", Boundary: [4]float64{0, 0, 15, 8}, Size: 3},
		{Kind: "path", Commands: "M 0 0 L 8 8", Boundary: [4]float64{15, 0, 5, 8}},
	})
	if err != nil {
		t.Fatal(err)
	}
	annotationID := strconv.Itoa(id)
	objects, err := doc.AnnotationObjects(0, annotationID)
	if err != nil || len(objects) != 2 {
		t.Fatalf("objects: %v, %v", objects, err)
	}
	if objects[0].Fill == nil || !*objects[0].Fill || objects[0].Stroke == nil || *objects[0].Stroke {
		t.Fatalf("initial text flags: %+v", objects[0])
	}
	fill, stroke := false, true
	if err := doc.SetAnnotationTextPaintFlags(0, annotationID, *objects[0].ID,
		TextPaintFlags{Fill: &fill, Stroke: &stroke}); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.AnnotationObjects(0, annotationID)
	if err != nil || objects[0].Fill == nil || *objects[0].Fill || objects[0].Stroke == nil || !*objects[0].Stroke {
		t.Fatalf("edited text flags: %+v, %v", objects, err)
	}
	annotations, err := doc.Annotations()
	if err != nil {
		t.Fatal(err)
	}
	data, err := doc.Entry(annotations[0].SourcePath)
	if err != nil || !bytes.Contains(data, []byte(`Fill="false" Stroke="true"`)) {
		t.Fatalf("paint flags: %s, %v", data, err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationTextPaintFlags(0, annotationID, *objects[1].ID, TextPaintFlags{}); err == nil {
		t.Fatal("accepted path object")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed edit changed document: %v", err)
	}
	if err := doc.SetAnnotationTextPaintFlags(0, annotationID, *objects[0].ID, TextPaintFlags{}); err != nil {
		t.Fatal(err)
	}
	data, err = doc.Entry(annotations[0].SourcePath)
	textTag := regexp.MustCompile(`<ofd:TextObject\b[^>]*>`).Find(data)
	if err != nil || textTag == nil || bytes.Contains(textTag, []byte(`Fill="false"`)) ||
		bytes.Contains(textTag, []byte(`Stroke="true"`)) {
		t.Fatalf("paint flags not reset: %s, %v", data, err)
	}
	objects, err = doc.AnnotationObjects(0, annotationID)
	if err != nil || objects[0].Fill == nil || !*objects[0].Fill || objects[0].Stroke == nil || *objects[0].Stroke {
		t.Fatalf("reset text flags: %+v, %v", objects, err)
	}
}

func TestSetAnnotationObjectNameAndBlendMode(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddMixedAnnotation(0, [4]float64{2, 2, 20, 10}, []AnnotationGraphic{
		{Kind: "text", Text: "AB", Boundary: [4]float64{0, 0, 15, 8}, Size: 3},
	})
	if err != nil {
		t.Fatal(err)
	}
	annotationID := strconv.Itoa(id)
	objects, err := doc.AnnotationObjects(0, annotationID)
	if err != nil || len(objects) != 1 || objects[0].ID == nil {
		t.Fatalf("objects: %+v, %v", objects, err)
	}
	objectID := *objects[0].ID
	name, mode := `标题 & "注释"`, "multiply"
	if err := doc.SetAnnotationObjectName(0, annotationID, objectID, &name); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationObjectBlendMode(0, annotationID, objectID, &mode); err != nil {
		t.Fatal(err)
	}
	width := 1.25
	drawID, err := doc.AddDocumentDrawParam(0, DrawParamStyle{LineWidth: &width})
	if err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationObjectDrawParam(0, annotationID, objectID, strconv.Itoa(drawID)); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.AnnotationObjects(0, annotationID)
	if err != nil || objects[0].Name == nil || *objects[0].Name != name ||
		objects[0].BlendMode == nil || *objects[0].BlendMode != "Multiply" ||
		objects[0].DrawParam == nil || *objects[0].DrawParam != strconv.Itoa(drawID) {
		t.Fatalf("appearance: %+v, %v", objects, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "mix-blend-mode:multiply") {
		t.Fatalf("preview: %v", err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	invalid := "unknown"
	if err := doc.SetAnnotationObjectBlendMode(0, annotationID, objectID, &invalid); err == nil {
		t.Fatal("accepted invalid BlendMode")
	}
	if err := doc.SetAnnotationObjectDrawParam(0, annotationID, objectID, "999999"); err == nil {
		t.Fatal("accepted missing DrawParam")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed edit changed document: %v", err)
	}
	if err := doc.SetAnnotationObjectName(0, annotationID, objectID, nil); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationObjectBlendMode(0, annotationID, objectID, nil); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationObjectDrawParam(0, annotationID, objectID, ""); err != nil {
		t.Fatal(err)
	}
	objects, err = doc.AnnotationObjects(0, annotationID)
	if err != nil || objects[0].Name != nil || objects[0].BlendMode != nil || objects[0].DrawParam != nil {
		t.Fatalf("cleared appearance: %+v, %v", objects, err)
	}
}

func TestSetAnnotationGraphicResource(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(40, 30); err != nil {
		t.Fatal(err)
	}
	png, err := doc.PagePNG(0, 25.4)
	if err != nil {
		t.Fatal(err)
	}
	first, err := doc.AddMultimediaResource(0, "Image", "PNG", png)
	if err != nil {
		t.Fatal(err)
	}
	second, err := doc.AddMultimediaResource(0, "Image", "PNG", png)
	if err != nil {
		t.Fatal(err)
	}
	id, err := doc.AddMixedAnnotation(0, [4]float64{2, 2, 20, 12}, []AnnotationGraphic{
		{Kind: "image", ResourceID: strconv.Itoa(first), Boundary: [4]float64{0, 0, 8, 8}},
		{Kind: "path", Commands: "M 0 0 L 8 8", Boundary: [4]float64{8, 0, 8, 8}},
	})
	if err != nil {
		t.Fatal(err)
	}
	annotationID := strconv.Itoa(id)
	objects, err := doc.AnnotationObjects(0, annotationID)
	if err != nil || len(objects) != 2 {
		t.Fatalf("annotation objects: %v, %v", objects, err)
	}
	if err := doc.SetAnnotationGraphicResource(0, annotationID, *objects[0].ID, strconv.Itoa(second)); err != nil {
		t.Fatal(err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	reopened, err := FromBytes(before)
	if err != nil {
		t.Fatal(err)
	}
	edited, err := reopened.AnnotationObjects(0, annotationID)
	if err != nil || edited[0].ResourceID == nil || *edited[0].ResourceID != strconv.Itoa(second) ||
		!reflect.DeepEqual(edited[0].Boundary, objects[0].Boundary) || !reflect.DeepEqual(edited[1], objects[1]) {
		t.Fatalf("edited annotation resource: %v, %v", edited, err)
	}
	reopened.Close()
	for _, test := range []struct {
		name string
		call func() error
	}{
		{"missing resource", func() error { return doc.SetAnnotationGraphicResource(0, annotationID, *objects[0].ID, "999999") }},
		{"wrong object", func() error {
			return doc.SetAnnotationGraphicResource(0, annotationID, *objects[1].ID, strconv.Itoa(second))
		}},
		{"negative page", func() error {
			return doc.SetAnnotationGraphicResource(-1, annotationID, *objects[0].ID, strconv.Itoa(second))
		}},
	} {
		t.Run(test.name, func(t *testing.T) {
			if err := test.call(); err == nil {
				t.Fatal("expected resource edit to fail")
			}
		})
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("failed edit changed document: %v", err)
	}
}

func TestAddsShapeAnnotations(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(30, 20); err != nil {
		t.Fatal(err)
	}
	blue := [3]uint8{0, 0, 255}
	red := [3]uint8{255, 0, 0}
	width := 0.5
	square, err := doc.AddShapeAnnotation(0, "Square", [4]float64{2, 3, 10, 5}, ShapeAnnotationOptions{
		StrokeColor: &blue, FillColor: &red, LineWidth: &width, DashSequence: []float64{1, 2},
	})
	if err != nil || square == 0 {
		t.Fatalf("square: %d, %v", square, err)
	}
	stroke := false
	circle, err := doc.AddShapeAnnotation(0, "Circle", [4]float64{15, 3, 8, 8}, ShapeAnnotationOptions{Stroke: &stroke, FillColor: &red})
	if err != nil || circle == 0 {
		t.Fatalf("circle: %d, %v", circle, err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 2 {
		t.Fatalf("shape annotations: %v, %v", annotations, err)
	}
	if annotations[0].Type != "Path" || annotations[0].Subtype == nil || *annotations[0].Subtype != "Square" ||
		annotations[1].Type != "Path" || annotations[1].Subtype == nil || *annotations[1].Subtype != "Circle" ||
		annotations[0].Parameters["PDFShapeDash"] != "1 2" || annotations[1].Parameters["PDFShapeStroke"] != "" {
		t.Fatalf("shape annotations: %v, %v", annotations, err)
	}
	if _, err := doc.ToPDF(); err != nil {
		t.Fatal(err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddShapeAnnotation(0, "Triangle", [4]float64{2, 3, 10, 5}, ShapeAnnotationOptions{}); err == nil {
		t.Fatal("expected invalid kind")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid shape changed document: %v", err)
	}
}

func TestAddsLineAnnotation(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(30, 20); err != nil {
		t.Fatal(err)
	}
	red := [3]uint8{255, 0, 0}
	width := 0.5
	id, err := doc.AddLineAnnotation(0, [4]float64{2, 3, 10, 5}, [4]float64{0, 5, 10, 0}, LineAnnotationOptions{StrokeColor: &red, LineWidth: &width, DashSequence: []float64{1, 2}})
	if err != nil || id == 0 {
		t.Fatalf("line: %d, %v", id, err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 || annotations[0].Subtype == nil || *annotations[0].Subtype != "Line" || annotations[0].Parameters["PDFLine"] != "0 1 1 0" {
		t.Fatalf("line annotations: %v, %v", annotations, err)
	}
	if _, err := doc.ToPDF(); err != nil {
		t.Fatal(err)
	}
	before, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddLineAnnotation(0, [4]float64{2, 3, 10, 5}, [4]float64{0, 5, 11, 0}, LineAnnotationOptions{}); err == nil {
		t.Fatal("expected invalid endpoints")
	}
	after, err := doc.ToBytes()
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invalid line changed document: %v", err)
	}
}

func TestAddsHighlightAnnotation(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(30, 20); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddHighlightAnnotation(-1, [4]float64{2, 3, 10, 5}, [3]uint8{255, 220, 0}, 96); err == nil {
		t.Fatal("expected negative page index error")
	}
	id, err := doc.AddHighlightAnnotation(0, [4]float64{2, 3, 10, 5}, [3]uint8{255, 220, 0}, 96)
	if err != nil || id == 0 {
		t.Fatalf("add highlight: %d, %v", id, err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 || annotations[0].Type != "Highlight" || annotations[0].LastModDate == nil {
		t.Fatalf("annotations: %v, %v", annotations, err)
	}
	annotationID := strconv.Itoa(id)
	if err := doc.SetAnnotationCreator(0, annotationID, "张三 & Team"); err != nil {
		t.Fatal(err)
	}
	subtype := "审阅 & <校对>"
	if err := doc.SetAnnotationSubtype(0, annotationID, &subtype); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationLastModDate(0, annotationID, "2024-02-29"); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationLastModDate(0, annotationID, "2023-02-29"); err == nil {
		t.Fatal("expected invalid LastModDate error")
	}
	if err := doc.SetAnnotationAppearanceBoundary(0, annotationID, [4]float64{12, 8, 10, 5}); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationAppearanceBoundary(0, annotationID, [4]float64{12, 8, 0, 5}); err == nil {
		t.Fatal("expected invalid appearance boundary error")
	}
	remark := "审核 & 通过"
	if err := doc.SetAnnotationRemark(0, annotationID, &remark); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationParameter(0, annotationID, "分类 & <键>", "甲 & <乙>"); err != nil {
		t.Fatal(err)
	}
	annotations, err = doc.Annotations()
	if err != nil || annotations[0].Attributes["Creator"] != "张三 & Team" || annotations[0].Subtype == nil || *annotations[0].Subtype != subtype || annotations[0].LastModDate == nil || *annotations[0].LastModDate != "2024-02-29" || annotations[0].Remark == nil || *annotations[0].Remark != remark || annotations[0].Parameters["分类 & <键>"] != "甲 & <乙>" || len(annotations[0].AppearanceBox) != 4 || annotations[0].AppearanceBox[0] != 12 {
		t.Fatalf("annotation metadata: %v, %v", annotations, err)
	}
	if err := doc.RemoveAnnotationParameter(0, annotationID, "分类 & <键>"); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationRemark(0, annotationID, nil); err != nil {
		t.Fatal(err)
	}
	if err := doc.SetAnnotationSubtype(0, annotationID, nil); err != nil {
		t.Fatal(err)
	}
	annotations, err = doc.Annotations()
	if err != nil || annotations[0].Subtype != nil {
		t.Fatalf("clear annotation subtype: %v, %v", annotations, err)
	}
	svg, err := doc.PageSVG(0)
	if err != nil || !strings.Contains(svg, "#ffdc00") || !strings.Contains(svg, `fill-opacity="0.3764705882352941"`) {
		t.Fatalf("highlight SVG: %v", err)
	}
	if _, err := doc.AddHighlightAnnotation(0, [4]float64{2, 3, 0, 5}, [3]uint8{255, 220, 0}, 96); err == nil {
		t.Fatal("expected invalid boundary")
	}
	annotations, err = doc.Annotations()
	if err != nil || len(annotations) != 1 {
		t.Fatalf("invalid highlight changed document: %v, %v", annotations, err)
	}
	invalidSubtype := "bad\x00subtype"
	invalidRemark := "bad\x00remark"
	invalidInputs := []struct {
		name string
		call func() error
	}{
		{"creator ID", func() error { return doc.SetAnnotationCreator(0, "bad\x00id", "creator") }},
		{"creator value", func() error { return doc.SetAnnotationCreator(0, annotationID, "bad\x00creator") }},
		{"creator page", func() error { return doc.SetAnnotationCreator(-1, annotationID, "creator") }},
		{"subtype ID", func() error { return doc.SetAnnotationSubtype(0, "bad\x00id", nil) }},
		{"subtype value", func() error { return doc.SetAnnotationSubtype(0, annotationID, &invalidSubtype) }},
		{"subtype page", func() error { return doc.SetAnnotationSubtype(-1, annotationID, nil) }},
		{"date ID", func() error { return doc.SetAnnotationLastModDate(0, "bad\x00id", "2024-02-29") }},
		{"date value", func() error { return doc.SetAnnotationLastModDate(0, annotationID, "bad\x00date") }},
		{"date page", func() error { return doc.SetAnnotationLastModDate(-1, annotationID, "2024-02-29") }},
		{"remark ID", func() error { return doc.SetAnnotationRemark(0, "bad\x00id", nil) }},
		{"remark value", func() error { return doc.SetAnnotationRemark(0, annotationID, &invalidRemark) }},
		{"remark page", func() error { return doc.SetAnnotationRemark(-1, annotationID, nil) }},
		{"parameter ID", func() error { return doc.SetAnnotationParameter(0, "bad\x00id", "key", "value") }},
		{"parameter key", func() error { return doc.SetAnnotationParameter(0, annotationID, "bad\x00key", "value") }},
		{"parameter value", func() error { return doc.SetAnnotationParameter(0, annotationID, "key", "bad\x00value") }},
		{"parameter page", func() error { return doc.SetAnnotationParameter(-1, annotationID, "key", "value") }},
		{"remove parameter ID", func() error { return doc.RemoveAnnotationParameter(0, "bad\x00id", "key") }},
		{"remove parameter key", func() error { return doc.RemoveAnnotationParameter(0, annotationID, "bad\x00key") }},
		{"remove parameter page", func() error { return doc.RemoveAnnotationParameter(-1, annotationID, "key") }},
		{"appearance ID", func() error { return doc.SetAnnotationAppearanceBoundary(0, "bad\x00id", [4]float64{1, 2, 3, 4}) }},
		{"appearance page", func() error { return doc.SetAnnotationAppearanceBoundary(-1, annotationID, [4]float64{1, 2, 3, 4}) }},
		{"visible ID", func() error { return doc.SetAnnotationVisible(0, "bad\x00id", true) }},
		{"visible page", func() error { return doc.SetAnnotationVisible(-1, annotationID, true) }},
		{"print ID", func() error { return doc.SetAnnotationPrint(0, "bad\x00id", true) }},
		{"print page", func() error { return doc.SetAnnotationPrint(-1, annotationID, true) }},
		{"flag ID", func() error { return doc.SetAnnotationFlag(0, "bad\x00id", "NoZoom", true) }},
		{"flag page", func() error { return doc.SetAnnotationFlag(-1, annotationID, "NoZoom", true) }},
		{"flag name", func() error { return doc.SetAnnotationFlag(0, annotationID, "bad\x00flag", true) }},
	}
	beforeInvalid, err := doc.ToBytes()
	if err != nil {
		t.Fatal(err)
	}
	for _, check := range invalidInputs {
		if err := check.call(); err == nil {
			t.Fatalf("%s accepted invalid C string or page index", check.name)
		}
	}
	afterInvalid, err := doc.ToBytes()
	if err != nil || !bytes.Equal(beforeInvalid, afterInvalid) {
		t.Fatalf("invalid annotation input changed document: %v", err)
	}
	if _, err := doc.AddHighlightAnnotation(0, [4]float64{2, 3, math.NaN(), 5}, [3]uint8{255, 220, 0}, 96); err == nil {
		t.Fatal("expected JSON encoding error")
	}
	doc.Close()
	if _, err := doc.AddHighlightAnnotation(0, [4]float64{2, 3, 10, 5}, [3]uint8{255, 220, 0}, 96); err == nil {
		t.Fatal("expected closed document error")
	}
}

func TestAddsURILinkAnnotation(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(30, 20); err != nil {
		t.Fatal(err)
	}
	before, err := doc.PagePNG(0, 25.4)
	if err != nil {
		t.Fatal(err)
	}
	uri := "https://example.com/?a=1&b=2"
	id, err := doc.AddURILinkAnnotation(0, uri, [4]float64{2, 3, 10, 5})
	if err != nil || id <= 0 {
		t.Fatalf("add URI link: %d, %v", id, err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 1 || annotations[0].Type != "Link" || len(annotations[0].Actions) != 1 {
		t.Fatalf("link annotations: %+v, %v", annotations, err)
	}
	if action := annotations[0].Actions[0]; action.Kind != "URI" || action.Attributes["URI"] != uri {
		t.Fatalf("link action: %+v", action)
	}
	after, err := doc.PagePNG(0, 25.4)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatalf("invisible link changed image: %v", err)
	}
	pdf, err := doc.ToPDF()
	if err != nil || !bytes.Contains(pdf, []byte("/Subtype /Link")) {
		t.Fatalf("PDF link missing: %v", err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("link references: %+v, %v", issues, err)
	}
	if _, err := doc.AddURILinkAnnotation(0, "", [4]float64{2, 3, 10, 5}); err == nil {
		t.Fatal("expected empty URI rejection")
	}
}

func TestAddsGotoAndBookmarkLinkAnnotations(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(30, 20); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBlankPage(40, 25); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	target := pages[1].ID
	if err := doc.AddBookmark(0, "chapter & 2", Destination{PageID: target, Kind: "Fit"}); err != nil {
		t.Fatal(err)
	}
	left, top := 4.0, 6.0
	first, err := doc.AddGotoLinkAnnotation(0, Destination{PageID: target, Kind: "XYZ", Left: &left, Top: &top}, [4]float64{2, 3, 10, 5})
	if err != nil {
		t.Fatal(err)
	}
	second, err := doc.AddBookmarkLinkAnnotation(0, "chapter & 2", [4]float64{13, 3, 10, 5})
	if err != nil || first == second {
		t.Fatalf("bookmark link: %d, %v", second, err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 2 {
		t.Fatalf("annotations: %+v, %v", annotations, err)
	}
	if annotations[0].Actions[0].Destination.PageID != target || *annotations[1].Actions[0].Bookmark != "chapter & 2" {
		t.Fatalf("link targets: %+v", annotations)
	}
	pdf, err := doc.ToPDF()
	if err != nil || bytes.Count(pdf, []byte("/Subtype /Link")) != 2 {
		t.Fatalf("PDF links: %v", err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %+v, %v", issues, err)
	}
	if _, err := doc.AddBookmarkLinkAnnotation(0, "missing", [4]float64{1, 1, 2, 2}); err == nil {
		t.Fatal("expected missing bookmark rejection")
	}
	if _, err := doc.AddGotoLinkAnnotation(0, Destination{PageID: "missing", Kind: "Fit"}, [4]float64{1, 1, 2, 2}); err == nil {
		t.Fatal("expected missing page rejection")
	}
}

func TestNumericPageIDAliasesForNavigationAndAnnotationRecords(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	for range 2 {
		if err := doc.AddBlankPage(40, 30); err != nil {
			t.Fatal(err)
		}
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	alias := "0" + pages[1].ID
	destination := Destination{PageID: alias, Kind: "Fit"}
	if err := doc.AddBookmark(0, "alias", destination); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddOutline(
		0,
		"alias",
		nil,
		&destination,
		true,
	); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddGotoLinkAnnotation(0, destination, [4]float64{2, 2, 8, 8}); err != nil {
		t.Fatal(err)
	}
	if pdf, err := doc.ToPDF(); err != nil || !bytes.HasPrefix(pdf, []byte("%PDF-")) {
		t.Fatalf("PDF navigation alias: %v", err)
	}
	if _, err := doc.AddTextNoteAnnotation(
		0,
		"first",
		[4]float64{12, 2, 8, 8},
		TextNoteOptions{},
	); err != nil {
		t.Fatal(err)
	}
	const listPath = "Doc_0/Annots/Annotations.xml"
	source, err := doc.Entry(listPath)
	if err != nil {
		t.Fatal(err)
	}
	aliased := strings.Replace(
		string(source),
		`PageID="`+pages[0].ID+`"`,
		`PageID="0`+pages[0].ID+`"`,
		1,
	)
	if aliased == string(source) {
		t.Fatal("annotation page record was not found")
	}
	if err := doc.SetEntry(listPath, []byte(aliased)); err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddTextNoteAnnotation(
		0,
		"second",
		[4]float64{22, 2, 8, 8},
		TextNoteOptions{},
	); err != nil {
		t.Fatal(err)
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 3 {
		t.Fatalf("annotations: %d, %v", len(annotations), err)
	}
	current, err := doc.Entry(listPath)
	if err != nil || bytes.Count(current, []byte("<ofd:Page PageID=")) != 1 {
		t.Fatalf("annotation page records: %v", err)
	}
	issues, err := doc.ValidateReferences()
	if err != nil || len(issues) != 0 {
		t.Fatalf("references: %+v, %v", issues, err)
	}
	duplicate := `<ofd:Page PageID="` + pages[0].ID + `">` +
		`<ofd:FileLoc>Page_0/Annotation.xml</ofd:FileLoc></ofd:Page>`
	modified := strings.Replace(string(current), "</ofd:Annotations>", duplicate+"</ofd:Annotations>", 1)
	if err := doc.SetEntry(listPath, []byte(modified)); err != nil {
		t.Fatal(err)
	}
	issues, err = doc.ValidateReferences()
	if err != nil {
		t.Fatal(err)
	}
	hasDuplicatePage := slices.ContainsFunc(issues, func(issue ValidationIssue) bool {
		return issue.Code == "duplicate_annotation_page_id"
	})
	if !hasDuplicatePage {
		t.Fatalf("missing duplicate annotation page diagnostic: %+v", issues)
	}
}

func TestWritesQuadrilateralLinkRegions(t *testing.T) {
	doc, err := New()
	if err != nil {
		t.Fatal(err)
	}
	defer doc.Close()
	if err := doc.AddBlankPage(30, 20); err != nil {
		t.Fatal(err)
	}
	if err := doc.AddBlankPage(30, 20); err != nil {
		t.Fatal(err)
	}
	quad := [4][2]float64{{3, 4}, {6, 4}, {7, 6}, {4, 6}}
	invalid := [4][2]float64{{3, 4}, {7, 6}, {6, 4}, {4, 6}}
	if _, err := doc.AddURILinkAnnotationWithQuads(0, "https://example.org", [4]float64{2, 3, 10, 5}, [][4][2]float64{invalid}); err == nil {
		t.Fatal("expected invalid quad")
	}
	annotations, err := doc.Annotations()
	if err != nil || len(annotations) != 0 {
		t.Fatalf("annotations after rejection: %+v, %v", annotations, err)
	}
	if _, err := doc.AddURILinkAnnotationWithQuads(0, "https://example.org", [4]float64{2, 3, 10, 5}, [][4][2]float64{quad}); err != nil {
		t.Fatal(err)
	}
	pages, err := doc.Pages()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := doc.AddGotoLinkAnnotationWithQuads(0, Destination{PageID: pages[1].ID, Kind: "Fit"}, [4]float64{2, 3, 10, 5}, [][4][2]float64{quad}); err != nil {
		t.Fatal(err)
	}
	annotations, err = doc.Annotations()
	if err != nil || len(annotations) != 2 || annotations[0].Actions[0].Region == nil || annotations[1].Actions[0].Region == nil {
		t.Fatalf("quad actions: %+v, %v", annotations, err)
	}
	pdf, err := doc.ToPDF()
	if err != nil || bytes.Count(pdf, []byte("/QuadPoints")) != 2 {
		t.Fatalf("PDF quad links: %v", err)
	}
}
