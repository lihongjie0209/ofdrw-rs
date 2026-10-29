import os
import subprocess
import sys
from pathlib import Path
from xml.etree import ElementTree

import pytest

from ofdrw_rs import Document, OfdError, create_ses_seal


FIXTURES = Path(__file__).resolve().parents[4] / "ofdrw-reader" / "src" / "test" / "resources"


def test_explicit_native_library_path_does_not_fall_back(tmp_path):
    missing = tmp_path / "missing-native-library"
    environment = dict(os.environ, OFDRW_LIB_PATH=str(missing))
    result = subprocess.run(
        [sys.executable, "-c", "import ofdrw_rs"],
        cwd=Path(__file__).resolve().parents[1],
        env=environment,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode != 0
    assert f"cannot load OFDRW_LIB_PATH: {missing}" in result.stderr


def test_package_header_roundtrip():
    with Document.new() as document:
        assert document.package_info() == {"version": "1.2", "doc_type": "OFD"}
        document.set_package_header("1.3 & beta", "OFD-A")
        with pytest.raises(OfdError):
            document.set_package_header("", "OFD")
        data = document.to_bytes()
    with Document.from_bytes(data) as reopened:
        assert reopened.package_info() == {"version": "1.3 & beta", "doc_type": "OFD-A"}
        reopened.set_entry("OFDEntries.xml", b"<DocEntries/>")
        protected = reopened.to_bytes()
        with pytest.raises(OfdError):
            reopened.set_package_header("1.2", "OFD")
        with pytest.raises(OfdError):
            reopened.set_document_info_field(0, "Title", "edit")
        with pytest.raises(OfdError):
            reopened.append_blank_page(0, 20, 20)
        assert reopened.to_bytes() == protected


def test_utf16_page_xml_roundtrip_preserves_original_bytes():
    with Document.new() as document:
        document.add_text_page("跨语言 UTF-16", 40, 30)
        path = document.pages()[0]["path"]
        source = document.entry(path).decode("utf-8")
        raw = b"\xff\xfe" + source.encode("utf-16-le")
        document.set_entry(path, raw)
        data = document.to_bytes()
    with Document.from_bytes(data) as reopened:
        assert reopened.page_text(0) == ["跨语言 UTF-16"]
        assert reopened.entry(path) == raw


def test_page_content_ignores_extension_namespace_across_python_binding():
    path = "Doc_0/Pages/Page_0/Content.xml"
    with Document.new() as document:
        document.add_text_page("Python real", 80, 50)
        page = document.entry(path).decode()
        page = page.replace(
            "<ofd:Page xmlns:ofd=",
            '<ofd:Page xmlns:ext="urn:example:extension" xmlns:ofd=',
            1,
        ).replace(
            "<ofd:Content>",
            '<ext:Content><ext:Layer ID="fake"><ext:TextCode>fake</ext:TextCode></ext:Layer></ext:Content>'
            '<ofd:Content><ext:Layer ID="fake"><ext:TextCode>fake</ext:TextCode></ext:Layer>',
            1,
        ).replace(
            "<ofd:TextObject ",
            '<ext:TextObject ID="fake" Boundary="0 0 1 1"><ext:TextCode>fake</ext:TextCode></ext:TextObject>'
            '<ofd:TextObject ',
            1,
        ).replace(
            "<ofd:TextCode ",
            "<ext:TextCode>fake</ext:TextCode><ofd:TextCode ",
            1,
        )
        document.set_entry(path, page.encode())
        data = document.to_bytes()

    with Document.from_bytes(data) as reopened:
        assert reopened.page_text(0) == ["Python real"]
        assert len(reopened.page_layers(0)) == 1
        assert reopened.page_layers(0)[0]["object_count"] == 1
        assert len(reopened.page_objects(0)) == 1
        assert len(reopened.page_text_objects(0)[0]["codes"]) == 1
        assert reopened.keyword_positions(["fake"]) == []
        svg = reopened.page_svg(0)
        assert "Python real" in svg
        assert "fake" not in svg


def test_all_text_preserves_page_order_and_empty_pages():
    with Document.new() as document:
        document.add_text_page("首页", 100, 100)
        document.add_blank_page(100, 100)
        document.add_text_page("last", 100, 100)
        assert document.all_text() == [["首页"], [], ["last"]]
        data = document.to_bytes()
    with Document.from_bytes(data) as reopened:
        assert reopened.all_text() == [["首页"], [], ["last"]]


def test_text_code_hex_escapes_across_binding():
    path = "Doc_0/Pages/Page_0/Content.xml"
    with Document.new() as document:
        document.add_text_page("A B", 100, 100)
        source = document.entry(path)
        assert b">A B</ofd:TextCode>" in source
        escaped = source.replace(b">A B</ofd:TextCode>",
                                 b">A\\0020B</ofd:TextCode>")
        document.set_entry(path, escaped)
        assert document.page_text(0) == ["A B"]
        assert document.page_text_objects(0)[0]["codes"][0]["text"] == "A B"
        assert len(document.keyword_positions(["A B"])) == 1
        assert ">A B</text>" in document.page_svg(0)

    with Document.new() as document:
        document.add_text_page(r"\0041", 100, 100)
        assert document.page_text(0) == [r"\0041"]
        assert len(document.keyword_positions([r"\0041"])) == 1

    with Document.new() as document:
        document.add_text_page("A\x01B", 100, 100)
        assert b"A\\0001B" in document.entry(path)
        assert document.page_text(0) == ["A\x01B"]
        assert "A\uFFFDB" in document.page_svg(0)

    with Document.new() as document:
        document.add_text_page("A\rB", 100, 100)
        assert b"A\\000DB" in document.entry(path)
        data = document.to_bytes()
    with Document.from_bytes(data) as reopened:
        assert reopened.page_text(0) == ["A\rB"]


def test_text_writers_preserve_embedded_nul_across_binding():
    with Document.new() as document:
        document.add_text_page("A\0B", 100, 100)
        obj_id = document.page_text_objects(0)[0]["id"]
        document.set_page_text_code(0, obj_id, 0, "C\0D")
        document.append_page_text_code(0, obj_id, "E\0F", {})
        assert document.page_text(0) == ["C\0D", "E\0F"]
        document.append_text_page(0, "G\0H", 100, 100)
        assert document.insert_text_page(0, 1, "I\0J", 100, 100) == 1
        assert document.page_text(1) == ["I\0J"]
        assert document.page_text(2) == ["G\0H"]

    font = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
            "tests" / "fixtures" / "dejavu-sans.ttf").read_bytes()
    with Document.new() as document:
        font_id = document.add_embedded_font("DejaVu Sans", font)
        document.add_text_page_with_font("K\0L", font_id, 100, 100)
        document.append_text_page_with_font(0, "M\0N", font_id, 100, 100)
        assert document.page_text(0) == ["K\0L"]
        assert document.page_text(1) == ["M\0N"]


def test_imports_embedded_pdf_file_across_binding(tmp_path):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "embedded-file.pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert [item["name"] for item in document.attachments()] == ["报告.txt"]
        assert document.attachment_data(0) == b"fixture payload"
        assert document.validate_references() == []
        target = tmp_path / "embedded-file.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.attachment_data(0) == b"fixture payload"


def test_imports_and_exports_linked_pdf_popup_across_binding():
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "popup.pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        annotations = document.annotations()
        assert len(annotations) == 1
        note = annotations[0]
        assert note["subtype"] == "Text"
        assert note["remark"] == "review"
        assert note["parameters"]["PDFPopupOpen"] == "true"
        assert len(note["parameters"]["PDFPopupBoundary"].split()) == 4
        assert document.validate_references() == []
        exported = document.to_pdf()
    with Document.from_pdf_bytes(exported) as reopened:
        again = reopened.annotations()[0]
        assert again["parameters"]["PDFPopupOpen"] == "true"
        before = [float(value) for value in note["parameters"]["PDFPopupBoundary"].split()]
        after = [float(value) for value in again["parameters"]["PDFPopupBoundary"].split()]
        assert all(abs(left - right) < 0.01 for left, right in zip(before, after))


def test_imports_and_exports_standalone_pdf_popup_across_binding():
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "standalone-popup.pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        annotations = document.annotations()
        assert len(annotations) == 1
        popup = annotations[0]
        assert popup["remark"] == "standalone review"
        assert popup["attributes"]["Creator"] == "Reviewer"
        assert popup["parameters"]["PDFPopupStandalone"] == "true"
        assert popup["parameters"]["PDFTextOpen"] == "true"
        assert document.validate_references() == []
        exported = document.to_pdf()
    with Document.from_pdf_bytes(exported) as reopened:
        again = reopened.annotations()[0]
        assert again["remark"] == "standalone review"
        assert again["parameters"]["PDFPopupStandalone"] == "true"


def test_imports_and_exports_pdf_ink_across_binding():
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "ink.pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        ink = document.annotations()[0]
        assert ink["subtype"] == "Ink"
        assert ink["remark"] == "two pen strokes"
        assert ink["attributes"]["Creator"] == "Reviewer"
        assert len(document.annotation_objects(0, ink["id"])) == 2
        assert document.validate_references() == []
        exported = document.to_pdf()
    with Document.from_pdf_bytes(exported) as reopened:
        again = reopened.annotations()[0]
        assert again["subtype"] == "Ink"
        assert len(reopened.annotation_objects(0, again["id"])) == 2


def test_imports_and_exports_pdf_polygon_and_polyline_across_binding():
    root = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
            "tests" / "fixtures")
    for name, subtype, paths in [("polygon.pdf", "Polygon", 1),
                                 ("polyline.pdf", "PolyLine", 3)]:
        with Document.from_pdf_bytes((root / name).read_bytes()) as document:
            shape = document.annotations()[0]
            assert shape["subtype"] == subtype
            assert shape["attributes"]["Creator"] == "Reviewer"
            assert len(document.annotation_objects(0, shape["id"])) == paths
            assert document.validate_references() == []
            exported = document.to_pdf()
        with Document.from_pdf_bytes(exported) as reopened:
            again = reopened.annotations()[0]
            assert again["subtype"] == subtype
            assert len(reopened.annotation_objects(0, again["id"])) == paths


def test_imports_indirect_form_group_across_binding():
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "form-group-indirect.pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert len(document.page_paths(0)) == 1
        assert document.validate_references() == []
        assert document.page_png(0, 72).startswith(b"\x89PNG\r\n\x1a\n")


def test_imports_sixteen_bit_pdf_image_across_binding():
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "gray-16-bit.pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert len(document.page_images(0)) == 1
        assert document.page_image_data(0, 0).startswith(b"\x89PNG\r\n\x1a\n")
        assert document.validate_references() == []
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.page_image_data(0, 0) == document.page_image_data(0, 0)


@pytest.mark.parametrize("name", ["jbig2-mmr.pdf", "jbig2-globals.pdf"])
def test_imports_jbig2_pdf_image_across_binding(name):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / name)
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert len(document.page_images(0)) == 1
        assert document.page_image_data(0, 0).startswith(b"\x89PNG\r\n\x1a\n")
        assert document.validate_references() == []
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.page_image_data(0, 0) == document.page_image_data(0, 0)


@pytest.mark.parametrize("filename", [
    "shading-sampled-axial.pdf", "shading-sampled-radial.pdf",
    "shading-sampled-cubic-axial.pdf", "shading-sampled-cubic-radial.pdf",
    "shading-nested-stitching-axial.pdf", "shading-nested-stitching-radial.pdf",
    "shading-indirect-functions-axial.pdf", "shading-indirect-functions-radial.pdf",
    "shading-calculator-axial.pdf", "shading-calculator-radial.pdf",
    "shading-function-array-axial.pdf", "shading-function-array-radial.pdf",
])
def test_imports_sampled_shading_pdf_across_binding(filename):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / filename)
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        svg = document.page_svg(0)
        assert all(color in svg for color in ["#ff0000", "#00ff00", "#0000ff"])
        assert document.page_png(0, 72).startswith(b"\x89PNG\r\n\x1a\n")
        assert document.validate_references() == []


@pytest.mark.parametrize("name,kind", [
    ("mesh-free-rgb", "Gouraud"),
    ("mesh-lattice-rgb", "LaGouraud"),
    ("mesh-packed-gray", "Gouraud"),
    ("mesh-wide-rgb", "Gouraud"),
    ("mesh-lattice-clipped", "LaGouraud"),
    ("mesh-cmyk", "Gouraud"),
    ("mesh-calgray", "Gouraud"),
    ("mesh-function-array", "Gouraud"),
    ("mesh-sampled-function", "Gouraud"),
    ("mesh-stitching-function", "Gouraud"),
    ("mesh-calculator-function", "Gouraud"),
    ("patch-coons-flat", "Gouraud"),
    ("patch-coons-curved", "Gouraud"),
    ("patch-tensor-curved", "Gouraud"),
    ("patch-tensor-folded", "Gouraud"),
    ("patch-coons-reuse-1", "Gouraud"),
    ("patch-coons-reuse-2", "Gouraud"),
    ("patch-coons-reuse-3", "Gouraud"),
    ("patch-tensor-reuse", "Gouraud"),
    ("patch-coons-function", "Gouraud"),
    ("patch-tensor-clipped", "Gouraud"),
    ("patch-coons-wide", "Gouraud"),
    ("patch-coons-packed", "Gouraud"),
    ("patch-coons-calgray", "Gouraud"),
    ("patch-tensor-cmyk", "Gouraud"),
    ("mesh-function", "Gouraud"),
])
def test_imports_pdf_gouraud_meshes_across_binding(name, kind):
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / (name + ".pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        paints = document.page_object_paints(0)
        assert len(paints) == 1
        assert paints[0]["fill"]["kind"] == kind
        png = document.page_png(0, 72)
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.page_png(0, 72) == png
            assert reopened.page_object_paints(0) == paints
            assert reopened.validate_references() == []


@pytest.mark.parametrize("name", [
    "plane-calculator-rgb",
    "plane-calculator-step",
    "plane-component-array",
    "plane-sampled-rgb",
    "plane-sampled-packed",
    "plane-sampled-cubic",
    "plane-sampled-cubic-small",
    "plane-sampled-reversed",
    "plane-cmyk",
    "plane-calgray",
    "plane-transformed",
    "plane-bbox-background",
])
def test_imports_pdf_function_based_shadings_across_binding(name):
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / (name + ".pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        images = document.page_images(0)
        assert len(images) == 1
        png = document.page_png(0, 72)
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.page_png(0, 72) == png
            assert reopened.page_images(0) == images
            assert reopened.validate_references() == []


def test_imports_group4_fax_pdf_image_across_binding():
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "ccitt-g4.pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert len(document.page_images(0)) == 1
        assert document.page_image_data(0, 0).startswith(b"\x89PNG\r\n\x1a\n")
        assert document.page_png(0, 72).startswith(b"\x89PNG\r\n\x1a\n")
        assert document.validate_references() == []
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.page_image_data(0, 0) == document.page_image_data(0, 0)


def test_imports_pdf_xobject_associated_files_across_binding(tmp_path):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "xobject-associated-files.pdf")
    target = tmp_path / "xobject-associated-files.ofd"
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert len(document.page_images(0)) == 1
        assert [item["name"] for item in document.attachments()] == [
            "form-source.csv", "image-source.rgb",
        ]
        assert document.attachment_data(0) == b"a,b\n"
        assert document.attachment_data(1) == bytes([255, 0, 0])
        assert document.validate_references() == []
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.attachment_data(0) == b"a,b\n"
        assert reopened.attachment_data(1) == bytes([255, 0, 0])


def test_imports_pdf_page_resource_associated_files_across_binding(tmp_path):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "page-resource-associated-files.pdf")
    expected = [
        ("page-tree.txt", b"page tree"),
        ("resources.txt", b"resources"),
        ("graphics.txt", b"graphics"),
        ("profile.txt", b"profile"),
        ("pattern-list.txt", b"pattern list"),
        ("pattern.txt", b"pattern"),
        ("shading.txt", b"shading"),
        ("properties.txt", b"properties"),
    ]
    target = tmp_path / "page-resource-associated-files.ofd"
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert [item["name"] for item in document.attachments()] == [
            name for name, _ in expected
        ]
        assert [document.attachment_data(i) for i in range(len(expected))] == [
            payload for _, payload in expected
        ]
        assert document.validate_references() == []
        document.save(target)
    with Document.open(target) as reopened:
        assert [reopened.attachment_data(i) for i in range(len(expected))] == [
            payload for _, payload in expected
        ]


def test_imports_default_gray_jpeg2000_across_binding(tmp_path):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "default-gray-jpx.pdf")
    target = tmp_path / "default-gray-jpx.ofd"
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert "#bcbcbc" in document.page_svg(0)
        assert document.page_images(0)[0]["format"] == "PNG"
        png = document.page_image_data(0, 0)
        assert png.startswith(b"\x89PNG\r\n\x1a\n")
        assert document.validate_references() == []
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_image_data(0, 0) == png


def test_exports_and_reimports_pdf_attachments_across_binding():
    with Document.new() as source:
        source.add_blank_page(40, 30)
        source.add_attachment("报告.txt", b"unicode payload")
        source.add_attachment("a.txt", b"ascii payload")
        source.set_attachment_metadata(
            0, format="text/plain", creation_date="2024-02-29T12:30:59",
            mod_date="2024-03-01T08:00:00", usage="Data",
        )
        pdf = source.to_pdf()
        assert pdf.startswith(b"%PDF-2.0")
    with Document.from_pdf_bytes(pdf) as imported:
        assert [item["name"] for item in imported.attachments()] == ["a.txt", "报告.txt"]
        assert imported.attachment_data(0) == b"ascii payload"
        assert imported.attachment_data(1) == b"unicode payload"
        assert imported.attachments()[1]["format"] == "text/plain"
        assert imported.attachments()[1]["creation_date"] == "2024-02-29T12:30:59"
        assert imported.attachments()[1]["mod_date"] == "2024-03-01T08:00:00"
        assert imported.attachments()[1]["usage"] == "Data"
        assert imported.validate_references() == []


def test_imports_hairline_pdf_text_across_binding(tmp_path):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "hairline-text.pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert document.page_text(0) == ["Hairline"]
        assert 'stroke-width="0.1"' in document.page_svg(0)
        target = tmp_path / "hairline.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_text(0) == ["Hairline"]
        assert 'stroke-width="0.1"' in reopened.page_svg(0)


def test_imports_direct_ocmd_pdf_across_binding():
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "direct-ocmd.pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert document.page_text(0) == ["Shown"]
        assert document.validate_references() == []


@pytest.mark.parametrize("name", ["inline-image.pdf", "filtered-inline-image.pdf"])
def test_imports_inline_pdf_image_across_binding(tmp_path, name):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / name)
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        images = document.page_images(0)
        assert len(images) == 1
        assert images[0]["format"] == "PNG"
        png = document.page_image_data(0, 0)
        assert png.startswith(b"\x89PNG\r\n\x1a\n")
        assert int.from_bytes(png[16:20], "big") == 2
        assert int.from_bytes(png[20:24], "big") == 1
        assert "<image" in document.page_svg(0)
        target = tmp_path / "inline.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_image_data(0, 0) == png


def test_imports_axial_shading_pdf_across_binding(tmp_path):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "shading-axial.pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        svg = document.page_svg(0)
        assert "<linearGradient" in svg
        assert "<clipPath" in svg
        assert document.page_png(0, 72).startswith(b"\x89PNG")
        target = tmp_path / "shading.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_svg(0) == svg


@pytest.mark.parametrize("name", ["shading-radial.pdf", "shading-radial-reverse.pdf"])
def test_imports_radial_shading_pdf_across_binding(tmp_path, name):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / name)
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        svg = document.page_svg(0)
        assert "<radialGradient" in svg
        assert document.page_png(0, 72).startswith(b"\x89PNG")
        target = tmp_path / "shading.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_svg(0) == svg


def test_imports_stitched_shading_pdf_across_binding(tmp_path):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "shading-stitching.pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        svg = document.page_svg(0)
        assert svg.count('offset="0.5"') >= 2
        assert document.page_png(0, 72).startswith(b"\x89PNG")
        target = tmp_path / "stitched.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_svg(0) == svg


@pytest.mark.parametrize("mode,extend", [
    ("none", 0), ("start", 1), ("end", 2), ("both", 3),
])
def test_imports_pdf_shading_extend_modes(tmp_path, mode, extend):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / f"shading-extend-{mode}.pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        svg = document.page_svg(0)
        assert "<linearGradient" in svg
        assert document.page_png(0, 72).startswith(b"\x89PNG")
        target = tmp_path / f"extend-{mode}.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_svg(0) == svg
        page_xml = reopened.entry(reopened.pages()[0]["path"]).decode()
        assert f'Extend="{extend}"' in page_xml


@pytest.mark.parametrize("name", ["shading-domain.pdf", "shading-stitching-domain.pdf"])
def test_imports_nondefault_pdf_shading_domain(tmp_path, name):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / name)
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        svg = document.page_svg(0)
        assert "<linearGradient" in svg
        assert document.page_png(0, 72).startswith(b"\x89PNG")
        target = tmp_path / "domain.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_svg(0) == svg


def test_imports_pdf_shading_bbox_with_existing_clip(tmp_path):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "shading-bbox.pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        svg = document.page_svg(0)
        assert svg.count("<clipPath ") >= 2
        assert document.page_png(0, 72).startswith(b"\x89PNG")
        target = tmp_path / "bbox.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_svg(0) == svg


@pytest.mark.parametrize("name", [
    "shading-range.pdf", "shading-stitching-range.pdf", "shading-cmyk-range.pdf",
])
def test_imports_pdf_shading_function_range(tmp_path, name):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / name)
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        svg = document.page_svg(0)
        assert svg.count("<stop offset=") > 2
        assert document.page_png(0, 72).startswith(b"\x89PNG")
        target = tmp_path / "range.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_svg(0) == svg


def test_decrypts_java_password_encrypted_document(tmp_path):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "java-password-encrypted.ofd")
    with Document.open(fixture) as encrypted:
        assert encrypted.is_encrypted
        assert encrypted.encryption_info()[0]["usernames"] == ["777"]
        with pytest.raises(OfdError):
            encrypted.decrypt_with_password("wrong")
        with pytest.raises(OfdError):
            encrypted.decrypt_with_password("12345678", "other")
        with encrypted.decrypt_with_password("12345678", "777") as plain:
            assert not plain.is_encrypted
            assert plain.page_count == 2
            assert plain.page_text(0)
            with plain.encrypt_with_passwords([
                {"username": "alice", "password": "first"},
                {"username": "bob", "password": "second", "user_type": "Owner"},
            ]) as wrapped:
                assert wrapped.is_encrypted
                assert wrapped.encryption_info()[0]["usernames"] == ["alice", "bob"]
                target = tmp_path / "wrapped.ofd"
                wrapped.save(target)
            with Document.open(target) as reopened:
                with reopened.decrypt_with_password("second", "bob") as restored:
                    assert restored.page_text(0) == plain.page_text(0)
            with pytest.raises(OfdError):
                plain.encrypt_with_passwords([])
        assert encrypted.is_encrypted


def test_certificate_encryption_interoperates_with_java(tmp_path):
    fixtures = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
                "tests" / "fixtures")
    key = (fixtures / "java-sm2-private-key.bin").read_bytes()
    cert = (fixtures / "java-sm2-cert.der").read_bytes()
    with Document.open(fixtures / "java-cert-encrypted.ofd") as encrypted:
        with pytest.raises(OfdError):
            encrypted.decrypt_with_sm2_private_key(bytes(32), "777")
        with encrypted.decrypt_with_sm2_private_key(key, "777") as plain:
            assert not plain.is_encrypted
            text = plain.page_text(0)
            assert text
            with plain.encrypt_with_certificates([
                {"username": "rust", "certificate_der": cert},
            ]) as wrapped:
                assert wrapped.encryption_info()[0]["protection_case_id"] == "1.1.2"
                target = tmp_path / "cert-encrypted.ofd"
                wrapped.save(target)
            with Document.open(target) as reopened:
                with reopened.decrypt_with_sm2_private_key(key, "rust") as restored:
                    assert restored.page_text(0) == text
            with pytest.raises(OfdError):
                plain.encrypt_with_certificates([])


def test_mixed_password_and_certificate_recipients(tmp_path):
    fixtures = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
                "tests" / "fixtures")
    key = (fixtures / "java-sm2-private-key.bin").read_bytes()
    certificate = (fixtures / "java-sm2-cert.der").read_bytes()
    with Document.open(fixtures / "java-cert-encrypted.ofd") as java_encrypted:
        with java_encrypted.decrypt_with_sm2_private_key(key) as plain:
            expected = plain.page_text(0)
            with plain.encrypt_with_recipients([
                {"type": "password", "username": "password", "password": "secret"},
                {"type": "certificate", "username": "certificate",
                 "certificate_der": certificate},
            ]) as mixed:
                assert mixed.encryption_info()[0]["protection_case_id"] is None
                target = tmp_path / "mixed.ofd"
                mixed.save(target)
            with Document.open(target) as reopened:
                with reopened.decrypt_with_password("secret", "password") as restored:
                    assert restored.page_text(0) == expected
                with reopened.decrypt_with_sm2_private_key(key, "certificate") as restored:
                    assert restored.page_text(0) == expected
                with pytest.raises(OfdError):
                    reopened.decrypt_with_password("secret", "certificate")
            with pytest.raises(OfdError):
                plain.encrypt_with_recipients([])
    with Document.open(fixtures / "java-mixed-encrypted.ofd") as java_mixed:
        with java_mixed.decrypt_with_password("java-mixed-pass", "java-password-user") as restored:
            assert restored.page_text(0) == expected
        with java_mixed.decrypt_with_sm2_private_key(key, "java-cert-user") as restored:
            assert restored.page_text(0) == expected


@pytest.mark.parametrize("name", [
    "type1-tounicode.pdf",
    "type1-subset-tounicode.pdf",
    "rotated-tounicode.pdf",
    "metadata-tounicode.pdf",
    "outline-tounicode.pdf",
    "type0-identity-tounicode.pdf",
    "type0-indirect-widths.pdf",
    "type0-mixed-cmap.pdf",
    "type0-gbk-cmap.pdf",
    "type0-gbk-vertical-cmap.pdf",
    "type0-gbk-vertical-collection.pdf",
    "type0-cns-vertical-cmap.pdf",
    "type0-cns-vertical-collection.pdf",
    "type0-cns-cmap.pdf",
    "type0-gbk-collection-unicode.pdf",
    "type0-cns-collection-unicode.pdf",
    "type0-identity-collection-unicode.pdf",
    "type0-embedded-collection-unicode.pdf",
    "encoding-differences.pdf",
    "custom-type1-differences.pdf",
    "simple-indirect-widths.pdf",
    "text-default-origin.pdf",
])
def test_imports_tounicode_pdf(tmp_path, name):
    source = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
              "tests" / "fixtures" / name)
    with Document.from_pdf_bytes(source.read_bytes()) as document:
        assert document.page_text(0) == ["中文"]
        target = tmp_path / "unicode.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_text(0) == ["中文"]


@pytest.mark.parametrize("name,text,advance", [
    ("standard14-tounicode.pdf", "中文", 6.804),
    ("standard14-symbol.pdf", "ΑΒ", 8.664),
    ("standard14-dingbats.pdf", "✁✂", 11.688),
])
def test_imports_standard14_fonts_without_widths(tmp_path, name, text, advance):
    source = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / name
    with Document.from_pdf_bytes(source.read_bytes()) as document:
        assert document.page_text(0) == [text]
        delta = float(document.page_text_objects(0)[0]["codes"][0]["delta_x"])
        assert abs(delta - advance * 25.4 / 72.0) < 1e-6
        target = tmp_path / "standard.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_text(0) == [text]
        assert reopened.validate_references() == []


@pytest.mark.parametrize("name,weight", [
    ("font-style-standard.pdf", 700),
    ("font-style-descriptor.pdf", 600),
])
def test_imports_pdf_font_styles_and_preserves_them_after_save(tmp_path, name, weight):
    source = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / name
    with Document.from_pdf_bytes(source.read_bytes()) as document:
        assert document.page_text(0) == ["中文"]
        objects = document.page_text_objects(0)
        assert objects[0]["weight"] == weight
        assert objects[0]["italic"] is True
        expected_png = document.page_png(0, 72)
        target = tmp_path / "styled.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_text_objects(0) == objects
        assert reopened.page_png(0, 72) == expected_png
        assert reopened.validate_references() == []


@pytest.mark.parametrize("name,text,glyphs", [
    ("type0-ligature-glyph.pdf", ["fiA", "fi"], [3315, 36]),
    ("type1-opentype-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type1-affine-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type1-reflected-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type1-rotated-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type1-opentype-affine-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type0-fd-matrices-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type0-fd-translated-top-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type0-fd-ranges-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type0-fd-only-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type0-vertical-fd-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type1-stroked-affine-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type0-clipped-fd-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type0-clipped-ligature.pdf", ["fiA", "fi"], [3315, 36]),
    ("type1-raw-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type0-opentype-cid-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type0-raw-cid-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type0-name-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type0-vertical-cid-cff.pdf", ["fiA", "fi"], [2, 1]),
    ("type1-internal-cff.pdf", ["ﬁA", "ﬁ"], [2, 1]),
    ("type1-implicit-base-cff.pdf", ["ﬁﬁ", "ﬁ"], [2, 2]),
    ("type1-dingbats-base-cff.pdf", ["✂✂", "✂"], [2, 2]),
    ("type0-vertical-default.pdf", ["fiA", "fi"], [3315, 36]),
    ("type0-vertical-metrics.pdf", ["fiA", "fi"], [3315, 36]),
    ("type0-vertical-transformed.pdf", ["fiA", "fi"], [3315, 36]),
    ("truetype-ligature-glyph.pdf", ["fiA", "fi"], [3315, 36]),
    ("truetype-glyph-name-sequence.pdf", ["fiA", "fi"], [3315, 36]),
    ("type0-no-unicode-cmap-glyph.pdf", ["fiA", "fi"], [3315, 36]),
    ("type0-variation-glyph.pdf", ["0\ufe00", "0\ufe00"], [19]),
])
def test_imports_pdf_unicode_sequences_with_original_glyphs(tmp_path, name, text, glyphs):
    source = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / name
    with Document.from_pdf_bytes(source.read_bytes()) as document:
        assert document.page_text(0) == text
        objects = document.page_text_objects(0)
        assert objects[0]["cg_transforms"][0]["glyphs"] == glyphs
        assert len([font for font in document.fonts() if font["embedded_path"] is not None]) == 1
        png = document.page_png(0, 72)
        target = tmp_path / "glyphs.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_text(0) == text
        assert reopened.page_text_objects(0) == objects
        assert reopened.page_png(0, 72) == png
        assert reopened.validate_references() == []


def test_imports_macroman_truetype_pdf_with_embedded_font(tmp_path):
    fixtures = Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" / "fixtures"
    source = (fixtures / "truetype-macroman.pdf").read_bytes()
    expected_font = (fixtures / "dejavu-sans.ttf").read_bytes()
    with Document.from_pdf_bytes(source) as document:
        assert document.page_text(0) == ["é"]
        embedded = [(index, font) for index, font in enumerate(document.fonts())
                    if font["embedded_path"] is not None]
        assert len(embedded) == 1
        assert embedded[0][1]["font_name"] == "DejaVu Sans"
        assert document.font_data(embedded[0][0]) == expected_font
        target = tmp_path / "macroman.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_text(0) == ["é"]
        assert reopened.font_data(embedded[0][0]) == expected_font


@pytest.mark.parametrize("name", ["optional-content.pdf", "optional-content-indirect.pdf"])
def test_imports_pdf_optional_content_visibility(tmp_path, name):
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / name)
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert document.page_text(0) == ["B"]
        assert len(document.page_text_objects(0)) == 1
        target = tmp_path / "optional.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_text(0) == ["B"]


def test_imports_pdf_document_info():
    source = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
              "tests" / "fixtures" / "metadata-tounicode.pdf")
    with Document.from_pdf_bytes(source.read_bytes()) as document:
        info = document.documents()[0]
        assert info["title"] == "示例文档"
        assert info["author"] == "Alice"
        assert info["creation_date"] == "2024-02-29"
        assert info["keywords"] == ["测试, Rust"]


def test_imports_pdf_outline_tree():
    source = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
              "tests" / "fixtures" / "outline-tounicode.pdf")
    with Document.from_pdf_bytes(source.read_bytes()) as document:
        outlines = document.outlines()
        assert [item["title"] for item in outlines] == ["章节一", "Section 1.1", "Chapter 2"]
        assert [item["parent_index"] for item in outlines] == [None, 0, None]
        assert not outlines[0]["expanded"]


def test_document_color_space_round_trip_and_validation(tmp_path):
    target = tmp_path / "colors.ofd"
    with Document.new() as document:
        document.add_blank_page(20, 20)
        color_id = document.add_document_color_space(0, {
            "color_type": "RGB", "palette": [[255, 0, 0], [0, 0, 255]],
        })
        spaces = document.document_color_spaces(0)
        assert len(spaces) == 1
        assert spaces[0]["id"] == str(color_id)
        assert spaces[0]["bits_per_component"] == 8
        assert spaces[0]["palette"] == [[255, 0, 0], [0, 0, 255]]
        assert document.page_color_spaces(0) == spaces
        assert document.document_default_color_space(0) is None
        with pytest.raises(OfdError):
            document.set_document_default_color_space(0, "999999")
        document.set_document_default_color_space(0, color_id)
        assert document.document_default_color_space(0) == str(color_id)
        with pytest.raises(OfdError):
            document.remove_document_color_space(0, color_id)
        page_id = document.add_page_color_space(0, {
            "color_type": "GRAY", "bits_per_component": 4,
        })
        assert page_id > color_id
        page_spaces = document.page_color_spaces(0)
        assert [space["id"] for space in page_spaces] == [str(page_id), str(color_id)]
        assert len(document.document_color_spaces(0)) == 1
        document.set_document_color_space(0, color_id, {
            "color_type": "RGB", "palette": [[0, 255, 0]],
        })
        document.set_page_color_space(0, page_id, {
            "color_type": "GRAY", "bits_per_component": 4, "palette": [[15]],
        })
        spaces = document.document_color_spaces(0)
        page_spaces = document.page_color_spaces(0)
        assert spaces[0]["palette"] == [[0, 255, 0]]
        assert page_spaces[0]["palette"] == [[15]]
        assert document.document_default_color_space(0) == str(color_id)
        with pytest.raises(OfdError):
            document.set_document_color_space(0, page_id, {"color_type": "GRAY"})
        with pytest.raises(OfdError):
            document.set_page_color_space(0, page_id, {
                "color_type": "GRAY", "bits_per_component": 4, "palette": [[16]],
            })
        assert document.page_color_spaces(0) == page_spaces
        with pytest.raises(OfdError):
            document.add_document_color_space(0, {"color_type": "RGB", "palette": [[256, 0, 0]]})
        with pytest.raises(IndexError):
            document.page_color_spaces(-1)
        assert document.document_color_spaces(0) == spaces
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.page_color_spaces(0) == page_spaces
        assert reopened.document_default_color_space(0) == str(color_id)
        reopened.set_document_default_color_space(0, None)
        assert reopened.document_default_color_space(0) is None


def test_remove_unreferenced_color_spaces(tmp_path):
    target = tmp_path / "removed-colors.ofd"
    with Document.new() as document:
        document.add_blank_page(20, 20)
        public = document.add_document_color_space(0, {"color_type": "RGB"})
        local = document.add_page_color_space(0, {"color_type": "GRAY"})
        document.set_document_default_color_space(0, public)
        with pytest.raises(OfdError):
            document.remove_document_color_space(0, public)
        document.remove_page_color_space(0, local)
        with pytest.raises(OfdError):
            document.remove_page_color_space(0, public)
        document.set_document_default_color_space(0, None)
        document.remove_document_color_space(0, public)
        assert document.page_color_spaces(0) == []
        document.save(target)
    with Document.open(target) as reopened:
        assert reopened.document_color_spaces(0) == []


def test_reports_missing_color_space_reference():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        page = document.pages()[0]["path"]
        xml = document.entry(page).decode()
        document.set_entry(page, xml.replace(
            "</ofd:Content>",
            '<ofd:Layer ID="500"><ofd:PathObject ID="501" Boundary="0 0 1 1" Fill="true">'
            '<ofd:FillColor ColorSpace="999" Index="0"/>'
            '<ofd:AbbreviatedData>M 0 0 L 1 1</ofd:AbbreviatedData>'
            '</ofd:PathObject></ofd:Layer></ofd:Content>',
        ).encode())
        assert any(issue["code"] == "color_space_reference"
                   for issue in document.validate_references())
        color_id = document.add_document_color_space(0, {
            "color_type": "RGB", "palette": [[255, 0, 0]],
        })
        document.set_entry(page, document.entry(page).replace(
            b'ColorSpace="999" Index="0"',
            f'ColorSpace="{color_id}" Index="1"'.encode(),
        ))
        assert any(issue["code"] == "invalid_color_index"
                   for issue in document.validate_references())


def test_reads_and_renders_java_nested_page_block():
    with Document.open(FIXTURES / "helloworld_with_pageblock.ofd") as document:
        assert document.page_text(0) == ["你好呀，OFD Reader&Writer！"]
        assert document.page_layers(0)[0]["object_count"] == 1
        assert len(document.page_objects(0)) == 1
        assert len(document.page_text_objects(0)) == 1
        assert len(document.page_object_paints(0)) == 1
        assert "OFD Reader&amp;Writer" in document.page_svg(0)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        object_id = document.page_objects(0)[0]["id"]
        document.set_page_object_visible(0, object_id, False)
        assert document.page_text(0) == []
        document.set_page_object_visible(0, object_id, True)
        assert document.page_text(0) == ["你好呀，OFD Reader&Writer！"]
        added = document.add_text_object(0, "outside", (10, 20, 40, 10), 5)
        document.move_page_object(0, added, object_id)
        assert document.page_text(0) == ["outside", "你好呀，OFD Reader&Writer！"]
        document.move_page_object(0, object_id, added)
        assert document.page_text(0) == ["你好呀，OFD Reader&Writer！", "outside"]


def test_groups_page_objects_into_nested_blocks():
    with Document.new() as document:
        document.add_text_page("grouped", 30, 20)
        first = document.page_objects(0)[0]["id"]
        second = document.add_path_object(0, "M 0 0 L 3 3", (1, 1, 4, 4))
        before = document.page_svg(0)
        document.group_page_objects(0, first, second)
        document.group_page_objects(0, first, first)
        page = document.pages()[0]["path"]
        assert document.entry(page).count(b"<ofd:PageBlock>") == 2
        assert document.page_svg(0) == before
        assert document.page_text(0) == ["grouped"]
        assert document.validate_references() == []
        snapshot = document.entry(page)
        with pytest.raises(OfdError):
            document.group_page_objects(0, first, "missing")
        assert document.entry(page) == snapshot
        document.ungroup_page_objects(0, first)
        document.ungroup_page_objects(0, second)
        assert b"PageBlock" not in document.entry(page)
        with pytest.raises(OfdError):
            document.ungroup_page_objects(0, first)


def test_promotes_and_applies_page_template(tmp_path):
    path = tmp_path / "template.ofd"
    with Document.new() as document:
        document.add_text_page("shared", 30, 20)
        document.add_blank_page(30, 20)
        object_id = document.page_objects(0)[0]["id"]
        document.add_page_object_uri_action(0, object_id, "https://example.org/template")
        template_id = document.promote_page_to_template(0, [1])
        assert document.page_text(0) == []
        assert document.page_text_with_templates(0) == ["shared"]
        assert document.page_text_with_templates(1) == ["shared"]
        assert document.page_sources(1)[0]["template_id"] == str(template_id)
        source_path = document.page_sources(1)[0]["path"]
        assert document.page_source_object_actions(1, source_path, object_id)[0]["attributes"]["URI"] == "https://example.org/template"
        with pytest.raises(OfdError):
            document.page_source_object_actions(1, "OFD.xml", object_id)
        document.add_blank_page(30, 20)
        document.apply_page_template(2, template_id, "Foreground")
        assert document.page_text_with_templates(2) == ["shared"]
        assert document.to_pdf().count(b"/Subtype /Link") == 3
        before = document.page_sources(2)
        with pytest.raises(OfdError):
            document.apply_page_template(2, template_id)
        with pytest.raises(OfdError):
            document.promote_page_to_template(0, [2])
        with pytest.raises(IndexError):
            document.promote_page_to_template(0, [-1])
        assert document.page_sources(2) == before
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text_with_templates(2) == ["shared"]
        assert reopened.page_source_object_actions(2, source_path, object_id)[0]["kind"] == "URI"
        assert reopened.validate_references() == []


def test_reports_missing_font_in_template_page():
    with Document.new() as document:
        document.add_text_page("template", 30, 20)
        document.add_blank_page(30, 20)
        document.promote_page_to_template(0, [1])
        source = document.page_sources(1)[0]["path"]
        xml = document.entry(source).decode()
        font = ElementTree.fromstring(xml).find(".//{*}TextObject").attrib["Font"]
        document.set_entry(source, xml.replace(f'Font="{font}"', 'Font="999999"', 1).encode())
        assert any(
            issue["path"] == source and issue["code"] == "missing_font"
            for issue in document.validate_references()
        )


def test_removes_template_reference_and_definition(tmp_path):
    path = tmp_path / "removed-template.ofd"
    with Document.new() as document:
        document.add_text_page("shared", 30, 20)
        document.add_blank_page(30, 20)
        template_id = document.promote_page_to_template(0, [1])
        with pytest.raises(OfdError):
            document.remove_document_template(0, template_id)
        with pytest.raises(IndexError):
            document.remove_page_template(-1, template_id)
        with pytest.raises(IndexError):
            document.remove_document_template(-1, template_id)
        with pytest.raises(OfdError):
            document.remove_page_template(1, "missing")
        document.remove_page_template(1, template_id)
        assert document.page_text_with_templates(1) == []
        assert document.page_text_with_templates(0) == ["shared"]
        document.remove_page_template(0, template_id)
        document.remove_document_template(0, template_id)
        with pytest.raises(OfdError):
            document.apply_page_template(1, template_id)
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text_with_templates(0) == []
        assert reopened.validate_references() == []


def test_adds_paginated_table_with_repeated_header(tmp_path):
    path = tmp_path / "table.ofd"
    rows = [["H1", "H2"], ["A\nA2", "B"], ["C", "D"], ["E", "F"]]
    with Document.new() as document:
        pages = document.add_table(
            0, rows, [12, 12], page_size=(30, 26), margins=(2, 2, 2, 2),
            row_height=8, font_size=3, line_height=3, padding=1,
            repeat_header=True,
        )
        assert pages == [0, 1, 2]
        assert document.page_text(0) == ["H1", "H2", "A", "A2", "B"]
        assert document.page_text(1) == ["H1", "H2", "C", "D"]
        assert len(document.page_paths(2)) == 4
        auto = document.add_table(
            0, rows, [12, 12], page_size=(30, 26), margins=(2, 2, 2, 2),
            row_height=0, font_size=3, line_height=3, padding=1,
            repeat_header=True,
        )
        assert auto == [3, 4]
        assert document.page_paths(3)[0]["boundary"][3] == 5
        assert document.page_paths(3)[2]["boundary"][3] == 8
        assert document.page_text(4) == ["H1", "H2", "E", "F"]
        before = document.page_count
        with pytest.raises(OfdError):
            document.add_table(0, rows, [12], page_size=(30, 26))
        with pytest.raises(ValueError):
            document.add_table(0, rows, [float("nan"), 12])
        assert document.page_count == before
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(2) == ["H1", "H2", "E", "F"]
        assert reopened.page_text(4) == ["H1", "H2", "E", "F"]
        assert reopened.validate_references() == []


def test_adds_merged_table_cells_and_preserves_row_groups(tmp_path):
    cells = [
        {"row": 0, "column": 0, "text": "H1"},
        {"row": 0, "column": 1, "text": "H2"},
        {"row": 1, "column": 0, "row_span": 2, "text": "L1\nL2\nL3\nL4"},
        {"row": 1, "column": 1, "text": "Top"},
        {"row": 2, "column": 1, "text": "Low"},
        {"row": 3, "column": 0, "column_span": 2, "text": "Wide"},
    ]
    options = dict(page_size=(30, 23), margins=(2, 2, 2, 2),
                   row_height=0, font_size=3, line_height=3, padding=1,
                   repeat_header=True)
    path = tmp_path / "merged-table.ofd"
    with Document.new() as document:
        with pytest.raises(IndexError):
            document.add_table_cells(-1, 4, cells, [12, 12], **options)
        with pytest.raises(IndexError):
            document.add_table_cells(0, -1, cells, [12, 12], **options)
        assert document.add_table_cells(0, 4, cells, [12, 12], **options) == [0, 1]
        assert document.page_paths(0)[2]["boundary"] == [2, 7, 12, 14]
        assert document.page_paths(1)[2]["boundary"] == [2, 7, 24, 5]
        assert document.page_text(1) == ["H1", "H2", "Wide"]
        with pytest.raises(OfdError):
            document.add_table_cells(0, 4, cells[:-1], [12, 12], **options)
        assert document.page_count == 2
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(1) == ["H1", "H2", "Wide"]
        assert reopened.validate_references() == []


def test_styles_individual_table_cells(tmp_path):
    path = tmp_path / "styled-table.ofd"
    cells = [{"row": 0, "column": 0, "text": "A", "background_color": [20, 30, 40],
              "text_color": [1, 2, 3], "bold": True, "italic": True,
              "underline": True, "strikethrough": True,
              "align": "right"}]
    with Document.new() as document:
        assert document.add_table_cells(0, 1, cells, [20], page_size=(30, 20),
                                        margins=(2, 2, 2, 2), row_height=8,
                                        font_size=3, line_height=3) == [0]
        assert len(document.page_paths(0)) == 4
        assert document.page_paths(0)[0]["fill"] is True
        assert document.page_text_objects(0)[0]["boundary"][0] > 10
        assert document.page_text_objects(0)[0]["weight"] == 700
        assert document.page_text_objects(0)[0]["italic"] is True
        with pytest.raises(OfdError):
            document.add_table_cells(0, 1, [dict(cells[0], align="justify")], [20])
        with pytest.raises(OfdError):
            document.add_table_cells(0, 1, [dict(cells[0], bold="yes")], [20])
        with pytest.raises(OfdError):
            document.add_table_cells(0, 1, [dict(cells[0], underline="yes")], [20])
        assert document.page_count == 1
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["A"]
        assert reopened.page_text_objects(0)[0]["weight"] == 700
        assert reopened.page_text_objects(0)[0]["italic"] is True
        assert len(reopened.page_paths(0)) == 4
        assert reopened.validate_references() == []


def test_table_cell_text_geometry(tmp_path):
    path = tmp_path / "cell-geometry.ofd"
    cell = {"row": 0, "column": 0, "text": "A\nB", "font_size": 5, "line_height": 7}
    options = dict(page_size=(30, 50), margins=(2, 2, 2, 2),
                   row_height=0, font_size=3, line_height=4, padding=1)
    with Document.new() as document:
        assert document.add_table_cells(0, 1, [cell], [20], **options) == [0]
        objects = document.page_text_objects(0)
        assert [item["size"] for item in objects] == [5, 5]
        assert objects[1]["boundary"][1] - objects[0]["boundary"][1] == 7
        assert document.page_paths(0)[0]["boundary"][3] == 16
        for bad in ({**cell, "font_size": 0}, {**cell, "line_height": 4},
                    {**cell, "font_size": "large"}):
            with pytest.raises(OfdError):
                document.add_table_cells(0, 1, [bad], [20], **options)
        assert document.page_count == 1
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["A", "B"]


def test_table_cell_edge_padding(tmp_path):
    path = tmp_path / "cell-padding.ofd"
    cell = {"row": 0, "column": 0, "text": "A\nB", "padding": [2, 3, 4, 5]}
    options = dict(page_size=(30, 30), margins=(2, 2, 2, 2),
                   row_height=0, font_size=3, line_height=4, padding=1)
    with Document.new() as document:
        assert document.add_table_cells(0, 1, [cell], [20], **options) == [0]
        objects = document.page_text_objects(0)
        assert [item["boundary"][0:2] for item in objects] == [[7, 4], [7, 8]]
        assert document.page_paths(0)[0]["boundary"][3] == 14
        for bad in ([1, 10, 1, 10], [-1, 1, 1, 1], [1, 2, 3]):
            with pytest.raises(OfdError):
                document.add_table_cells(0, 1, [dict(cell, padding=bad)], [20], **options)
        assert document.page_count == 1
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["A", "B"]


def test_table_cell_border_override_and_omission(tmp_path):
    path = tmp_path / "cell-borders.ofd"
    cells = [{"row": 0, "column": 0, "text": "A",
              "border_width": 0.8, "border_color": [255, 0, 0]},
             {"row": 0, "column": 1, "text": "B", "border_width": 0}]
    options = dict(page_size=(50, 20), margins=(2, 2, 2, 2),
                   row_height=8, font_size=3, line_height=3)
    with Document.new() as document:
        assert document.add_table_cells(0, 1, cells, [20, 20], **options) == [0]
        assert len(document.page_paths(0)) == 1
        assert '#ff0000' in document.page_svg(0)
        with pytest.raises(OfdError):
            document.add_table_cells(0, 1,
                                     [dict(cells[0], border_width=-1), cells[1]],
                                     [20, 20], **options)
        assert document.page_count == 1
        document.save(path)
    with Document.open(path) as reopened:
        assert len(reopened.page_paths(0)) == 1
        assert reopened.page_text(0) == ["A", "B"]


def test_table_cell_letter_spacing_sets_text_deltas(tmp_path):
    path = tmp_path / "table-spacing.ofd"
    cells = [{"row": 0, "column": 0, "text": "ABCD"},
             {"row": 0, "column": 1, "text": "ABCD", "letter_spacing": 1}]
    options = dict(page_size=(50, 20), margins=(2, 2, 2, 2),
                   row_height=8, font_size=3, line_height=4)
    with Document.new() as document:
        assert document.add_table_cells(0, 1, cells, [20, 20], **options) == [0]
        objects = document.page_text_objects(0)
        assert abs(objects[1]["boundary"][2] - objects[0]["boundary"][2] - 3) < 1e-9
        assert len(objects[0]["codes"][0]["delta_x"].split()) == 3
        assert len(objects[1]["codes"][0]["delta_x"].split()) == 3
        unicode_cells = [dict(cells[0], text="e\u0301X"),
                         dict(cells[1], text="e\u0301X")]
        assert document.add_table_cells(0, 1, unicode_cells,
                                        [20, 20], **options) == [1]
        unicode_objects = document.page_text_objects(1)
        assert abs(unicode_objects[1]["boundary"][2] -
                   unicode_objects[0]["boundary"][2] - 1) < 1e-9
        assert [float(value) for value in
                unicode_objects[1]["codes"][0]["delta_x"].split()][0] == 0
        with pytest.raises(OfdError):
            document.add_table_cells(0, 1,
                                     [cells[0], dict(cells[1], letter_spacing=-100)],
                                     [20, 20], **options)
        assert document.page_count == 2
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text_objects(0)[1]["codes"][0]["delta_x"] is not None
        assert reopened.page_text(1) == ["e\u0301X", "e\u0301X"]


def test_table_cells_select_font_resources(tmp_path):
    font = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" /
            "fixtures" / "dejavu-sans.ttf").read_bytes()
    path = tmp_path / "table-fonts.ofd"
    with Document.new() as document:
        serif = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" /
                 "fixtures" / "dejavu-serif.ttf").read_bytes()
        default_id = str(document.add_embedded_font("DejaVu Serif", serif))
        selected_id = document.add_embedded_font("DejaVu Sans", font)
        cells = [{"row": 0, "column": 0, "text": "WWWW", "font_id": None},
                 {"row": 0, "column": 1, "text": "WWWW", "font_id": selected_id}]
        options = dict(page_size=(50, 30), margins=(2, 2, 2, 2),
                       row_height=0, font_size=3, line_height=4, padding=1,
                       font_id=default_id)
        assert document.add_table_cells(0, 1, cells, [20, 20], **options) == [0]
        objects = document.page_text_objects(0)
        assert [item["font_id"] for item in objects] == [default_id, str(selected_id)]
        assert objects[0]["boundary"][2] != objects[1]["boundary"][2]
        with pytest.raises(OfdError):
            document.add_table_cells(0, 1,
                                     [dict(cells[0], font_id="999999"), cells[1]],
                                     [20, 20], **options)
        assert document.page_count == 1
        document.save(path)
    with Document.open(path) as reopened:
        assert [item["font_id"] for item in reopened.page_text_objects(0)] == [
            default_id, str(selected_id)]


def test_table_cell_image_sizes_row_and_round_trips(tmp_path):
    with Document.new() as source:
        source.add_blank_page(10, 10)
        png = source.page_png(0, 25.4)
    path = tmp_path / "image-table.ofd"
    cell = {"row": 0, "column": 0, "text": "C",
            "image": {"data": png, "format": "png", "width": 8, "height": 6}}
    options = dict(page_size=(20, 20), margins=(2, 2, 2, 2),
                   row_height=0, font_size=3, line_height=3, padding=1)
    with Document.new() as document:
        assert document.add_table_cells(0, 1, [cell], [12], **options) == [0]
        assert document.page_images(0)[0]["boundary"] == [3, 7, 8, 6]
        assert document.page_text(0) == ["C"]
        with pytest.raises(OfdError):
            invalid = dict(cell, image=dict(cell["image"], width=11))
            document.add_table_cells(0, 1, [invalid], [12], **options)
        assert document.page_count == 1
        document.save(path)
    with Document.open(path) as reopened:
        assert len(reopened.page_images(0)) == 1
        assert reopened.validate_references() == []


def test_ordered_table_cell_blocks(tmp_path):
    with Document.new() as source:
        source.add_blank_page(10, 10)
        png = source.page_png(0, 25.4)
    path = tmp_path / "block-table.ofd"
    blocks = [
        {"type": "text", "text": "A", "align": "left"},
        {"type": "image", "data": png, "format": "png", "width": 6, "height": 4},
        {"type": "text", "text": "B", "text_color": [4, 5, 6], "align": "right"},
        {"type": "image", "data": png, "format": "png", "width": 8, "height": 5},
    ]
    cell = {"row": 0, "column": 0, "align": "center", "blocks": blocks}
    options = dict(page_size=(40, 40), margins=(2, 2, 2, 2),
                   row_height=0, font_size=3, line_height=3, padding=1)
    with Document.new() as document:
        assert document.add_table_cells(0, 1, [cell], [20], **options) == [0]
        assert document.page_text(0) == ["A", "B"]
        assert [image["boundary"] for image in document.page_images(0)] == [
            [9, 7, 6, 4], [8, 16, 8, 5]]
        with pytest.raises(OfdError):
            document.add_table_cells(0, 1, [dict(cell, text="legacy")], [20], **options)
        assert document.page_count == 1
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["A", "B"]
        assert reopened.validate_references() == []


def test_table_spacer_and_rule_blocks(tmp_path):
    path = tmp_path / "rule-table.ofd"
    cell = {"row": 0, "column": 0, "blocks": [
        {"type": "spacer", "height": 3},
        {"type": "rule", "thickness": 1, "color": [255, 0, 0]},
        {"type": "spacer", "height": 2},
    ]}
    options = dict(page_size=(40, 40), margins=(2, 2, 2, 2),
                   row_height=0, font_size=3, line_height=3, padding=1)
    with Document.new() as document:
        assert document.add_table_cells(0, 1, [cell], [20], **options) == [0]
        assert document.page_text(0) == []
        assert document.page_paths(0)[1]["boundary"] == [3, 7, 18, 1]
        with pytest.raises(OfdError):
            bad = dict(cell, blocks=[{"type": "spacer", "height": 0}])
            document.add_table_cells(0, 1, [bad], [20], **options)
        document.save(path)
    with Document.open(path) as reopened:
        assert len(reopened.page_paths(0)) == 2


def test_nested_table_with_image_and_invalid_grid(tmp_path):
    with Document.new() as source:
        source.add_blank_page(10, 10)
        png = source.page_png(0, 25.4)
    inner = {"row_count": 1, "column_widths": [7, 7], "row_height": 10, "cells": [
        {"row": 0, "column": 0, "text": "A"},
        {"row": 0, "column": 1, "vertical_align": "bottom", "image": {
            "data": png, "format": "png", "width": 3, "height": 3}},
    ]}
    cell = {"row": 0, "column": 0, "blocks": [
        {"type": "text", "text": "Before"},
        {"type": "table", "table": inner},
        {"type": "text", "text": "After"},
    ]}
    options = dict(page_size=(40, 40), margins=(2, 2, 2, 2),
                   row_height=0, font_size=3, line_height=3, padding=1)
    path = tmp_path / "nested-table.ofd"
    with Document.new() as document:
        assert document.add_table_cells(0, 1, [cell], [20], **options) == [0]
        assert document.page_text(0) == ["Before", "A", "After"]
        assert len(document.page_images(0)) == 1
        assert document.page_images(0)[0]["boundary"][1] == 13
        bad = dict(cell, blocks=[{"type": "table", "table": dict(inner, cells=inner["cells"][:1])}])
        with pytest.raises(OfdError):
            document.add_table_cells(0, 1, [bad], [20], **options)
        bad_align = dict(cell, vertical_align="middle")
        with pytest.raises(OfdError):
            document.add_table_cells(0, 1, [bad_align], [20], **options)
        assert document.page_count == 1
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["Before", "A", "After"]
        assert reopened.validate_references() == []


def test_image_only_table_does_not_require_text(tmp_path):
    with Document.new() as source:
        source.add_blank_page(10, 10)
        png = source.page_png(0, 25.4)
    path = tmp_path / "image-only-table.ofd"
    cell = {"row": 0, "column": 0, "blocks": [
        {"type": "image", "data": png, "format": "png", "width": 8, "height": 6}]}
    with Document.new() as document:
        assert document.add_table_cells(0, 1, [cell], [12], page_size=(20, 20),
                                        margins=(2, 2, 2, 2), row_height=0,
                                        font_size=3, line_height=3, padding=1) == [0]
        assert document.page_text(0) == []
        assert len(document.page_images(0)) == 1
        document.save(path)
    with Document.open(path) as reopened:
        assert len(reopened.page_images(0)) == 1
        assert reopened.validate_references() == []


def test_reads_java_fixture():
    with Document.open(FIXTURES / "helloworld.ofd") as document:
        assert document.page_count == 1
        assert "OFD.xml" in document.entries()
        assert document.pages()[0]["physical_box"] == [0, 0, 210, 297]
        assert document.page_text(0) == ["你好呀，OFD Reader&Writer！"]
        obj = document.page_text_objects(0)[0]
        assert obj["boundary"] == [31.7, 25.4, 40.5, 5]
        assert obj["font_id"] == "3"
        assert obj["codes"][0]["text"] == "你好呀，OFD Reader&Writer！"
        with pytest.raises(IndexError):
            document.page_text_objects(-1)
        with pytest.raises(OfdError, match="out of bounds"):
            document.page_text_objects(1)


def test_replaces_unshaped_text_code_atomically():
    with Document.new() as document:
        document.add_text_page("AB", 20, 20)
        obj_id = document.page_text_objects(0)[0]["id"]
        document.set_page_text_code(0, obj_id, 0, "A & <B>")
        assert document.page_text(0) == ["A & <B>"]
        assert document.page_text_objects(0)[0]["codes"][0]["text"] == "A & <B>"
        assert len(document.page_text_objects(0)[0]["codes"][0]["delta_x"].split()) == 6
        assert "A &amp; &lt;B&gt;" in document.page_svg(0)
        path = document.pages()[0]["path"]
        before = document.entry(path)
        with pytest.raises(OfdError):
            document.set_page_text_code(0, obj_id, 1, "X")
        with pytest.raises(IndexError):
            document.set_page_text_code(0, obj_id, -1, "X")
        assert document.entry(path) == before


def test_sets_text_code_layout_atomically():
    with Document.new() as document:
        document.add_text_page("AB", 30, 20)
        obj_id = document.page_text_objects(0)[0]["id"]
        document.set_page_text_code_layout(
            0, obj_id, 0, {"x": 2, "y": 7, "delta_x": [3.5]})
        code = document.page_text_objects(0)[0]["codes"][0]
        assert (code["x"], code["y"], code["delta_x"]) == (2, 7, "3.5")
        assert 'x="2 5.5" y="7 7"' in document.page_svg(0)
        path = document.pages()[0]["path"]
        before = document.entry(path)
        with pytest.raises(OfdError):
            document.set_page_text_code_layout(
                0, obj_id, 0, {"x": 2, "y": 7, "delta_x": [1, 2]})
        with pytest.raises(IndexError):
            document.set_page_text_code_layout(0, obj_id, -1, {})
        assert document.entry(path) == before


def test_appends_positioned_text_code_atomically(tmp_path):
    path = tmp_path / "appended-code.ofd"
    with Document.new() as document:
        document.add_text_page("AB", 30, 20)
        obj_id = document.page_text_objects(0)[0]["id"]
        document.append_page_text_code(
            0, obj_id, "C&D", {"delta_x": [2, 2.5]})
        assert document.page_text(0) == ["AB", "C&D"]
        codes = document.page_text_objects(0)[0]["codes"]
        assert codes[1]["delta_x"] == "2 2.5"
        assert (codes[1]["x"], codes[1]["y"]) == (None, None)
        assert 'x="0 2 4.5" y="5 5 5"' in document.page_svg(0)
        page_path = document.pages()[0]["path"]
        before = document.entry(page_path)
        with pytest.raises(OfdError):
            document.append_page_text_code(
                0, obj_id, "XY", {"delta_x": [1, 2]})
        assert document.entry(page_path) == before
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["AB", "C&D"]


def test_removes_text_code_without_moving_inherited_run():
    with Document.new() as document:
        document.add_text_page("A", 30, 20)
        obj_id = document.page_text_objects(0)[0]["id"]
        document.append_page_text_code(0, obj_id, "B", {"x": 5, "y": 6})
        document.append_page_text_code(0, obj_id, "C", {})
        document.remove_page_text_code(0, obj_id, 1)
        assert document.page_text(0) == ["A", "C"]
        code = document.page_text_objects(0)[0]["codes"][1]
        assert (code["x"], code["y"]) == (5, 6)
        page_path = document.pages()[0]["path"]
        before = document.entry(page_path)
        with pytest.raises(OfdError):
            document.remove_page_text_code(0, obj_id, 4)
        with pytest.raises(IndexError):
            document.remove_page_text_code(0, obj_id, -1)
        assert document.entry(page_path) == before
        document.remove_page_text_code(0, obj_id, 0)
        assert document.page_text(0) == ["C"]
        with pytest.raises(OfdError):
            document.remove_page_text_code(0, obj_id, 0)


def test_sets_text_font_and_rejects_invalid_references(tmp_path):
    font = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
            "tests" / "fixtures" / "dejavu-serif.ttf").read_bytes()
    path = tmp_path / "font-edit.ofd"
    with Document.new() as document:
        document.add_text_page("AB", 30, 20)
        original = document.page_text_objects(0)[0]
        font_id = document.add_embedded_font("DejaVu Serif", font)
        document.set_page_text_font(0, original["id"], str(font_id))
        assert document.page_text_objects(0)[0]["font_id"] == str(font_id)
        assert 'font-family="ofd-embedded-font-0"' in document.page_svg(0)
        page_path = document.pages()[0]["path"]
        before = document.entry(page_path)
        with pytest.raises(OfdError):
            document.set_page_text_font(0, original["id"], "999999")
        with pytest.raises(OfdError):
            document.set_page_text_font(0, "missing", str(font_id))
        with pytest.raises(ValueError):
            document.set_page_text_font(0, original["id"], "bad\x00id")
        assert document.entry(page_path) == before
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text_objects(0)[0]["font_id"] == str(font_id)


def test_sets_text_paint_flags_and_restores_defaults():
    with Document.new() as document:
        document.add_text_page("AB", 30, 20)
        obj_id = document.page_text_objects(0)[0]["id"]
        document.set_page_text_paint_flags(0, obj_id, fill=False, stroke=True)
        paint = document.page_object_paints(0)[0]
        assert not paint["fill_enabled"]
        assert paint["stroke_enabled"]
        assert 'fill="none"' in document.page_svg(0)
        page_path = document.pages()[0]["path"]
        before = document.entry(page_path)
        with pytest.raises(OfdError):
            document.set_page_text_paint_flags(0, "missing", fill=True)
        with pytest.raises(OfdError):
            document.set_page_text_paint_flags(0, obj_id, fill="false")
        assert document.entry(page_path) == before
        document.set_page_text_paint_flags(0, obj_id)
        paint = document.page_object_paints(0)[0]
        assert paint["fill_enabled"]
        assert not paint["stroke_enabled"]
        assert b' Fill="' not in document.entry(page_path)
        assert b' Stroke="' not in document.entry(page_path)


def test_sets_text_typography_atomically():
    with Document.new() as document:
        document.add_text_page("AB", 20, 20)
        obj_id = document.page_text_objects(0)[0]["id"]
        document.set_page_text_typography(0, obj_id, {
            "size": 4, "h_scale": 0.75, "read_direction": 180,
            "char_direction": 90, "weight": 700, "italic": True,
        })
        obj = document.page_text_objects(0)[0]
        assert obj["size"] == 4
        assert obj["h_scale"] == 0.75
        assert (obj["read_direction"], obj["char_direction"],
                obj["weight"], obj["italic"]) == (180, 90, 700, True)
        path = document.pages()[0]["path"]
        before = document.entry(path)
        with pytest.raises(OfdError):
            document.set_page_text_typography(0, obj_id, {"size": 4, "weight": 750})
        assert document.entry(path) == before
        document.set_page_text_typography(0, obj_id, {"size": 3})
        obj = document.page_text_objects(0)[0]
        assert obj["size"] == 3
        assert obj["h_scale"] is None
        assert (obj["read_direction"], obj["char_direction"],
                obj["weight"], obj["italic"]) == (0, 0, 400, False)


def test_reads_text_typography_attributes_and_defaults():
    with Document.new() as document:
        document.add_text_page("AB", 20, 20)
        obj = document.page_text_objects(0)[0]
        assert (obj["read_direction"], obj["char_direction"],
                obj["weight"], obj["italic"]) == (0, 0, 400, False)
        path = document.pages()[0]["path"]
        source = document.entry(path)
        styled = source.replace(
            b"<ofd:TextObject ID=",
            b'<ofd:TextObject ReadDirection="180" CharDirection="90" Weight="700" Italic="true" ID=',
        )
        document.set_entry(path, styled)
        obj = document.page_text_objects(0)[0]
        assert (obj["read_direction"], obj["char_direction"],
                obj["weight"], obj["italic"]) == (180, 90, 700, True)
        document.set_entry(path, styled.replace(b'Weight="700"', b'Weight="750"'))
        with pytest.raises(OfdError):
            document.page_text_objects(0)


def test_extracts_text_in_page_region(tmp_path):
    with Document.new() as document:
        document.add_text_page("区域", 80, 50)
        assert document.page_text_in_region(0, 9, 9, 20, 10) == ["区域"]
        assert document.page_text_in_region(0, 0, 0, 5, 5) == []
        with pytest.raises(IndexError):
            document.page_text_in_region(-1, 0, 0, 20, 20)
        with pytest.raises(OfdError, match="positive dimensions"):
            document.page_text_in_region(0, 0, 0, 0, 20)
        path = tmp_path / "region.ofd"
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text_in_region(0, 9, 9, 20, 10) == ["区域"]
    with pytest.raises(OfdError):
        document.page_text_in_region(0, 9, 9, 20, 10)


def test_finds_keyword_positions_in_java_fixture_and_checks_pages():
    with Document.open(FIXTURES / "keyword.ofd") as document:
        matches = document.keyword_positions(["办理", "不动产权"])
        assert len(matches) == 3
        assert all(match["page_index"] == 0 for match in matches)
        assert all(match["box_mm"][2] > 0 and match["box_mm"][3] > 0 for match in matches)
        assert document.keyword_positions(["办理"], [0])[0]["keyword"] == "办理"
        assert document.keyword_positions(["办理"], []) == []
        with pytest.raises(OfdError, match="keyword must not be empty"):
            document.keyword_positions([""])
        with pytest.raises(OfdError, match="out of bounds"):
            document.keyword_positions(["办理"], [1])


def test_keyword_search_does_not_join_separate_text_objects():
    with Document.new() as document:
        document.add_blank_page(80, 50)
        document.add_text_object(0, "A", (10, 20, 10, 8), 5)
        document.add_text_object(0, "B", (20, 20, 10, 8), 5)
        assert document.page_text(0) == ["A", "B"]
        assert document.keyword_positions(["AB"]) == []
        assert len(document.keyword_positions(["A", "B"])) == 2
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.keyword_positions(["AB"]) == []


def test_keyword_box_uses_character_direction_and_horizontal_scale():
    with Document.new() as document:
        document.add_text_page("A", 30, 20)
        obj_id = document.page_text_objects(0)[0]["id"]
        document.set_page_text_typography(0, obj_id, {
            "size": 5, "h_scale": 0.5, "char_direction": 90,
        })
        matches = document.keyword_positions(["A"])
        assert len(matches) == 1
        assert matches[0]["box_mm"] == [10, 15, 5, 2.5]


def test_moves_pages_and_keeps_order_after_append(tmp_path):
    with Document.new() as document:
        for text in ("A", "B", "C"):
            document.add_text_page(text, 20, 20)
        document.move_page(0, 2)
        document.add_text_page("D", 20, 20)
        assert [document.page_text(i) for i in range(4)] == [
            ["B"], ["C"], ["A"], ["D"]
        ]
        with pytest.raises(IndexError):
            document.move_page(-1, 0)
        with pytest.raises(OfdError, match="out of bounds"):
            document.move_page(0, 4)
        output = tmp_path / "moved.ofd"
        document.save(output)
    with Document.open(output) as reopened:
        assert [reopened.page_text(i) for i in range(4)] == [
            ["B"], ["C"], ["A"], ["D"]
        ]


def test_reorders_all_pages_atomically(tmp_path):
    with Document.new() as document:
        for text in ("A", "B", "C"):
            document.add_text_page(text, 20, 20)
        before = document.to_bytes()
        with pytest.raises(OfdError, match="permutation"):
            document.reorder_document_pages(0, [0, 0, 2])
        assert document.to_bytes() == before
        document.reorder_document_pages(0, [2, 0, 1])
        document.add_text_page("D", 20, 20)
        assert [document.page_text(i) for i in range(4)] == [["C"], ["A"], ["B"], ["D"]]
        output = tmp_path / "reordered.ofd"
        document.save(output)
    with Document.open(output) as reopened:
        assert [reopened.page_text(i) for i in range(4)] == [["C"], ["A"], ["B"], ["D"]]


def test_inserts_pages_at_body_positions(tmp_path):
    output = tmp_path / "inserted.ofd"
    with Document.new() as document:
        document.add_text_page("A", 20, 20)
        document.add_text_page("C", 20, 20)
        assert document.insert_text_page(0, 1, "B", 20, 20) == 1
        assert document.insert_blank_page(0, 0, 20, 20) == 0
        assert [document.page_text(i) for i in range(4)] == [[], ["A"], ["B"], ["C"]]
        before = document.to_bytes()
        with pytest.raises(IndexError):
            document.insert_blank_page(0, -1, 20, 20)
        with pytest.raises(OfdError, match="out of bounds"):
            document.insert_text_page(0, 5, "bad", 20, 20)
        assert document.to_bytes() == before
        assert document.validate_references() == []
        document.save(output)
    with Document.open(output) as reopened:
        assert [reopened.page_text(i) for i in range(4)] == [[], ["A"], ["B"], ["C"]]


def test_duplicates_page_across_binding(tmp_path):
    output = tmp_path / "duplicated.ofd"
    with Document.new() as document:
        document.add_text_page("copy me", 40, 30)
        document.add_text_watermark(0, "copied mark", (1, 1, 12, 6), 4)
        assert document.duplicate_page(0) == 1
        assert document.page_text(1) == ["copy me"]
        assert len(document.annotations()) == 2
        assert document.annotations()[0]["id"] != document.annotations()[1]["id"]
        assert "copied mark" in document.page_svg(1)
        assert document.pages()[0]["id"] != document.pages()[1]["id"]
        assert document.validate_references() == []
        with pytest.raises(IndexError):
            document.duplicate_page(-1)
        with pytest.raises(OfdError, match="out of bounds"):
            document.duplicate_page(2)
        document.save(output)
    with Document.open(output) as reopened:
        assert reopened.page_text(1) == ["copy me"]
        assert len(reopened.annotations()) == 2


def test_duplicates_page_at_body_position():
    with Document.new() as document:
        document.add_text_page("A", 40, 30)
        document.add_text_page("B", 40, 30)
        document.add_text_watermark(1, "mark", (1, 1, 12, 6), 4)
        assert document.duplicate_page_at(1, 0) == 0
        assert [document.page_text(i) for i in range(3)] == [["B"], ["A"], ["B"]]
        assert len(document.annotations()) == 2
        assert document.pages()[0]["id"] != document.pages()[2]["id"]
        assert document.validate_references() == []
        before = document.to_bytes()
        with pytest.raises(IndexError):
            document.duplicate_page_at(-1, 0)
        with pytest.raises(OfdError, match="out of bounds"):
            document.duplicate_page_at(0, 4)
        assert document.to_bytes() == before


def test_removes_page_and_keeps_later_append_order(tmp_path):
    with Document.new() as document:
        for text in ("A", "B", "C"):
            document.add_text_page(text, 20, 20)
        document.remove_page(1)
        document.add_text_page("D", 20, 20)
        assert [document.page_text(i) for i in range(3)] == [["A"], ["C"], ["D"]]
        with pytest.raises(IndexError):
            document.remove_page(-1)
        with pytest.raises(OfdError, match="out of bounds"):
            document.remove_page(3)
        output = tmp_path / "removed.ofd"
        document.save(output)
    with Document.open(output) as reopened:
        assert [reopened.page_text(i) for i in range(3)] == [["A"], ["C"], ["D"]]


def test_validates_references_and_reports_broken_font():
    with Document.open(FIXTURES / "helloworld.ofd") as fixture:
        assert fixture.validate_references() == []
    with Document.new() as document:
        document.add_text_page("check")
        assert document.validate_references() == []
        page_path = document.pages()[0]["path"]
        page = document.entry(page_path)
        assert b'Font="1"' in page
        document.set_entry(page_path, page.replace(b'Font="1"', b'Font="999"'))
        issues = document.validate_references()
        assert any(issue["code"] == "missing_font" and issue["path"] == page_path for issue in issues)


def test_validates_navigation_destination_page():
    with Document.new() as document:
        document.add_text_page("page")
        path = "Doc_0/Document.xml"
        original = document.entry(path)
        document.set_entry(
            path,
            original.replace(
                b"</ofd:Document>",
                b'<ofd:Bookmarks><ofd:Bookmark Name="broken"><ofd:Dest Type="Fit" PageID="999"/></ofd:Bookmark></ofd:Bookmarks></ofd:Document>',
            ),
        )
        assert any(
            issue["code"] == "navigation_page_reference"
            for issue in document.validate_references()
        )


def test_validates_page_action_destination_page():
    with Document.new() as document:
        document.add_text_page("source")
        document.add_text_page("target")
        pages = document.pages()
        document.add_page_object_goto_action(
            0, document.page_text_objects(0)[0]["id"], pages[1]["id"]
        )
        assert document.validate_references() == []
        path = pages[0]["path"]
        original = document.entry(path)
        needle = f'PageID="{pages[1]["id"]}"'.encode()
        assert needle in original
        document.set_entry(path, original.replace(needle, b'PageID="999"'))
        assert any(
            issue["code"] == "navigation_page_reference" and issue["path"] == path
            for issue in document.validate_references()
        )


def test_validates_closed_document():
    document = Document.new()
    document.close()
    with pytest.raises(OfdError, match="closed"):
        document.validate_references()


def test_create_save_reopen(tmp_path):
    path = tmp_path / "created.ofd"
    with Document.new() as document:
        document.add_text_page("Python & Rust")
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["Python & Rust"]


def test_ofd_bytes_round_trip_and_invalid_input():
    with Document.new() as document:
        document.add_text_page("内存 OFD", 40, 30)
        source = document.to_bytes()
        assert source.startswith(b"PK")
    with Document.from_bytes(source) as reopened:
        assert reopened.page_text(0) == ["内存 OFD"]
        assert reopened.validate_references() == []
    with pytest.raises(OfdError):
        Document.from_bytes(b"")
    with pytest.raises(OfdError):
        Document.from_bytes(b"not a ZIP")
    with pytest.raises(OfdError, match="closed"):
        reopened.to_bytes()


def test_closed_document_reports_error():
    document = Document.new()
    document.close()
    with pytest.raises(OfdError, match="closed"):
        _ = document.page_count


def test_entry_edit_and_error_paths(tmp_path):
    path = tmp_path / "edited.ofd"
    with Document.new() as document:
        document.set_entry("Doc_0/data.bin", b"\x00\x01")
        document.set_entry("scratch.bin", b"remove")
        assert document.remove_entry("scratch.bin") is True
        assert document.remove_entry("scratch.bin") is False
        with pytest.raises(OfdError):
            document.remove_entry("../outside")
        assert document.entry("Doc_0/data.bin") == b"\x00\x01"
        with pytest.raises(OfdError, match="entry not found"):
            document.entry("missing.bin")
        with pytest.raises(OfdError, match="invalid XML"):
            document.set_entry("Doc_0/bad.xml", b"<broken>")
        with pytest.raises(OfdError, match="out of bounds"):
            document.page_text(0)
        with pytest.raises(IndexError):
            document.page_text(-1)
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.entry("Doc_0/data.bin") == b"\x00\x01"
        assert "scratch.bin" not in reopened.entries()


def test_open_missing_file_reports_error(tmp_path):
    with pytest.raises(OfdError, match="I/O error"):
        Document.open(tmp_path / "missing.ofd")


def test_attachment_reader():
    with Document.open(FIXTURES / "AddAttachment.ofd") as document:
        items = document.attachments()
        assert len(items) == 1
        assert items[0]["name"] == "Gao"
        assert items[0]["creation_date"] == "2020-05-20"
        assert items[0]["visible"] is True
        assert items[0]["usage"] == "none"
        assert len(document.attachment_data(0)) == 36_494
        with pytest.raises(OfdError, match="out of bounds"):
            document.attachment_data(1)


def test_reports_missing_signed_resource_across_binding():
    fixture = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
               "tests" / "fixtures" / "missing-signed-resource.ofd")
    with Document.open(fixture) as document:
        checks = document.check_signature_references(0)
        missing = [check for check in checks if check["path"] == "Doc_0/PublicRes.xml"]
        assert len(missing) == 1
        assert missing[0]["valid"] is False
        assert missing[0]["actual"] == ""
        assert all(check["valid"] for check in checks if check not in missing)
        issues = document.validate_references()
        assert any(issue["code"] == "signature_reference" and
                   "Doc_0/PublicRes.xml" in issue["message"] for issue in issues)


def test_reads_signature_description_and_bytes():
    with Document.open(FIXTURES / "SESV4SignDoc.ofd") as document:
        signatures = document.signatures()
        assert len(signatures) == 1
        assert signatures[0]["check_method"] == "SM3"
        assert signatures[0]["references"][0]["path"] == "OFD.xml"
        assert signatures[0]["stamps"][0]["boundary"] == [50, 50, 40, 40]
        assert len(document.signed_value_data(0)) == 17_726
        assert len(document.seal_data(0)) == 17_011
        checks = document.check_signature_references(0)
        assert len(checks) == 5
        assert all(check["valid"] for check in checks)
        description = document.check_signature_description_hash(0)
        assert description["valid"]
        assert description["expected"] == description["actual"]
        assert document.check_signature_seal_match(0) is True
        assert document.check_ses_seal_signature(0)
        assert document.check_ses_seal_certificate_validity(0, 1_600_000_000)
        assert not document.check_ses_seal_certificate_validity(0, 0)
        assert document.check_ses_seal_signer_authorization(0)
        assert document.check_ses_seal_validity(0, 1_600_000_000)
        assert not document.check_ses_seal_validity(0, 1_800_000_000)
        assert not document.check_ses_seal_validity(0, 0)
        root_cert = (FIXTURES.parents[3] / "ofdrw-rs/crates/ofdrw/tests/fixtures/root-cert.der").read_bytes()
        assert not document.check_ses_seal_issuer_ordered_sm2_chain(
            0, [], root_cert, 1_600_000_000
        )
        trust_report = document.verify_signature_with_ordered_sm2_chains(
            0, b"", [], root_cert, [], root_cert, 1_600_000_000
        )
        assert trust_report["seal_issuer_chain_valid"] is False
        assert trust_report["signature"]["seal_signer_authorized"] is True
        assert trust_report["signature"]["seal_validity_valid"] is True
        assert not trust_report["all_valid"]
        assert not document.check_ses_seal_signature(0, "different-id")
        assert document.check_ses_signature_value(0)
        seal_path = signatures[0]["seal_path"]
        seal = bytearray(document.seal_data(0))
        seal[12] ^= 1
        document.set_entry(seal_path, bytes(seal))
        assert document.check_signature_seal_match(0) is False
        with pytest.raises(OfdError, match="out of bounds"):
            document.check_ses_signature_value(1)
        with pytest.raises(IndexError):
            document.signed_value_data(-1)
        with pytest.raises(IndexError):
            document.seal_data(-1)
        with pytest.raises(IndexError):
            document.check_signature_references(-1)
        with pytest.raises(IndexError):
            document.check_signature_seal_match(-1)
        with pytest.raises(IndexError):
            document.check_ses_seal_signature(-1)
        with pytest.raises(ValueError):
            document.check_ses_seal_certificate_validity(0, -1)
        with pytest.raises(ValueError):
            document.check_ses_seal_validity(0, -1)
        with pytest.raises(ValueError):
            document.check_ses_seal_issuer_ordered_sm2_chain(0, [], root_cert, -1)
        with pytest.raises(OfdError, match="out of bounds"):
            document.signed_value_data(1)


def test_signature_reference_accepts_xml_base64_whitespace():
    with Document.open(FIXTURES / "SESV4SignDoc.ofd") as document:
        signature = document.signatures()[0]
        value = signature["references"][0]["check_value"]
        wrapped = value[:20] + "\n\t" + value[20:]
        path = signature["path"]
        source = document.entry(path)
        assert value.encode() in source
        document.set_entry(path, source.replace(value.encode(), wrapped.encode(), 1))
        checks = document.check_signature_references(0)
        assert all(check["valid"] for check in checks)
        assert checks[0]["expected"] == wrapped


def test_checks_raw_digital_signature_with_certificate():
    root = FIXTURES.parents[3]
    certificate = (root / "ofdrw-rs/crates/ofdrw/tests/fixtures/user-cert.der").read_bytes()
    with Document.open(root / "ofdrw-sign/src/test/resources/DigitalSign.ofd") as document:
        assert document.check_digital_signature_with_certificate(0, certificate)
        with pytest.raises(OfdError, match="certificate"):
            document.check_digital_signature_with_certificate(0, b"invalid")
        with pytest.raises(IndexError):
            document.check_digital_signature_with_certificate(-1, certificate)


def test_checks_signer_certificate_validity_at_supplied_time():
    root = FIXTURES.parents[3]
    certificate = (root / "ofdrw-rs/crates/ofdrw/tests/fixtures/user-cert.der").read_bytes()
    with Document.open(root / "ofdrw-sign/src/test/resources/DigitalSign.ofd") as document:
        assert document.check_signature_certificate_validity(0, 1_600_000_000, certificate)
        assert not document.check_signature_certificate_validity(0, 1_700_000_000, certificate)
        with pytest.raises(OfdError, match="requires a signer certificate"):
            document.check_signature_certificate_validity(0, 1_600_000_000)
        with pytest.raises(ValueError):
            document.check_signature_certificate_validity(0, -1, certificate)
        with pytest.raises(IndexError):
            document.check_signature_certificate_validity(-1, 1_600_000_000, certificate)


def test_creates_raw_sm2_signature_and_verifies_it(tmp_path):
    root = FIXTURES.parents[3]
    key = (root / "ofdrw-rs/crates/ofdrw/tests/fixtures/user-private-key.bin").read_bytes()
    cert = (root / "ofdrw-rs/crates/ofdrw/tests/fixtures/user-cert.der").read_bytes()
    output = tmp_path / "signed.ofd"
    with Document.new() as document:
        document.add_text_page("signed")
        assert document.add_digital_signature(key, "2020-04-22 16:22:44") == 0
        assert document.check_digital_signature_with_certificate(0, cert)
        assert all(item["valid"] for item in document.check_signature_references(0))
        assert document.add_digital_signature(key, "2020-04-23 10:11:12") == 1
        assert document.check_digital_signature_with_certificate(0, cert)
        assert document.check_digital_signature_with_certificate(1, cert)
        assert all(item["valid"] for item in document.check_signature_references(1))
        with pytest.raises(ValueError, match="NUL"):
            document.add_digital_signature(key, "2020-04-22 16:22:44\x00ignored")
        document.save(output)
    with Document.open(output) as reopened:
        for index in range(2):
            assert reopened.check_digital_signature_with_certificate(index, cert)
            assert all(item["valid"] for item in reopened.check_signature_references(index))


def test_creates_ses_signature_and_appends_raw_signature(tmp_path):
    root = FIXTURES.parents[3]
    fixtures = root / "ofdrw-rs/crates/ofdrw/tests/fixtures"
    key = (fixtures / "user-private-key.bin").read_bytes()
    certificate = (fixtures / "user-cert.der").read_bytes()
    seal = (root / "ofdrw-gm/src/test/resources/UserV4.esl").read_bytes()
    output = tmp_path / "ses.ofd"
    with Document.new() as document:
        document.add_text_page("signed")
        with pytest.raises(ValueError, match="together"):
            document.add_ses_signature(key, certificate, seal, "2020-09-13 12:26:40", page_index=0)
        assert document.add_ses_signature(
            key, certificate, seal, "2020-09-13 12:26:40",
            page_index=0, boundary=(10, 20, 40, 40),
        ) == 0
        assert document.signatures()[0]["stamps"][0]["boundary"] == [10, 20, 40, 40]
        assert 'data-ofd-stamp="s002"' in document.page_svg(0)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        assert document.check_ses_signature_value(0)
        assert document.check_signature_seal_match(0)
        assert document.check_ses_seal_signer_authorization(0)
        assert document.check_signature_description_hash(0)["valid"]
        assert document.add_digital_signature(key, "2020-09-13 12:26:40") == 1
        document.save(output)
    with Document.open(output) as reopened:
        assert reopened.check_ses_signature_value(0)
        assert all(check["valid"] for check in reopened.check_signature_references(0))
        assert reopened.check_digital_signature_with_certificate(1, certificate)


def test_creates_ses_seal_and_signs_document():
    root = FIXTURES.parents[3]
    fixtures = root / "ofdrw-rs/crates/ofdrw/tests/fixtures"
    key = (fixtures / "user-private-key.bin").read_bytes()
    certificate = (fixtures / "user-cert.der").read_bytes()
    picture = (root / "ofdrw-layout/src/test/resources/testimg.png").read_bytes()
    options = dict(
        version=5, vendor_id="ofdrw-rs", seal_id="python-seal", name="Python 印章",
        seal_type=3, authorized_signer_certificates=[certificate],
        created_at=1_600_000_000, valid_from=1_600_000_000,
        valid_until=1_900_000_000, picture_type="PNG",
        picture_width_mm=40, picture_height_mm=40,
    )
    with pytest.raises(OfdError, match="does not match"):
        create_ses_seal((fixtures / "trust-leaf-private-key.bin").read_bytes(),
                        certificate, picture, **options)
    seal = create_ses_seal(key, certificate, picture, **options)
    assert seal.startswith(b"\x30")
    with Document.new() as document:
        document.add_text_page("signed")
        assert document.add_ses_signature(key, certificate, seal, "2020-09-13 12:26:40") == 0
        assert document.check_ses_seal_signature(0)
        assert document.check_ses_seal_signer_authorization(0)
        assert document.check_ses_signature_value(0)


def test_verifies_ses_signer_and_seal_issuer_certificate_pools():
    root = FIXTURES.parents[3]
    fixtures = root / "ofdrw-rs/crates/ofdrw/tests/fixtures"
    key = (fixtures / "chain-leaf-private-key.bin").read_bytes()
    certificate = (fixtures / "chain-leaf-cert.der").read_bytes()
    intermediate = (fixtures / "chain-intermediate-cert.der").read_bytes()
    anchor = (fixtures / "chain-root-cert.der").read_bytes()
    unrelated = (fixtures / "root-cert.der").read_bytes()
    picture = (root / "ofdrw-layout/src/test/resources/testimg.png").read_bytes()
    seal = create_ses_seal(
        key, certificate, picture, version=4, vendor_id="ofdrw-rs",
        seal_id="python-chain-seal", name="Python chain seal", seal_type=3,
        authorized_signer_certificates=[certificate], created_at=1_893_456_000,
        valid_from=1_893_456_000, valid_until=1_950_000_000,
        picture_type="PNG", picture_width_mm=20, picture_height_mm=20,
    )
    with Document.new() as document:
        document.add_text_page("trusted SES")
        document.add_ses_signature(key, certificate, seal, "2030-01-01 00:00:00")
        report = document.verify_signature_with_certificate_pools(
            0, b"", [unrelated, intermediate], anchor,
            [unrelated, intermediate], anchor, 1_893_456_000,
        )
        assert report["all_valid"]
        assert report["seal_issuer_chain_valid"] is True
        report = document.verify_signature_with_certificate_pools(
            0, b"", [intermediate], anchor, [unrelated], anchor, 1_893_456_000,
        )
        assert report["signature"]["all_valid"]
        assert report["seal_issuer_chain_valid"] is False
        assert not report["all_valid"]


def test_riding_stamp_clip_is_exposed_and_rendered():
    sample = FIXTURES.parents[3] / "ofdrw-converter/src/test/resources/V4RideRight.ofd"
    with Document.open(sample) as document:
        stamps = document.signatures()[0]["stamps"]
        assert len(stamps) == 5
        assert stamps[0]["clip"] == [0, 0, 8, 40]
        assert stamps[1]["clip"] == [8, 0, 8, 40]
        assert 'clip-path="url(#ofd-stamp-clip-' in document.page_svg(0)


def test_checks_direct_sm2_trust_anchor():
    root = FIXTURES.parents[3] / "ofdrw-rs/crates/ofdrw/tests/fixtures"
    key = (root / "trust-leaf-private-key.bin").read_bytes()
    signer = (root / "trust-leaf-cert.der").read_bytes()
    anchor = (root / "trust-root-cert.der").read_bytes()
    with Document.new() as document:
        document.add_digital_signature(key, "2030-01-01 00:00:00")
        assert document.check_signature_direct_trust_anchor(0, signer, anchor, 1_893_456_000)
        assert not document.check_signature_direct_trust_anchor(
            0, signer, anchor, 1_893_456_000, "1234567812345678"
        )
        with pytest.raises(OfdError, match="trust anchor certificate"):
            document.check_signature_direct_trust_anchor(0, signer, b"bad", 1_893_456_000)
        with pytest.raises(ValueError):
            document.check_signature_direct_trust_anchor(0, signer, anchor, -1)
        with pytest.raises(ValueError, match="NUL"):
            document.check_signature_direct_trust_anchor(0, signer, anchor, 1_893_456_000, "bad\x00id")
        with pytest.raises(IndexError):
            document.check_signature_direct_trust_anchor(-1, signer, anchor, 1_893_456_000)


def test_reports_complete_signature_verification():
    root = FIXTURES.parents[3] / "ofdrw-rs/crates/ofdrw/tests/fixtures"
    key = (root / "trust-leaf-private-key.bin").read_bytes()
    signer = (root / "trust-leaf-cert.der").read_bytes()
    anchor = (root / "trust-root-cert.der").read_bytes()
    with Document.new() as document:
        document.add_text_page("protected")
        document.add_digital_signature(key, "2030-01-01 00:00:00")
        report = document.verify_signature_with_direct_trust_anchor(
            0, signer, anchor, 1_893_456_000
        )
        assert report["all_valid"]
        assert report["references_valid"]
        assert report["description_hash"] is None
        path = "Doc_0/Pages/Page_0/Content.xml"
        document.set_entry(path, document.entry(path).replace(b"protected", b"changed"))
        report = document.verify_signature_with_direct_trust_anchor(
            0, signer, anchor, 1_893_456_000
        )
        assert not report["all_valid"]
        assert not report["references_valid"]
        assert report["signature_value_valid"]
        assert report["direct_trust_anchor_valid"]
        with pytest.raises(OfdError, match="trust anchor certificate"):
            document.verify_signature_with_direct_trust_anchor(0, signer, b"bad", 1_893_456_000)
        with pytest.raises(ValueError):
            document.verify_signature_with_direct_trust_anchor(0, signer, anchor, -1)
        with pytest.raises(ValueError, match="NUL"):
            document.verify_signature_with_direct_trust_anchor(0, signer, anchor, 1_893_456_000, "bad\x00id")
        with pytest.raises(IndexError):
            document.verify_signature_with_direct_trust_anchor(-1, signer, anchor, 1_893_456_000)


def test_reports_ordered_sm2_chain_verification():
    root = FIXTURES.parents[3] / "ofdrw-rs/crates/ofdrw/tests/fixtures"
    key = (root / "chain-leaf-private-key.bin").read_bytes()
    signer = (root / "chain-leaf-cert.der").read_bytes()
    intermediate = (root / "chain-intermediate-cert.der").read_bytes()
    anchor = (root / "chain-root-cert.der").read_bytes()
    with Document.new() as document:
        document.add_text_page("chained")
        document.add_digital_signature(key, "2030-01-01 00:00:00")
        report = document.verify_signature_with_ordered_sm2_chain(
            0, signer, [intermediate], anchor, 1_893_456_000
        )
        assert report["all_valid"]
        assert report["certificate_chain_valid"]
        assert report["references_valid"]
        unrelated = (root / "root-cert.der").read_bytes()
        pooled = document.verify_signature_with_certificate_pool(
            0, signer, [unrelated, intermediate], anchor, 1_893_456_000
        )
        assert pooled["all_valid"]
        missing = document.verify_signature_with_certificate_pool(
            0, signer, [unrelated], anchor, 1_893_456_000
        )
        assert missing["signature_value_valid"]
        assert not missing["certificate_chain_valid"]
        trust_report = document.verify_signature_with_ordered_sm2_chains(
            0, signer, [intermediate], anchor, [], b"", 1_893_456_000
        )
        assert trust_report["all_valid"]
        assert trust_report["seal_issuer_chain_valid"] is None
        assert trust_report["signature"]["all_valid"]
        pooled_trust = document.verify_signature_with_certificate_pools(
            0, signer, [unrelated, intermediate], anchor, [], b"", 1_893_456_000
        )
        assert pooled_trust["all_valid"]
        assert pooled_trust["seal_issuer_chain_valid"] is None
        no_intermediate = document.verify_signature_with_ordered_sm2_chain(
            0, signer, [], anchor, 1_893_456_000
        )
        assert not no_intermediate["certificate_chain_valid"]
        assert not no_intermediate["all_valid"]
        path = "Doc_0/Pages/Page_0/Content.xml"
        document.set_entry(path, document.entry(path).replace(b"chained", b"changed"))
        report = document.verify_signature_with_ordered_sm2_chain(
            0, signer, [intermediate], anchor, 1_893_456_000
        )
        assert not report["all_valid"]
        assert not report["references_valid"]
        assert report["certificate_chain_valid"]
        with pytest.raises(OfdError, match="intermediate certificate"):
            document.verify_signature_with_ordered_sm2_chain(0, signer, [b"bad"], anchor, 1_893_456_000)
        with pytest.raises(ValueError):
            document.verify_signature_with_ordered_sm2_chain(0, signer, [intermediate], anchor, -1)
        with pytest.raises(ValueError, match="NUL"):
            document.verify_signature_with_ordered_sm2_chain(
                0, signer, [intermediate], anchor, 1_893_456_000, "bad\x00id"
            )
        with pytest.raises(IndexError):
            document.verify_signature_with_ordered_sm2_chain(-1, signer, [intermediate], anchor, 1_893_456_000)


def test_custom_sm2_signature_distinguishing_id():
    root = FIXTURES.parents[3] / "ofdrw-rs/crates/ofdrw/tests/fixtures"
    key = (root / "chain-leaf-private-key.bin").read_bytes()
    signer = (root / "chain-leaf-cert.der").read_bytes()
    intermediate = (root / "chain-intermediate-cert.der").read_bytes()
    anchor = (root / "chain-root-cert.der").read_bytes()
    custom_id = "OFD-Rust-Interop"
    with Document.new() as document:
        document.add_text_page("custom ID")
        assert document.add_digital_signature(key, "2030-01-01 00:00:00", custom_id) == 0
        assert not document.check_digital_signature_with_certificate(0, signer)
        assert document.check_digital_signature_with_certificate(0, signer, custom_id)
        report = document.verify_signature_with_ordered_sm2_chain(
            0, signer, [intermediate], anchor, 1_893_456_000, "", custom_id
        )
        assert report["all_valid"]
        assert not document.verify_signature_with_ordered_sm2_chain(
            0, signer, [intermediate], anchor, 1_893_456_000
        )["all_valid"]
        with pytest.raises(ValueError, match="NUL"):
            document.check_digital_signature_with_certificate(0, signer, "bad\x00id")
        with pytest.raises(ValueError, match="NUL"):
            document.add_digital_signature(key, "2030-01-01 00:00:00", "bad\x00id")
    with Document.open(FIXTURES / "SESV4SignDoc.ofd") as seal:
        assert seal.check_ses_signature_value(0)
        assert not seal.check_ses_signature_value(0, custom_id)


def test_reads_bookmarks_and_outlines():
    root = FIXTURES.parents[3]
    with Document.open(root / "ofdrw-layout/src/test/resources/Page5.ofd") as document:
        bookmarks = document.bookmarks()
        assert len(bookmarks) == 1
        assert bookmarks[0]["name"] == "新建书签"
        assert bookmarks[0]["destination"]["right"] == 167.873
        assert document.outlines() == []
    with Document.open(root / "ofdrw-converter/src/test/resources/z.ofd") as document:
        outlines = document.outlines()
        assert len(outlines) == 5
        assert outlines[0]["title"] == "11.4字型变换 "
        assert outlines[0]["destination"]["page_id"] == "10090"
        assert document.bookmarks() == []


def test_adds_nested_outlines(tmp_path):
    path = tmp_path / "outlines.ofd"
    with Document.new() as document:
        document.add_text_page("first")
        page_id = document.pages()[0]["id"]
        root = document.add_outline("Root & Python", destination={"page_id": page_id, "kind": "XYZ", "top": 10})
        other = document.add_outline("Other")
        child = document.add_outline("Child", parent_index=root, expanded=False)
        assert (root, other, child) == (0, 1, 1)
        assert [item["title"] for item in document.outlines()] == ["Root & Python", "Child", "Other"]
        document.set_outline_title(root, "章节 & Python")
        document.set_outline_expanded(root, False)
        assert document.outlines()[0]["title"] == "章节 & Python"
        assert document.outlines()[0]["expanded"] is False
        with pytest.raises(TypeError):
            document.set_outline_expanded(root, 1)
        with pytest.raises(IndexError):
            document.add_outline("bad", parent_index=-1)
        with pytest.raises(OfdError, match="out of bounds"):
            document.add_outline("bad", parent_index=9)
        with pytest.raises(ValueError):
            document.add_outline("bad", destination={"page_id": page_id, "kind": "XYZ", "top": float("nan")})
        document.add_text_page("second")
        second_page = document.pages()[1]["id"]
        document.set_outline_destination(root, {"page_id": second_page, "kind": "Fit"})
        assert document.outlines()[0]["destination"]["page_id"] == second_page
        document.add_bookmark("索引 & Python", second_page, kind="Fit")
        document.set_outline_bookmark(root, "索引 & Python")
        assert document.outlines()[0]["bookmark_name"] == "索引 & Python"
        with pytest.raises(OfdError):
            document.set_outline_bookmark(root, "missing")
        document.set_outline_destination(root, {"page_id": second_page, "kind": "Fit"})
        with pytest.raises(OfdError):
            document.set_outline_destination(root, {"page_id": "missing", "kind": "Fit"})
        document.save(path)
    with Document.open(path) as document:
        outlines = document.outlines()
        assert [item["title"] for item in outlines] == ["章节 & Python", "Child", "Other"]
        assert outlines[1]["parent_index"] == 0
        assert outlines[0]["destination"]["page_id"] == document.pages()[1]["id"]
        assert document.page_text(1) == ["second"]
    path = tmp_path / "existing_outlines.ofd"
    with Document.open(FIXTURES / "helloworld.ofd") as document:
        document.add_outline("Existing", destination={"page_id": document.pages()[0]["id"], "kind": "Fit"})
        document.save(path)
    with Document.open(path) as document:
        assert document.outlines()[0]["title"] == "Existing"


def test_adds_bookmark_and_preserves_it_after_more_pages(tmp_path):
    path = tmp_path / "bookmark.ofd"
    with Document.new() as document:
        document.add_text_page("first")
        page_id = document.pages()[0]["id"]
        document.add_bookmark("Start & <here>", page_id, left=12.5, top=25)
        with pytest.raises(OfdError, match="already exists"):
            document.add_bookmark("Start & <here>", page_id)
        with pytest.raises(IndexError):
            document.add_bookmark("bad", page_id, document_index=-1)
        document.add_text_page("second")
        document.save(path)
    with Document.open(path) as document:
        bookmarks = document.bookmarks()
        assert len(bookmarks) == 1
        assert bookmarks[0]["name"] == "Start & <here>"
        assert bookmarks[0]["destination"]["left"] == 12.5
        assert document.page_count == 2


def test_removes_bookmark_and_outline_subtree_across_binding(tmp_path):
    path = tmp_path / "navigation.ofd"
    with Document.new() as document:
        document.add_text_page("page")
        page_id = document.pages()[0]["id"]
        document.add_bookmark("Keep", page_id)
        document.add_bookmark("Delete", page_id)
        object_id = document.page_objects(0)[0]["id"]
        document.add_page_object_bookmark_action(0, object_id, "Delete")
        document.add_outline("Parent")
        document.add_outline("Child", parent_index=0)
        document.add_outline("Sibling")
        with pytest.raises(OfdError, match="not found"):
            document.remove_bookmark("Missing")
        with pytest.raises(OfdError, match="out of bounds"):
            document.remove_outline(3)
        with pytest.raises(IndexError):
            document.remove_outline(-1)
        with pytest.raises(OfdError, match="still referenced"):
            document.remove_bookmark("Delete")
        document.remove_page_object_action(0, object_id, 0)
        document.remove_bookmark("Delete")
        document.remove_outline(0)
        document.add_text_page("next")
        document.save(path)
    with Document.open(path) as document:
        assert [item["name"] for item in document.bookmarks()] == ["Keep"]
        assert [item["title"] for item in document.outlines()] == ["Sibling"]
        assert document.page_text(1) == ["next"]


def test_page_area_boxes_and_application_clip():
    with Document.new() as document:
        document.add_text_page("clipped", 100, 100)
        path = document.pages()[0]["path"]
        xml = document.entry(path).replace(
            b"</ofd:PhysicalBox>",
            b"</ofd:PhysicalBox><ofd:ApplicationBox>25 25 50 50</ofd:ApplicationBox>"
            b"<ofd:ContentBox>30 30 40 40</ofd:ContentBox>"
            b"<ofd:BleedBox>-5 -5 110 110</ofd:BleedBox>",
        )
        document.set_entry(path, xml)
        page = document.pages()[0]
        assert page["application_box"] == [25, 25, 50, 50]
        assert page["content_box"] == [30, 30, 40, 40]
        assert page["bleed_box"] == [-5, -5, 110, 110]
        assert 'clip-path="url(#ofd-application-clip)"' in document.page_svg(0)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        assert document.to_pdf().startswith(b"%PDF")


def test_set_page_area_preserves_page_content_and_round_trips(tmp_path):
    output = tmp_path / "page-area.ofd"
    with Document.new() as document:
        document.add_text_page("area", 100, 100)
        path = document.pages()[0]["path"]
        original = document.entry(path)
        document.set_page_area(
            0,
            (0, 0, 100, 100),
            application_box=(20, 20, 60, 60),
            content_box=(25, 25, 50, 50),
        )
        assert document.pages()[0]["application_box"] == [20, 20, 60, 60]
        assert document.page_text(0) == ["area"]
        assert b"<ofd:TextObject" in document.entry(path)
        assert 'ofd-application-clip' in document.page_svg(0)
        assert document.entry(path) != original
        with pytest.raises(IndexError):
            document.set_page_area(-1, (0, 0, 100, 100))
        with pytest.raises(OfdError, match="invalid dimensions"):
            document.set_page_area(0, (0, 0, 0, 100))
        with pytest.raises(ValueError):
            document.set_page_area(0, (0, 0, float("nan"), 100))
        document.save(output)
    with Document.open(output) as reopened:
        assert reopened.pages()[0]["content_box"] == [25, 25, 50, 50]
        reopened.set_page_area(0, (0, 0, 100, 100))
        assert reopened.pages()[0]["application_box"] is None


def test_document_page_area_round_trips(tmp_path):
    output = tmp_path / "document-page-area.ofd"
    with Document.new() as document:
        assert document.document_page_area(0)["physical_box"] == [0, 0, 210, 297]
        document.set_document_page_area(
            0, (0, 0, 150, 60), application_box=(10, 5, 130, 50)
        )
        area = document.document_page_area(0)
        assert area["physical_box"] == [0, 0, 150, 60]
        assert area["application_box"] == [10, 5, 130, 50]
        with pytest.raises(IndexError):
            document.set_document_page_area(-1, (0, 0, 150, 60))
        with pytest.raises(OfdError, match="invalid dimensions"):
            document.set_document_page_area(0, (0, 0, -1, 60))
        document.save(output)
    with Document.open(output) as reopened:
        assert reopened.document_page_area(0) == area


def test_exports_svg_preview():
    with Document.new() as document:
        document.add_text_page("SVG & Python")
        svg = document.page_svg(0)
        root = ElementTree.fromstring(svg)
        assert root.tag == "{http://www.w3.org/2000/svg}svg"
        assert root.find(".//{http://www.w3.org/2000/svg}text").text == "SVG & Python"
        with pytest.raises(IndexError):
            document.page_svg(-1)
        with pytest.raises(OfdError, match="out of bounds"):
            document.page_svg(1)


def test_svg_preview_respects_java_text_deltas():
    with Document.open(FIXTURES / "发票示例.ofd") as document:
        root = ElementTree.fromstring(document.page_svg(0))
        number = next(
            node for node in root.iter("{http://www.w3.org/2000/svg}text")
            if node.text == "24112000000048542163"
        )
        assert len(number.attrib["x"].split()) == len(number.text)


def test_java_clipped_template_renders_through_native_binding():
    with Document.open(FIXTURES / "path_unstd.ofd") as document:
        svg = ElementTree.fromstring(document.page_svg(0))
        clips = svg.findall(".//{http://www.w3.org/2000/svg}clipPath")
        assert clips
        assert any(clip.find("{http://www.w3.org/2000/svg}path") is not None for clip in clips)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG\r\n\x1a\n")


def test_java_radial_gradient_and_clip_render_through_native_binding():
    path = FIXTURES.parents[3] / "ofdrw-converter" / "src" / "test" / "resources" / "testPathClip.ofd"
    with Document.open(path) as document:
        svg = document.page_svg(0)
        assert "<radialGradient " in svg
        assert "<clipPath " in svg
        assert document.page_png(0, 25.4).startswith(b"\x89PNG\r\n\x1a\n")


def test_java_pattern_watermark_renders_through_native_binding():
    sample = (
        Path(__file__).resolve().parents[4]
        / "ofdrw-converter"
        / "src"
        / "test"
        / "resources"
        / "pattern类型.ofd"
    )
    with Document.open(sample) as document:
        assert "<pattern id=\"ofd-pattern-" in document.page_svg(0)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        assert document.to_pdf().startswith(b"%PDF")


def test_java_jbig2_image_renders_and_can_be_written():
    sample = (
        Path(__file__).resolve().parents[4]
        / "ofdrw-layout"
        / "src"
        / "test"
        / "resources"
        / "keyword2.ofd"
    )
    with Document.open(sample) as source:
        data = source.page_image_data(0, 0)
        assert source.page_images(0)[0]["format"] == "GBIG2"
        assert "data:image/png;base64," in source.page_svg(0)
    with Document.new() as document:
        document.add_image_page(data, "jb2", 20, 20)
        assert document.page_images(0)[0]["format"] == "GBIG2"
        assert document.page_image_data(0, 0) == data
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")


def test_java_text_weight_is_preserved_in_preview():
    sample = (
        Path(__file__).resolve().parents[4]
        / "ofdrw-layout"
        / "src"
        / "test"
        / "resources"
        / "SplitParagraph.ofd"
    )
    with Document.open(sample) as document:
        assert 'font-weight="800"' in document.page_svg(0)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")


def test_java_cgtransform_uses_embedded_font_glyphs():
    sample = (
        Path(__file__).resolve().parents[4]
        / "ofdrw-layout"
        / "src"
        / "test"
        / "resources"
        / "ano.ofd"
    )
    with Document.open(sample) as document:
        svg = document.page_svg(0)
        assert 'data-ofd-glyph="2591"' in svg
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        assert document.to_pdf().startswith(b"%PDF")


def test_invisible_path_is_omitted_from_native_preview():
    with Document.new() as document:
        document.add_path_page(
            "M 0 0 L 10 0 L 10 10 L 0 10 C", (0, 0, 10, 10), 20, 20, True, False
        )
        path = document.pages()[0]["path"]
        xml = document.entry(path).replace(
            b"<ofd:PathObject", b'<ofd:PathObject Visible="false"'
        )
        document.set_entry(path, xml)
        assert "<path d=" not in document.page_svg(0)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")


def test_hidden_text_is_absent_from_extraction_and_preview():
    with Document.new() as document:
        document.add_text_page("shown", 80, 50)
        object_id = document.add_text_object(0, "hidden", (10, 20, 40, 10), 5)
        path = document.pages()[0]["path"]
        marker = f'<ofd:TextObject ID="{object_id}"'.encode()
        xml = document.entry(path)
        assert marker in xml
        document.set_entry(path, xml.replace(marker, marker + b' Visible="false"'))
        assert document.page_text(0) == ["shown"]
        assert document.page_text_with_templates(0) == ["shown"]
        assert document.page_text_in_region(0, 0, 0, 80, 50) == ["shown"]
        assert document.keyword_positions(["hidden"]) == []
        assert "hidden" not in document.page_svg(0)


def test_java_invoice_annotation_blend_mode():
    with Document.open(FIXTURES / "发票示例.ofd") as document:
        assert 'style="mix-blend-mode:darken"' in document.page_svg(0)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")


def test_java_composite_graphic_renders_through_native_binding():
    path = FIXTURES.parents[3] / "ofdrw-converter" / "src" / "test" / "resources" / "y.ofd"
    with Document.open(path) as document:
        assert "30" in document.page_text(0)
        assert "30" in document.page_text_with_templates(0)
        svg = document.page_svg(0)
        assert "ofd-composite-" in svg
        assert ">30</text>" in svg


def test_converter_samples_render_with_optional_draw_parameters_and_glyph_counts():
    samples = FIXTURES.parents[3] / "ofdrw-converter" / "src" / "test" / "resources"
    with Document.open(samples / "发票示例.ofd") as invoice:
        assert 'data-ofd-source="Doc_0/Tpls/Tpl_0/Content.xml"' in invoice.page_svg(0)
        assert any(issue["code"] == "draw_param_reference" for issue in invoice.validate_references())
    with Document.open(samples / "文字横向-数科.ofd") as horizontal:
        svg = horizontal.page_svg(0)
        assert "<text " in svg or "<path " in svg
    with Document.open(samples / "intro-数科.ofd") as intro:
        assert any(path["boundary"] is None for path in intro.page_paths(7))
        assert "<path " in intro.page_svg(7)


def test_lists_and_removes_java_composite_graphics(tmp_path):
    path = FIXTURES.parents[3] / "ofdrw-converter" / "src" / "test" / "resources" / "y.ofd"
    with Document.open(path) as document:
        objects = document.page_composite_objects(0)
        assert objects
        assert objects[0]["id"]
        assert objects[0]["resource_id"]
        assert len(objects[0]["boundary"]) == 4
        with pytest.raises(IndexError):
            document.page_composite_objects(-1)
        with pytest.raises(OfdError, match="out of bounds"):
            document.page_composite_objects(document.page_count)
        with pytest.raises(OfdError, match="was not found"):
            document.add_composite_object(0, "missing", (1, 2, 10, 12))
        with pytest.raises(OfdError, match="alpha must be a byte"):
            document.add_composite_object(0, objects[0]["resource_id"], (1, 2, 10, 12), alpha=256)
        added = document.add_composite_object(
            0, objects[0]["resource_id"], (1, 2, 10, 12),
            ctm=(1, 0, 0, 1, 0, 0), alpha=180,
        )
        inserted = document.page_composite_objects(0)
        assert len(inserted) == len(objects) + 1
        assert inserted[-1]["id"] == str(added)
        assert inserted[-1]["alpha"] == 180
        document.remove_page_object(0, added)
        document.remove_page_object(0, objects[0]["id"])
        assert len(document.page_composite_objects(0)) == len(objects) - 1
        output = tmp_path / "without-composite.ofd"
        document.save(output)
    with Document.open(output) as reopened:
        assert len(reopened.page_composite_objects(0)) == len(objects) - 1


def test_creates_composite_path_resource_and_round_trips(tmp_path):
    with Document.new() as document:
        document.add_text_page("base", 40, 40)
        first = document.add_composite_path_object(
            0, "M 0 0 L 10 0 L 10 10 L 0 10 C", (2, 3, 10, 10),
            fill=True, stroke=False, fill_color=(255, 0, 0),
        )
        second = document.add_composite_path_object(
            0, "M 0 0 L 8 0 L 8 8 L 0 8 C", (18, 4, 8, 8),
            fill=True, stroke=False, fill_color=(0, 0, 255),
        )
        objects = document.page_composite_objects(0)
        assert [object_["id"] for object_ in objects] == [str(first), str(second)]
        units = document.page_composite_units(0)
        assert [unit["id"] for unit in units] == sorted(object_["resource_id"] for object_ in objects)
        assert all(unit["width"] > 0 and unit["height"] > 0 for unit in units)
        assert all(unit["thumbnail"] is None and unit["substitution"] is None for unit in units)
        with pytest.raises(IndexError):
            document.page_composite_units(-1)
        assert "fill=\"#ff0000\"" in document.page_svg(0)
        assert "fill=\"#0000ff\"" in document.page_svg(0)
        assert document.validate_references() == []
        with pytest.raises(OfdError, match="too few operands"):
            document.add_composite_path_object(0, "M 0", (1, 1, 10, 10))
        with pytest.raises(IndexError):
            document.add_composite_path_object(-1, "M 0 0 L 1 1", (1, 1, 10, 10))
        assert document.page_composite_objects(0) == objects
        output = tmp_path / "composite-path.ofd"
        document.save(output)
    with Document.open(output) as reopened:
        assert reopened.page_composite_objects(0) == objects
        assert reopened.validate_references() == []


def test_creates_composite_text_resource_and_round_trips(tmp_path):
    with Document.new() as document:
        document.add_text_page("base", 40, 40)
        added = document.add_composite_text_object(
            0, "Rust & 中", (2, 3, 20, 8), size=5, font_id=1,
        )
        objects = document.page_composite_objects(0)
        assert len(objects) == 1 and objects[0]["id"] == str(added)
        assert document.page_text(0) == ["base", "Rust & 中"]
        assert "Rust &amp; 中" in document.page_svg(0)
        assert document.validate_references() == []
        with pytest.raises(OfdError, match="font is not defined"):
            document.add_composite_text_object(0, "bad", (2, 3, 20, 8), font_id="missing")
        with pytest.raises(IndexError):
            document.add_composite_text_object(-1, "bad", (2, 3, 20, 8))
        assert document.page_composite_objects(0) == objects
        output = tmp_path / "composite-text.ofd"
        document.save(output)
    with Document.open(output) as reopened:
        assert reopened.page_text(0) == ["base", "Rust & 中"]
        assert reopened.validate_references() == []


def test_creates_composite_image_resource_and_round_trips(tmp_path):
    with Document.open(FIXTURES / "发票示例.ofd") as source:
        png = source.page_image_data(0, 0)
    with Document.new() as document:
        document.add_text_page("base", 40, 40)
        object_id = document.add_composite_image_object(0, png, "png", (2, 3, 20, 20))
        objects = document.page_composite_objects(0)
        assert len(objects) == 1 and objects[0]["id"] == str(object_id)
        assert document.page_images(0) == []
        images = document.page_composite_images(0)
        assert len(images) == 1 and images[0]["format"] == "PNG"
        unit_id = objects[0]["resource_id"]
        media_id = images[0]["resource_id"]
        before = document.page_composite_units(0)
        with pytest.raises(OfdError):
            document.set_page_composite_unit_images(0, unit_id, "missing", None)
        assert document.page_composite_units(0) == before
        document.set_page_composite_unit_images(0, unit_id, media_id, media_id)
        assert document.page_composite_units(0)[0]["thumbnail"] == media_id
        assert document.page_composite_units(0)[0]["substitution"] == media_id
        document.set_page_composite_unit_images(0, unit_id, None, None)
        assert document.page_composite_units(0) == before
        with pytest.raises(IndexError):
            document.set_page_composite_unit_images(-1, unit_id, None, None)
        assert document.page_composite_image_data(0, 0) == png
        with pytest.raises(OfdError):
            document.page_composite_image_data(0, 1)
        with pytest.raises(IndexError):
            document.page_composite_images(-1)
        assert "data:image/png;base64," in document.page_svg(0)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG\r\n\x1a\n")
        assert document.validate_references() == []


        with pytest.raises(IndexError):
            document.add_composite_image_object(-1, png, "png", (2, 3, 20, 20))
        with pytest.raises(ValueError):
            document.add_composite_image_object(0, png, "png", (2, 3, 20))
        with pytest.raises(OfdError):
            document.add_composite_image_object(0, b"bad", "png", (2, 3, 20, 20))
        assert document.page_composite_objects(0) == objects
        output = tmp_path / "composite-image.ofd"
        document.save(output)
    with Document.open(output) as reopened:
        assert reopened.page_composite_objects(0) == objects
        assert reopened.page_composite_image_data(0, 0) == png
        assert "data:image/png;base64," in reopened.page_svg(0)
        assert reopened.validate_references() == []
        resource = next(path for path in reopened.entries() if "CompositeRes_" in path)
        original = reopened.entry(resource)
        broken = original.replace(b'ResourceID="', b'ResourceID="999999', 1)
        reopened.set_entry(resource, broken)
        assert any(issue["code"] == "missing_media" for issue in reopened.validate_references())
        cyclic = original.replace(b"<ofd:ImageObject ", b"<ofd:CompositeObject ", 1)
        start = cyclic.index(b'ResourceID="') + len(b'ResourceID="')
        end = cyclic.index(b'"', start)
        cyclic = cyclic[:start] + objects[0]["resource_id"].encode() + cyclic[end:]
        reopened.set_entry(resource, cyclic)
        assert any(issue["code"] == "composite_cycle" for issue in reopened.validate_references())


def test_empty_composite_uses_thumbnail_in_python_previews():
    with Document.open(FIXTURES / "发票示例.ofd") as source:
        png = source.page_image_data(0, 0)
    with Document.new() as document:
        document.add_blank_page(20, 20)
        image_id = document.add_image_object(0, png, "png", (0, 0, 20, 20))
        media_id = document.page_images(0)[0]["resource_id"]
        document.remove_page_object(0, image_id)
        document.add_composite_path_object(
            0, "M 0 0 L 20 0 L 20 20 L 0 20 C", (0, 0, 20, 20),
            fill=True, stroke=False,
        )
        unit_id = document.page_composite_objects(0)[0]["resource_id"]
        path = f"Doc_0/CompositeRes_{unit_id}.xml"
        resource = document.entry(path).decode()
        start = resource.index("<ofd:Content>")
        end = resource.index("</ofd:Content>") + len("</ofd:Content>")
        document.set_entry(path, (resource[:start] + "<ofd:Content/>" + resource[end:]).encode())
        assert "data:image/png;base64," not in document.page_svg(0)
        document.set_page_composite_unit_images(0, unit_id, media_id, None)
        assert "data:image/png;base64," in document.page_svg(0)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG\r\n\x1a\n")
        assert document.to_pdf().startswith(b"%PDF-")
        assert document.validate_references() == []


def test_renders_png_preview():
    with Document.new() as document:
        document.add_text_page("PNG", 20, 20)
        png = document.page_png(0, 25.4)
        assert png.startswith(b"\x89PNG\r\n\x1a\n")
        assert int.from_bytes(png[16:20], "big") == 20
        assert int.from_bytes(png[20:24], "big") == 20
        with pytest.raises(OfdError, match="DPI"):
            document.page_png(0, 0)
        with pytest.raises(IndexError):
            document.page_png(-1)


def test_path_style_attributes_cross_native_binding():
    with Document.new() as document:
        document.add_path_page("M 1 10 L 19 10", (0, 0, 20, 20), 20, 20, False, True)
        path = document.pages()[0]["path"]
        original = document.entry(path)
        xml = original.replace(
            b'Stroke="true"',
            b'Stroke="true" LineWidth="2" Cap="Round" Join="Bevel" DashPattern="4 4"',
        )
        xml = xml.replace(
            b"</ofd:PathObject>",
            b'<ofd:StrokeColor Value="255 0 0" Alpha="128"/></ofd:PathObject>',
        )
        document.set_entry(path, xml)
        svg = ElementTree.fromstring(document.page_svg(0))
        graphic = svg.find(".//{http://www.w3.org/2000/svg}path")
        assert graphic.attrib["stroke-dasharray"] == "4 4"
        assert graphic.attrib["stroke-linecap"] == "round"
        assert float(graphic.attrib["stroke-opacity"]) == pytest.approx(128 / 255)

        resource_path = "Doc_0/PublicRes.xml"
        resources = document.entry(resource_path).replace(
            b"</ofd:Res>",
            b'<ofd:DrawParams><ofd:DrawParam ID="10" DashPattern="2 2"><ofd:StrokeColor Value="0 255 0"/></ofd:DrawParam></ofd:DrawParams></ofd:Res>',
        )
        document.set_entry(resource_path, resources)
        document.set_entry(
            path,
            original.replace(b"<ofd:PathObject ID=", b'<ofd:PathObject DrawParam="10" ID='),
        )
        svg = document.page_svg(0)
        assert 'stroke="#00ff00"' in svg
        assert 'stroke-dasharray="2 2"' in svg

        local_path = path.rsplit("/", 1)[0] + "/PageRes.xml"
        document.set_entry(
            local_path,
            b'<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016"><ofd:DrawParams><ofd:DrawParam ID="10"><ofd:StrokeColor Value="0 0 255"/></ofd:DrawParam></ofd:DrawParams></ofd:Res>',
        )
        document.set_entry(
            path,
            original.replace(
                b"<ofd:Content>",
                b"<ofd:PageRes>PageRes.xml</ofd:PageRes><ofd:Content>",
            ).replace(b"<ofd:PathObject ID=", b'<ofd:PathObject DrawParam="10" ID='),
        )
        assert 'stroke="#0000ff"' in document.page_svg(0)


def test_exports_multi_page_pdf():
    with Document.new() as document:
        with pytest.raises(OfdError, match="without pages"):
            document.to_pdf()
        document.add_text_page("first", 20, 30)
        document.add_text_page("second", 40, 50)
        pdf = document.to_pdf()
        assert pdf.startswith(b"%PDF-")
        assert b"/Count 2" in pdf


def test_adds_positioned_text_object(tmp_path):
    path = tmp_path / "positioned.ofd"
    with Document.new() as document:
        document.add_text_page("first")
        object_id = document.add_text_object(0, "定位 & Python", (30, 40, 80, 8), 5)
        assert document.page_text_objects(0)[1]["id"] == str(object_id)
        assert document.page_text(0)[1] == "定位 & Python"
        with pytest.raises(IndexError):
            document.add_text_object(-1, "bad", (0, 0, 10, 5))
        with pytest.raises(OfdError, match="font is not defined"):
            document.add_text_object(0, "bad", (0, 0, 10, 5), font_id="999")
        document.add_text_page("second")
        document.save(path)
    with Document.open(path) as document:
        assert document.page_text(0)[1] == "定位 & Python"
        assert document.page_text(1) == ["second"]


def test_adds_colored_positioned_text_atomically():
    with Document.new() as document:
        document.add_text_page("first")
        object_id = document.add_text_object(
            0, "red", (30, 40, 30, 8), 5, fill_color=(255, 0, 0),
        )
        paints = document.page_object_paints(0)
        assert next(item for item in paints if item["id"] == str(object_id))["fill"]["value"] == "255 0 0"
        before = document.to_bytes()
        with pytest.raises(OfdError, match="fill_color"):
            document.add_text_object(0, "bad", (30, 40, 30, 8), 5,
                                     fill_color=(256, 0, 0))
        assert document.to_bytes() == before


def test_shaped_text_object_preserves_ligature_text_and_glyphs(tmp_path):
    font = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" /
            "fixtures" / "dejavu-serif.ttf").read_bytes()
    path = tmp_path / "shaped.ofd"
    with Document.new() as document:
        font_id = document.add_embedded_font("DejaVu Serif", font)
        document.add_blank_page(40, 20)
        object_id = document.add_text_object(
            0, "AfiB", (5, 5, 30, 10), 6, font_id, shape=True,
            fill_color=(12, 34, 56),
        )
        assert document.page_object_paints(0)[0]["fill"]["value"] == "12 34 56"
        assert document.page_text_objects(0)[0]["id"] == str(object_id)
        ligature = document.page_text_objects(0)[0]["cg_transforms"][1]
        assert (ligature["code_position"], ligature["code_count"],
                ligature["glyph_count"]) == (1, 2, 1)
        assert len(ligature["glyphs"]) == 1
        assert document.page_text(0) == ["AfiB"]
        page_xml = document.entry(document.pages()[0]["path"]).decode()
        assert 'CodePosition="1" CodeCount="2" GlyphCount="1"' in page_xml
        assert 'data-ofd-glyph=' in document.page_svg(0)
        with pytest.raises(OfdError):
            document.add_text_object(0, "A\nB", (5, 5, 30, 10), 6, font_id,
                                     shape=True)
        with pytest.raises(OfdError):
            document.add_text_object(0, "fi", (5, 5, 30, 10), 6, shape=True)
        assert len(document.page_text_objects(0)) == 1
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["AfiB"]
        assert 'data-ofd-glyph=' in reopened.page_svg(0)


def test_shaped_text_language_selects_localized_glyphs():
    font = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" /
            "fixtures" / "dejavu-sans.ttf").read_bytes()
    with Document.new() as document:
        font_id = document.add_embedded_font("DejaVu Sans", font)
        document.add_blank_page(40, 20)
        document.add_text_object(0, "бгдпт", (2, 2, 30, 6), 5, font_id,
                                 shape=True, language="ru")
        document.add_text_object(0, "бгдпт", (2, 9, 30, 6), 5, font_id,
                                 shape=True, language="sr")
        objects = document.page_text_objects(0)
        glyphs = lambda obj: [glyph for mapping in obj["cg_transforms"]
                              for glyph in mapping["glyphs"]]
        assert glyphs(objects[0]) != glyphs(objects[1])
        assert document.page_text(0) == ["бгдпт", "бгдпт"]
        with pytest.raises(OfdError, match="language requires shape"):
            document.add_text_object(0, "бгдпт", (2, 2, 30, 6), 5, font_id,
                                     language="sr")
        with pytest.raises(OfdError, match="invalid shaping language"):
            document.add_text_object(0, "бгдпт", (2, 2, 30, 6), 5, font_id,
                                     shape=True, language="sr__invalid")
        assert len(document.page_text_objects(0)) == 2


def test_shaped_right_to_left_text_round_trips(tmp_path):
    font = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" /
            "fixtures" / "dejavu-sans.ttf").read_bytes()
    path = tmp_path / "arabic.ofd"
    with Document.new() as document:
        font_id = document.add_embedded_font("DejaVu Sans", font)
        document.add_blank_page(40, 20)
        document.add_text_object(0, "سلام", (5, 5, 30, 10), 6, font_id,
                                 shape=True)
        assert document.page_text(0) == ["سلام"]
        assert document.page_text_objects(0)[0]["cg_transforms"][0]["code_count"] == 4
        page_xml = document.entry(document.pages()[0]["path"]).decode()
        assert 'CodePosition="0" CodeCount="4"' in page_xml
        assert 'data-ofd-glyph=' in document.page_svg(0)
        document.add_text_object(0, "AسلامB", (5, 5, 30, 10), 6,
                                 font_id, shape=True)
        assert document.page_text(0) == ["سلام", "AسلامB"]
        mapping = document.page_text_objects(0)[1]["cg_transforms"][0]
        assert (mapping["code_count"], mapping["glyph_count"]) == (6, 5)
        assert len(mapping["glyphs"]) == 5
        page_xml = document.entry(document.pages()[0]["path"]).decode()
        assert 'CodePosition="0" CodeCount="6" GlyphCount="5"' in page_xml
        with pytest.raises(OfdError):
            document.add_text_object(0, "A\nسلام", (5, 5, 30, 10), 6,
                                     font_id, shape=True)
        assert len(document.page_text_objects(0)) == 2
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["سلام", "AسلامB"]
        assert reopened.page_text_objects(0)[1]["cg_transforms"][0]["glyph_count"] == 5
        assert 'data-ofd-glyph=' in reopened.page_svg(0)


def test_shaped_mixed_ltr_scripts_round_trip():
    font = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" /
            "fixtures" / "dejavu-sans.ttf").read_bytes()
    with Document.new() as document:
        font_id = document.add_embedded_font("DejaVu Sans", font)
        document.add_blank_page(40, 20)
        document.add_text_object(0, "AЖB", (5, 5, 30, 10), 6,
                                 font_id, shape=True)
        assert document.page_text(0) == ["AЖB"]
        mapping = document.page_text_objects(0)[0]["cg_transforms"]
        assert len(mapping) == 1
        assert (mapping[0]["code_count"], mapping[0]["glyph_count"]) == (3, 3)
        assert document.page_svg(0).count("data-ofd-glyph=") == 3


def test_duplicates_page_object_atomically():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        original = document.add_path_object(
            0, "M 0 0 L 10 10", (0, 0, 10, 10),
            dash_sequence=[1, 2, 3],
        )
        copied = document.duplicate_page_object(0, original)
        assert copied > original
        assert [path["id"] for path in document.page_paths(0)] == [str(original), str(copied)]
        assert document.page_object_stroke_styles(0)[1]["style"]["dash_sequence"] == [1, 2, 3]
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert len(reopened.page_paths(0)) == 2
        path = document.pages()[0]["path"]
        before = document.entry(path)
        with pytest.raises(OfdError):
            document.duplicate_page_object(0, "missing")
        assert document.entry(path) == before


def test_removes_page_object(tmp_path):
    path = tmp_path / "removed_object.ofd"
    with Document.new() as document:
        document.add_text_page("keep")
        object_id = document.add_text_object(0, "remove", (20, 30, 50, 8), 5)
        document.remove_page_object(0, object_id)
        assert document.page_text(0) == ["keep"]
        with pytest.raises(IndexError):
            document.remove_page_object(-1, object_id)
        with pytest.raises(OfdError, match="not found"):
            document.remove_page_object(0, object_id)
        document.save(path)
    with Document.open(path) as document:
        assert document.page_text(0) == ["keep"]
    path = tmp_path / "removed_existing.ofd"
    with Document.open(FIXTURES / "helloworld.ofd") as document:
        object_id = document.page_text_objects(0)[0]["id"]
        document.remove_page_object(0, object_id)
        document.save(path)
    with Document.open(path) as document:
        assert document.page_text(0) == []


def test_imports_pdf_user_unit_geometry(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/user-unit.pdf").read_bytes()
    path = tmp_path / "user-unit.ofd"
    with Document.from_pdf_bytes(source) as document:
        assert document.pages()[0]["physical_box"] == pytest.approx([0, 0, 50.8, 50.8])
        assert document.page_text(0) == ["Hi"]
        assert len(document.page_paths(0)) == 2
        assert document.page_images(0)[0]["boundary"] == pytest.approx([
            52 * 25.4 / 72, 40 * 25.4 / 72, 72 * 25.4 / 72, 72 * 25.4 / 72])
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["Hi"]
        assert len(reopened.page_images(0)) == 1


def test_imports_pdf_link_annotations():
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/links.pdf").read_bytes()
    with Document.from_pdf_bytes(source) as document:
        annotations = document.annotations()
        assert len(annotations) == 2
        assert annotations[0]["actions"][0]["attributes"]["URI"] == "https://example.com/?a=1&b=2"
        assert annotations[1]["actions"][0]["destination"]["page_id"] == document.pages()[1]["id"]
        assert document.to_pdf().count(b"/Subtype /Link") == 2
        assert document.validate_references() == []


def test_imports_nested_pdf_forms_with_scoped_resources(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/forms.pdf").read_bytes()
    path = tmp_path / "forms.ofd"
    with Document.from_pdf_bytes(source) as document:
        assert len(document.page_paths(0)) == 2
        assert len(document.page_images(0)) == 1
        assert document.page_text(0) == ["Hi"]
        assert b"<clipPath" in document.page_svg(0).encode()
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["Hi"]
        assert len(reopened.page_images(0)) == 1


def test_imports_pdf_device_cmyk_paths_and_image(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/cmyk.pdf").read_bytes()
    path = tmp_path / "cmyk.ofd"
    with Document.from_pdf_bytes(source) as document:
        assert len(document.page_paths(0)) == 2
        assert len(document.page_images(0)) == 1
        assert "#ff0000" in document.page_svg(0)
        assert "#00ffff" in document.page_svg(0)
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert len(reopened.page_images(0)) == 1


def test_imports_pdf_selected_color_spaces_with_form_resources(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/color-spaces.pdf").read_bytes()
    path = tmp_path / "color-spaces.ofd"
    with Document.from_pdf_bytes(source) as document:
        assert len(document.page_paths(0)) == 4
        svg = document.page_svg(0)
        for color in ("#808080", "#ff0000", "#0000ff", "#00ffff"):
            assert color in svg
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert len(reopened.page_paths(0)) == 4


@pytest.mark.parametrize("space", ["calgray", "calrgb"])
def test_imports_pdf_calibrated_colors_across_binding(tmp_path, space):
    fixture = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / f"{space}.pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert len(document.page_paths(0)) == 1
        assert '#808080' in document.page_svg(0)
        assert document.validate_references() == []
        target = tmp_path / f"{space}.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert '#808080' in reopened.page_svg(0)


def test_imports_pdf_lab_across_binding(tmp_path):
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/lab.pdf"
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert len(document.page_paths(0)) == 1
        assert '#777777' in document.page_svg(0)
        assert document.validate_references() == []
        target = tmp_path / "lab.ofd"
        document.save(target)
    with Document.open(target) as reopened:
        assert '#777777' in reopened.page_svg(0)


def test_imports_pdf_lab_jpeg_across_binding():
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/lab-jpeg.pdf"
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        images = document.page_images(0)
        assert len(images) == 1 and images[0]["format"] == "PNG"
        assert document.page_image_data(0, 0).startswith(b"\x89PNG\r\n\x1a\n")
        assert document.validate_references() == []


def test_imports_pdf_unmarked_yuvk_across_binding():
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/unmarked-yuvk.pdf"
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        images = document.page_images(0)
        assert len(images) == 1 and images[0]["format"] == "PNG"
        assert document.page_image_data(0, 0).startswith(b"\x89PNG\r\n\x1a\n")
        assert document.validate_references() == []


def test_reads_java_signed_template_with_mixed_path_case():
    fixture = Path(__file__).resolve().parents[4] / "ofdrw-sign/src/test/resources/v4signed.ofd"
    with Document.open(fixture) as document:
        assert document.page_sources(0)[0]["path"] == "Doc_0/Tpls/Tpl_0/Content.xml"
        assert document.page_svg(0).startswith("<svg ")


def test_reads_java_media_with_mixed_path_case():
    fixture = Path(__file__).resolve().parents[4] / "ofdrw-converter/src/test/resources/containsJPEG.ofd"
    with Document.open(fixture) as document:
        media = document.page_multimedia_resources(0)
        assert media[0]["path"] == "DOC_0/Res/Image_4.JPEG"
        assert document.page_image_data(0, 0).startswith(b"\xff\xd8")
        assert document.validate_references() == []


def test_document_resources_use_archive_case_for_reading_and_editing():
    fixture = Path(__file__).resolve().parents[4] / "ofdrw-converter/src/test/resources/containsJPEG.ofd"
    with Document.open(fixture) as document:
        root = "Doc_0/Document.xml"
        original = document.entry(root)
        changed = original.replace(b">DocumentRes.xml<", b">DOCUMENTRES.XML<")
        changed = changed.replace(b">PublicRes.xml<", b">PUBLICRES.XML<")
        assert changed != original
        document.set_entry(root, changed)
        assert document.page_image_data(0, 0).startswith(b"\xff\xd8")
        before = document.entry("Doc_0/PublicRes.xml")
        document.add_document_draw_param(0, {})
        assert document.entry("Doc_0/PublicRes.xml") != before
        with pytest.raises(OfdError, match="entry not found"):
            document.entry("Doc_0/PUBLICRES.XML")
        assert document.validate_references() == []


def test_imports_pdf_indexed_vector_colors(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/indexed-colors.pdf").read_bytes()
    path = tmp_path / "indexed-colors.ofd"
    with Document.from_pdf_bytes(source) as document:
        assert len(document.page_paths(0)) == 3
        svg = document.page_svg(0)
        for color in ("#ff0000", "#00ff00", "#0000ff"):
            assert color in svg
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert len(reopened.page_paths(0)) == 3


def test_imports_named_pdf_link_destinations(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/named-links.pdf").read_bytes()
    path = tmp_path / "named-links.ofd"
    with Document.from_pdf_bytes(source) as document:
        annotations = document.annotations()
        assert [item["actions"][0]["destination"]["kind"] for item in annotations] == ["XYZ", "Fit"]
        assert annotations[0]["actions"][0]["destination"]["zoom"] == 1.25
        assert all(item["actions"][0]["destination"]["page_id"] == document.pages()[1]["id"] for item in annotations)
        assert document.to_pdf().count(b"/Subtype /Link") == 2
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.annotations() == annotations


def test_imports_pdf_link_quadrilaterals(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/quad-links.pdf").read_bytes()
    path = tmp_path / "quad-links.ofd"
    with Document.from_pdf_bytes(source) as document:
        annotations = document.annotations()
        assert len(annotations[0]["actions"][0]["region"]["areas"]) == 2
        assert annotations[0]["actions"][0]["region"]["areas"][0]["commands"][3]["kind"] == "Close"
        assert document.to_pdf().count(b"/Subtype /Link") == 2
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.annotations() == annotations


@pytest.mark.parametrize("angle,size", [(90, (76.2, 50.8)), (180, (50.8, 76.2)), (270, (76.2, 50.8))])
def test_imports_pdf_rotated_page(tmp_path, angle, size):
    source = (Path(__file__).resolve().parents[3] / f"crates/ofdrw/tests/fixtures/rotate-{angle}.pdf").read_bytes()
    path = tmp_path / f"rotate-{angle}.ofd"
    with Document.from_pdf_bytes(source) as document:
        assert document.pages()[0]["physical_box"] == pytest.approx([0, 0, *size])
        assert document.page_text(0) == ["Hi"]
        assert len(document.page_paths(0)) == 2
        assert len(document.page_images(0)) == 1
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["Hi"]
        assert len(reopened.page_images(0)) == 1


def test_imports_pdf_rgb_page_group_with_rotated_text():
    source = (
        Path(__file__).resolve().parents[3]
        / "crates/ofdrw/tests/fixtures/page-group-rotated-text.pdf"
    ).read_bytes()
    with Document.from_pdf_bytes(source) as document:
        assert document.page_text(0) == ["AB"]
        assert len(document.page_paths(0)) == 1
        assert "#ff0000" in document.page_svg(0)
        assert document.validate_references() == []


def test_imports_pdf_affine_image(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/affine-image.pdf").read_bytes()
    path = tmp_path / "affine-image.ofd"
    with Document.from_pdf_bytes(source) as document:
        images = document.page_images(0)
        assert len(images) == 1
        assert images[0]["ctm"] is not None
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert len(reopened.page_images(0)) == 1


def test_imports_pdf_packed_gray_image(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/packed-gray.pdf").read_bytes()
    path = tmp_path / "packed-gray.ofd"
    with Document.from_pdf_bytes(source) as document:
        assert len(document.page_images(0)) == 1
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert len(reopened.page_images(0)) == 1


def test_imports_negative_pdf_horizontal_scale():
    source = (
        Path(__file__).resolve().parents[3]
        / "crates/ofdrw/tests/fixtures/negative-horizontal-scale.pdf"
    ).read_bytes()
    with Document.from_pdf_bytes(source) as document:
        assert document.page_text(0) == ["AB", "C", "D"]
        assert [item["h_scale"] for item in document.page_text_objects(0)] == [0.5, 0.5, None]
        objects = document.page_objects(0)
        assert objects[0]["ctm"][0] == pytest.approx(-1)
        assert objects[2]["boundary"][0] == pytest.approx(11 * 25.4 / 72)
        assert document.validate_references() == []
        data = document.to_bytes()
    with Document.from_bytes(data) as reopened:
        assert reopened.page_text(0) == ["AB", "C", "D"]


def test_imports_pdf_vector_fixture(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/vector.pdf").read_bytes()
    path = tmp_path / "imported.ofd"
    with Document.from_pdf_bytes(source) as document:
        assert document.page_count == 1
        assert len(document.page_paths(0)) == 2
        assert "#ff0000" in document.page_svg(0)
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert len(reopened.page_paths(0)) == 2
    with pytest.raises(OfdError):
        Document.from_pdf_bytes(b"invalid")
    image_source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/image.pdf").read_bytes()
    with Document.from_pdf_bytes(image_source) as document:
        assert len(document.page_images(0)) == 1
        assert document.validate_references() == []
    text_source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/text.pdf").read_bytes()
    with Document.from_pdf_bytes(text_source) as document:
        assert document.page_text(0) == ["Hello PDF"]
        assert document.validate_references() == []
    spaced_source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/spacing.pdf").read_bytes()
    with Document.from_pdf_bytes(spaced_source) as document:
        assert document.page_text(0) == ["AB", "CD"]
        assert document.validate_references() == []
    scaled_source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/horizontal-scale.pdf").read_bytes()
    with Document.from_pdf_bytes(scaled_source) as document:
        assert document.page_text(0) == ["AB", "C", "D"]
        objects = document.page_text_objects(0)
        assert [item["h_scale"] for item in objects] == [0.5, 0.5, 1.5]
        assert objects[1]["boundary"][0] == pytest.approx(16 * 25.4 / 72)
        assert objects[2]["boundary"][0] == pytest.approx(19 * 25.4 / 72)
        assert 'HScale="0.5"' in document.entry(document.pages()[0]["path"]).decode()
        assert document.validate_references() == []
    appearance_source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/text-appearance.pdf").read_bytes()
    with Document.from_pdf_bytes(appearance_source) as document:
        assert document.page_text(0) == ["A", "B", "C", "D", "E"]
        objects = document.page_text_objects(0)
        assert objects[4]["boundary"][1] == pytest.approx(objects[0]["boundary"][1] - 2 * 25.4 / 72)
        xml = document.entry(document.pages()[0]["path"]).decode()
        assert 'Fill="false" Stroke="true"' in xml
        assert '<ofd:FillColor Value="255 0 0"' in xml
        assert '<ofd:StrokeColor Value="0 0 255"' in xml
        assert document.validate_references() == []


def test_imports_pdf_clipping_fixture(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/clip.pdf").read_bytes()
    path = tmp_path / "clipped.ofd"
    with Document.from_pdf_bytes(source) as document:
        assert len(document.page_paths(0)) == 2
        assert "<clipPath " in document.page_svg(0)
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert "<clipPath " in reopened.page_svg(0)
        assert reopened.validate_references() == []


def test_imports_pdf_even_odd_fill_fixture(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/evenodd.pdf").read_bytes()
    path = tmp_path / "even_odd_import.ofd"
    with Document.from_pdf_bytes(source) as document:
        assert len(document.page_paths(0)) == 1
        assert 'fill-rule="evenodd"' in document.page_svg(0)
        document.save(path)
    with Document.open(path) as reopened:
        assert 'fill-rule="evenodd"' in reopened.page_svg(0)


def test_imports_pdf_stroke_style_fixture(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/stroke.pdf").read_bytes()
    path = tmp_path / "stroke_import.ofd"
    with Document.from_pdf_bytes(source) as document:
        svg = document.page_svg(0)
        assert 'stroke-linecap="round"' in svg
        assert 'stroke-linejoin="bevel"' in svg
        assert 'stroke-dasharray="' in svg
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_svg(0) == svg


def test_imports_pdf_alpha_fixture(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/alpha.pdf").read_bytes()
    path = tmp_path / "alpha_import.ofd"
    with Document.from_pdf_bytes(source) as document:
        svg = document.page_svg(0)
        assert 'fill-opacity="0.5019607843137255"' in svg
        assert 'stroke-opacity="0.25098039215686274"' in svg
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_svg(0) == svg


def test_imports_pdf_blend_mode_fixture(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/blend.pdf").read_bytes()
    path = tmp_path / "blend_import.ofd"
    with Document.from_pdf_bytes(source) as document:
        svg = document.page_svg(0)
        assert svg.count("mix-blend-mode:darken") == 1
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        assert b"/Darken" in document.to_pdf()
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_svg(0) == svg


def test_imports_pdf_text_clip_fixture(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/text-clip.pdf").read_bytes()
    path = tmp_path / "text_clip_import.ofd"
    with Document.from_pdf_bytes(source) as document:
        assert document.page_text(0) == ["A", "B"]
        svg = document.page_svg(0)
        assert svg.count('<clipPath id="ofd-clip-') == 1
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_svg(0) == svg


def test_imports_pdf_soft_mask_fixture(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/soft-mask.pdf").read_bytes()
    path = tmp_path / "soft_mask_import.ofd"
    with Document.from_pdf_bytes(source) as document:
        image = document.page_image_data(0, 0)
        assert image.startswith(b"\x89PNG\r\n\x1a\n")
        assert image[25] == 6  # PNG RGBA color type
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_image_data(0, 0) == image


@pytest.mark.parametrize("name", ["cmyk-icc-jpeg.pdf", "ycck-icc-jpeg.pdf"])
def test_imports_pdf_cmyk_icc_jpeg_fixture(tmp_path, name):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / name).read_bytes()
    path = tmp_path / f"{name}.ofd"
    with Document.from_pdf_bytes(source) as document:
        image = document.page_image_data(0, 0)
        assert image.startswith(b"\x89PNG\r\n\x1a\n")
        assert image[25] == 2  # PNG RGB color type
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_image_data(0, 0) == image


@pytest.mark.parametrize("name", [
    "cmyk-mask.pdf", "ycck-mask.pdf",
    "cmyk-decode-mask.pdf", "ycck-decode-mask.pdf",
])
def test_imports_pdf_cmyk_jpeg_color_key_mask(tmp_path, name):
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / name
    path = tmp_path / f"{name}.ofd"
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        image = document.page_image_data(0, 0)
        assert image.startswith(b"\x89PNG\r\n\x1a\n")
        assert image[25] == 6  # RGBA
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_image_data(0, 0) == image


def test_imports_pdf_color_key_mask_fixture(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/color-key-mask.pdf").read_bytes()
    path = tmp_path / "color_key_import.ofd"
    with Document.from_pdf_bytes(source) as document:
        image = document.page_image_data(0, 0)
        assert image.startswith(b"\x89PNG\r\n\x1a\n")
        assert image[25] == 6  # PNG RGBA color type
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_image_data(0, 0) == image


def test_imports_pdf_indexed_image_fixture(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/indexed-image.pdf").read_bytes()
    path = tmp_path / "indexed_image_import.ofd"
    with Document.from_pdf_bytes(source) as document:
        assert len(document.page_images(0)) == 1
        image = document.page_image_data(0, 0)
        assert image.startswith(b"\x89PNG\r\n\x1a\n")
        assert image[25] == 2  # Expanded RGB palette
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_image_data(0, 0) == image


def test_imports_pdf_chained_image_filters(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/asciihex-runlength-image.pdf").read_bytes()
    path = tmp_path / "filtered_image_import.ofd"
    with Document.from_pdf_bytes(source) as document:
        image = document.page_image_data(0, 0)
        assert image.startswith(b"\x89PNG\r\n\x1a\n")
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_image_data(0, 0) == image


def test_imports_pdf_lzw_predictor_image(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/lzw-predictor-image.pdf").read_bytes()
    path = tmp_path / "lzw_image_import.ofd"
    with Document.from_pdf_bytes(source) as document:
        image = document.page_image_data(0, 0)
        assert image.startswith(b"\x89PNG\r\n\x1a\n")
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_image_data(0, 0) == image


def test_imports_pdf_explicit_mask_fixture(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/explicit-mask.pdf").read_bytes()
    path = tmp_path / "explicit_mask_import.ofd"
    with Document.from_pdf_bytes(source) as document:
        image = document.page_image_data(0, 0)
        assert image.startswith(b"\x89PNG\r\n\x1a\n")
        assert image[25] == 6  # PNG RGBA color type
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_image_data(0, 0) == image


def test_imports_pdf_stencil_mask_fixture(tmp_path):
    source = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/stencil-mask.pdf").read_bytes()
    path = tmp_path / "stencil_mask_import.ofd"
    with Document.from_pdf_bytes(source) as document:
        image = document.page_image_data(0, 0)
        assert image.startswith(b"\x89PNG\r\n\x1a\n")
        assert image[25] == 6  # PNG RGBA color type
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_image_data(0, 0) == image


def test_creates_layers_and_moves_objects(tmp_path):
    path = tmp_path / "layered.ofd"
    with Document.new() as document:
        document.add_blank_page(20, 20)
        background = document.add_page_layer(0, "Background")
        foreground = document.add_page_layer(0, "Foreground")
        commands = "M 0 0 L 20 0 L 20 20 L 0 20 C"
        blue = document.add_path_object(0, commands, (0, 0, 20, 20), fill=True, stroke=False, fill_color=(0, 0, 255))
        document.move_page_object_to_layer(0, blue, background)
        red = document.add_path_object(0, commands, (0, 0, 20, 20), fill=True, stroke=False, fill_color=(255, 0, 0))
        document.move_page_object_to_layer(0, red, foreground)
        assert [layer["object_count"] for layer in document.page_layers(0)] == [1, 0, 1]
        svg = document.page_svg(0)
        assert svg.index("#0000ff") < svg.index("#ff0000")
        with pytest.raises(OfdError):
            document.add_page_layer(0, "bad")
        with pytest.raises(OfdError):
            document.add_page_layer(0, "Body", "missing")
        with pytest.raises(OfdError):
            document.move_page_object_to_layer(0, "missing", foreground)
        with pytest.raises(OfdError):
            document.move_page_object_to_layer(0, blue, "missing")
        with pytest.raises(IndexError):
            document.add_page_layer(-1, "Body")
        with pytest.raises(IndexError):
            document.move_page_object_to_layer(-1, blue, background)
        with pytest.raises(OfdError):
            document.remove_page_layer(0, foreground)
        empty = document.add_page_layer(0, "Foreground")
        document.remove_page_layer(0, empty)
        with pytest.raises(OfdError):
            document.remove_page_layer(0, empty)
        with pytest.raises(IndexError):
            document.remove_page_layer(-1, background)
        document.save(path)
    with Document.open(path) as document:
        assert [layer["object_count"] for layer in document.page_layers(0)] == [1, 0, 1]
    with pytest.raises(OfdError):
        document.add_page_layer(0, "Body")
    with pytest.raises(OfdError):
        document.remove_page_layer(0, background)


def test_reorders_page_objects(tmp_path):
    path = tmp_path / "reordered.ofd"
    commands = "M 0 0 L 20 0 L 20 20 L 0 20 C"
    with Document.new() as document:
        document.add_blank_page(20, 20)
        red = document.add_path_object(0, commands, (0, 0, 20, 20),
                                       fill=True, stroke=False, fill_color=(255, 0, 0))
        green = document.add_path_object(0, commands, (0, 0, 20, 20),
                                         fill=True, stroke=False, fill_color=(0, 255, 0))
        blue = document.add_path_object(0, commands, (0, 0, 20, 20),
                                        fill=True, stroke=False, fill_color=(0, 0, 255))
        document.move_page_object(0, blue, red)
        assert [item["id"] for item in document.page_paths(0)] == [str(blue), str(red), str(green)]
        document.move_page_object(0, red)
        assert [item["id"] for item in document.page_paths(0)] == [str(blue), str(green), str(red)]
        before = document.page_svg(0)
        with pytest.raises(OfdError):
            document.move_page_object(0, red, "missing")
        assert document.page_svg(0) == before
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_svg(0) == before
        assert reopened.validate_references() == []


def test_page_object_name_round_trips_across_binding():
    with Document.new() as document:
        document.add_text_page("named", 40, 30)
        object_id = document.page_objects(0)[0]["id"]
        document.set_page_object_name(0, object_id, '发票 & "A"')
        assert document.page_objects(0)[0]["name"] == '发票 & "A"'
        before = document.to_bytes()
        with pytest.raises(ValueError):
            document.set_page_object_name(0, object_id, "bad\0name")
        assert document.to_bytes() == before
        with Document.from_bytes(before) as reopened:
            assert reopened.page_objects(0)[0]["name"] == '发票 & "A"'
        document.set_page_object_name(0, object_id, None)
        assert document.page_objects(0)[0]["name"] is None


def test_page_object_blend_mode_round_trips_across_binding():
    with Document.new() as document:
        document.add_text_page("blend", 40, 30)
        object_id = document.page_objects(0)[0]["id"]
        document.set_page_object_blend_mode(0, object_id, "darken")
        assert document.page_objects(0)[0]["blend_mode"] == "Darken"
        assert "mix-blend-mode:darken" in document.page_svg(0)
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.set_page_object_blend_mode(0, object_id, "invalid")
        assert document.to_bytes() == before
        with Document.from_bytes(before) as reopened:
            assert reopened.page_objects(0)[0]["blend_mode"] == "Darken"
        document.set_page_object_blend_mode(0, object_id, None)
        assert document.page_objects(0)[0]["blend_mode"] is None


def test_page_object_clip_rectangles_round_trip_across_binding():
    with Document.new() as document:
        document.add_path_page("M 0 0 L 20 0 L 20 20 L 0 20 C",
                               [0, 0, 20, 20], 20, 20, fill=True, stroke=False)
        object_id = document.page_objects(0)[0]["id"]
        document.set_page_object_clip_rects(0, object_id, [[0, 0, 10, 20]])
        assert "ofd-clip-" in document.page_svg(0)
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.set_page_object_clip_rects(0, object_id, [[0, 0, -1, 20]])
        assert document.to_bytes() == before
        with Document.from_bytes(before) as reopened:
            assert "ofd-clip-" in reopened.page_svg(0)
        document.set_page_object_clip_rects(0, object_id, [])
        assert "ofd-clip-" not in document.page_svg(0)


def test_external_empty_and_malformed_clips_across_binding():
    with Document.new() as document:
        document.add_path_page("M 0 0 L 20 0 L 20 20 L 0 20 C",
                               [0, 0, 20, 20], 20, 20, fill=True, stroke=False)
        object_id = document.page_objects(0)[0]["id"]
        document.set_page_object_clip_rects(0, object_id, [[0, 0, 10, 10]])
        page_path = document.pages()[0]["path"]
        original = document.entry(page_path)
        start = original.index(b"<ofd:Clips")
        end = original.index(b"</ofd:Clips>", start) + len(b"</ofd:Clips>")

        document.set_entry(page_path, original[:start] + b"<ofd:Clips/>" + original[end:])
        assert document.validate_references() == []
        assert document.page_object_clips(0)[0]["clips"] == []
        assert "ofd-clip-" not in document.page_svg(0)

        broken = b"<ofd:Clips><ofd:Clip><ofd:Area><ofd:Path/><ofd:Text/>" \
                 b"</ofd:Area></ofd:Clip></ofd:Clips>"
        document.set_entry(page_path, original[:start] + broken + original[end:])
        assert any(issue["code"] == "invalid_clip_area"
                   for issue in document.validate_references())
        with pytest.raises(OfdError):
            document.page_object_clips(0)
        with pytest.raises(OfdError):
            document.page_svg(0)


def test_page_object_path_clips_round_trip_across_binding():
    with Document.new() as document:
        document.add_path_page("M 0 0 L 20 0 L 20 20 L 0 20 C",
                               [0, 0, 20, 20], 20, 20, fill=True, stroke=False)
        object_id = document.page_objects(0)[0]["id"]
        area = {"boundary": [0, 0, 10, 20],
                "commands": "M 0 0 L 10 0 L 10 20 L 0 20 C",
                "even_odd": True}
        document.set_page_object_path_clips(0, object_id, [[area]])
        assert "Rule=\"Even-Odd\"" in document.entry("Doc_0/Pages/Page_0/Content.xml").decode()
        assert "ofd-clip-" in document.page_svg(0)
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.set_page_object_path_clips(0, object_id, [[]])
        assert document.to_bytes() == before
        with Document.from_bytes(before) as reopened:
            assert "ofd-clip-" in reopened.page_svg(0)
        draw_param = document.add_document_draw_param(0, {"line_width": 2})
        stroke = {"boundary": [0, 0, 20, 20], "commands": "M 2 10 L 18 10",
                  "fill": False, "stroke": True, "draw_param": str(draw_param)}
        document.set_page_object_path_clips(0, object_id, [[stroke]])
        clip = next(item for item in document.page_object_clips(0)
                    if item["id"] == str(object_id))["clips"][0][0]
        assert clip["fill"] is False and clip["stroke"] is True
        assert clip["draw_param"] == str(draw_param)
        assert 'stroke-width="2"' in document.page_svg(0)
        document.set_page_object_path_clips(0, object_id, [])
        assert "ofd-clip-" not in document.page_svg(0)


def test_mixed_text_and_path_clips_round_trip_across_binding():
    with Document.new() as document:
        document.add_text_page("font", 30, 30)
        font_id = document.fonts()[0]["id"]
        object_id = document.add_path_object(
            0, "M 0 0 L 20 0 L 20 20 L 0 20 C", [0, 0, 20, 20],
            fill=True, stroke=False)
        clips = [[
            {"kind": "Path", "boundary": [0, 0, 5, 20],
             "commands": "M 0 0 L 5 0 L 5 20 L 0 20 C"},
            {"kind": "Text", "boundary": [0, 0, 20, 20],
             "font_id": font_id, "size": 14, "text": "O", "x": 10, "y": 15},
        ]]
        document.set_page_object_clips(0, object_id, clips)
        objects = document.page_object_clips(0)
        selected = next(item for item in objects if item["id"] == str(object_id))
        assert [area["kind"] for area in selected["clips"][0]] == ["Path", "Text"]
        assert selected["clips"][0][0]["commands"] == clips[0][0]["commands"]
        assert selected["clips"][0][0]["fill"] is True
        assert selected["clips"][0][0]["stroke"] is False
        assert selected["clips"][0][1]["text"] == "O"
        assert selected["clips"][0][1]["text_codes"][0]["x"] == 10
        assert selected["clips"][0][1]["text_codes"][0]["y"] == 15
        assert "TextCode" in selected["clips"][0][1]["xml"]
        assert "ofd-clip-" in document.page_svg(0)
        before = document.to_bytes()
        clips[0][1]["font_id"] = "999999"
        with pytest.raises(OfdError):
            document.set_page_object_clips(0, object_id, clips)
        assert document.to_bytes() == before
        with Document.from_bytes(before) as reopened:
            assert "ofd-clip-" in reopened.page_svg(0)
            assert reopened.page_object_clips(0) == objects
        rich = [[{"kind": "Text", "boundary": [0, 0, 20, 20],
                 "font_id": font_id, "size": 14, "text": "IX", "x": 2, "y": 15,
                 "text_codes": [{"x": 2, "y": 15, "text": "I"},
                                {"x": 8, "y": 15, "text": "X"}],
                 "cg_transforms": [{"code_position": 0, "code_count": 2,
                                    "glyph_count": 1, "glyphs": [1]}]}]]
        document.set_page_object_clips(0, object_id, rich)
        area = next(item for item in document.page_object_clips(0)
                    if item["id"] == str(object_id))["clips"][0][0]
        assert area["text_codes"][1]["x"] == 8
        assert area["cg_transforms"][0]["glyphs"] == [1]
        draw_param = document.add_document_draw_param(0, {"line_width": 2})
        stroke = [[{"kind": "Text", "boundary": [0, 0, 20, 20],
                   "font_id": font_id, "size": 14, "text": "O", "x": 3, "y": 16,
                   "fill": False, "stroke": True, "draw_param": str(draw_param)}]]
        document.set_page_object_clips(0, object_id, stroke)
        area = next(item for item in document.page_object_clips(0)
                    if item["id"] == str(object_id))["clips"][0][0]
        assert area["fill"] is False and area["stroke"] is True
        assert area["draw_param"] == str(draw_param)
        assert 'stroke-width="2"' in document.page_svg(0)
        document.set_page_object_clips(0, object_id, [])
        assert "ofd-clip-" not in document.page_svg(0)
        assert not next(item for item in document.page_object_clips(0)
                        if item["id"] == str(object_id))["clips"]


def test_page_layer_type_and_draw_param_round_trip():
    with Document.new() as document:
        document.add_text_page("layer text", 40, 30)
        layer_id = document.add_page_layer(0, "Foreground")
        draw_param = document.add_document_draw_param(0, {"line_width": 1.25})
        document.set_page_layer_draw_param(0, layer_id, draw_param)
        document.set_page_layer_type(0, layer_id, "background")
        document.set_page_layer_name(0, layer_id, "图层 & A")
        layer = document.page_layers(0)[0]
        assert layer["id"] == str(layer_id)
        assert layer["layer_type"] == "Background"
        assert layer["name"] == "图层 & A"
        assert layer["draw_param"] == str(draw_param)
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.set_page_layer_type(0, layer_id, "Invalid")
        with pytest.raises(OfdError):
            document.set_page_layer_draw_param(0, layer_id, "missing")
        with pytest.raises(ValueError):
            document.set_page_layer_name(0, layer_id, "bad\0name")
        assert document.to_bytes() == before
        with Document.from_bytes(before) as reopened:
            assert reopened.page_layers(0)[0] == layer
        document.set_page_layer_draw_param(0, layer_id, None)
        document.set_page_layer_name(0, layer_id, None)
        assert document.page_layers(0)[0]["draw_param"] is None
        assert document.page_layers(0)[0]["name"] is None


def test_updates_page_object_visibility_and_alpha(tmp_path):
    path = tmp_path / "appearance.ofd"
    with Document.new() as document:
        document.add_blank_page(20, 20)
        text_id = document.add_text_object(0, "hello", (1, 1, 15, 8), 4)
        path_id = document.add_path_object(
            0, "M 0 0 L 20 0 L 20 20 L 0 20 C", (0, 0, 20, 20),
            fill=True, stroke=False, fill_color=(255, 0, 0))
        document.set_page_object_visible(0, text_id, False)
        assert document.page_text(0) == []
        document.set_page_object_visible(0, text_id, True)
        assert document.page_text(0) == ["hello"]
        document.set_page_object_alpha(0, path_id, 128)
        objects = document.page_objects(0)
        assert [item["kind"] for item in objects] == ["TextObject", "PathObject"]
        assert objects[1]["alpha"] == 128
        assert objects[0]["visible"] is True
        svg = document.page_svg(0)
        assert "opacity=\"0.501" in svg
        with pytest.raises(ValueError):
            document.set_page_object_alpha(0, path_id, 256)
        with pytest.raises(TypeError):
            document.set_page_object_visible(0, text_id, 1)
        with pytest.raises(OfdError):
            document.set_page_object_visible(0, "missing", False)
        assert document.page_svg(0) == svg
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["hello"]
        assert reopened.page_svg(0) == svg
        assert reopened.page_objects(0) == objects

    with Document.open(FIXTURES / "发票示例.ofd") as invoice:
        assert any(item["template_id"] is not None for item in invoice.page_objects(0))


def test_edits_page_object_geometry(tmp_path):
    path = tmp_path / "geometry.ofd"
    with Document.new() as document:
        document.add_blank_page(30, 20)
        object_id = document.add_path_object(
            0, "M 0 0 L 5 0 L 5 5 L 0 5 C", (2, 2, 5, 5),
            fill=True, stroke=False, fill_color=(255, 0, 0))
        document.set_page_object_boundary(0, object_id, (12, 2, 5, 5))
        assert document.page_paths(0)[0]["boundary"] == [12, 2, 5, 5]
        document.set_page_object_ctm(0, object_id, (1, 0, 0, 1, 5, 0))
        assert document.page_paths(0)[0]["ctm"] == [1, 0, 0, 1, 5, 0]
        assert "matrix(1 0 0 1 5 0)" in document.page_svg(0)
        document.set_page_object_ctm(0, object_id, None)
        assert document.page_paths(0)[0]["ctm"] is None
        before = document.page_svg(0)
        with pytest.raises(OfdError):
            document.set_page_object_boundary(0, object_id, (0, 0, -1, 5))
        with pytest.raises(ValueError):
            document.set_page_object_ctm(0, object_id, (1, 2))
        with pytest.raises(ValueError):
            document.set_page_object_ctm(0, object_id, (1, 0, 0, 1, float("nan"), 0))
        assert document.page_svg(0) == before
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_paths(0)[0]["boundary"] == [12, 2, 5, 5]
        assert reopened.validate_references() == []


def test_adds_positioned_path_object(tmp_path):
    path = tmp_path / "positioned_path.ofd"
    with Document.new() as document:
        document.add_text_page("first")
        object_id = document.add_path_object(
            0, "M 0 0 L 20 10", (10, 20, 20, 10),
            fill=True, line_width=0.8,
            fill_color=(0, 0, 255), stroke_color=(255, 0, 0),
        )
        assert document.page_paths(0)[0]["id"] == str(object_id)
        assert "#ff0000" in document.page_svg(0)
        with pytest.raises(IndexError):
            document.add_path_object(-1, "M 0 0", (0, 0, 10, 10))
        with pytest.raises(OfdError, match="RGB"):
            document.add_path_object(0, "M 0 0", (0, 0, 10, 10), fill_color=(256, 0, 0))
        with pytest.raises(ValueError):
            document.add_path_object(0, "M 0 0", (float("nan"), 0, 10, 10))
        document.save(path)
    with Document.open(path) as document:
        assert document.page_paths(0)[0]["id"] == str(object_id)


def test_writes_even_odd_path_styles(tmp_path):
    path = tmp_path / "even_odd.ofd"
    commands = "M 0 0 L 20 0 L 20 20 L 0 20 C M 5 5 L 15 5 L 15 15 L 5 15 C"
    with Document.new() as document:
        document.add_blank_page(20, 20)
        document.add_path_object(0, commands, (0, 0, 20, 20),
                                 fill=True, stroke=False, even_odd=True, alpha=128)
        document.add_composite_path_object(0, commands, (0, 0, 20, 20),
                                           fill=True, stroke=False, even_odd=True, alpha=128)
        assert document.page_svg(0).count('fill-rule="evenodd"') == 2
        assert document.page_svg(0).count('opacity="0.5019607843137255"') == 2
        with pytest.raises(OfdError, match="even_odd"):
            document.add_path_object(0, commands, (0, 0, 20, 20), even_odd="yes")
        with pytest.raises(OfdError, match="alpha"):
            document.add_path_object(0, commands, (0, 0, 20, 20), alpha=256)
        document.save(path)
    with Document.open(path) as document:
        assert document.page_svg(0).count('fill-rule="evenodd"') == 2
        assert document.page_svg(0).count('opacity="0.5019607843137255"') == 2


def test_writes_path_stroke_styles(tmp_path):
    path = tmp_path / "styled_stroke.ofd"
    options = dict(cap="round", join="bevel", miter_limit=5,
                   dash_pattern=(3, 2), dash_offset=1, line_width=2,
                   fill=True, fill_alpha=128, stroke_alpha=64)
    with Document.new() as document:
        document.add_blank_page(30, 20)
        document.add_path_object(0, "M 2 10 L 28 10", (0, 0, 30, 20), **options)
        document.add_composite_path_object(0, "M 2 10 L 28 10", (0, 0, 30, 20), **options)
        svg = document.page_svg(0)
        for attribute in ('stroke-linecap="round"', 'stroke-linejoin="bevel"',
                          'stroke-miterlimit="5"', 'stroke-dasharray="3 2"',
                          'stroke-dashoffset="1"',
                          'fill-opacity="0.5019607843137255"',
                          'stroke-opacity="0.25098039215686274"'):
            assert svg.count(attribute) == 2
        with pytest.raises(OfdError, match="cap"):
            document.add_path_object(0, "M 0 0 L 1 1", (0, 0, 10, 10), cap="flat")
        with pytest.raises(OfdError, match="stroke style"):
            document.add_path_object(0, "M 0 0 L 1 1", (0, 0, 10, 10), dash_pattern=(3, 0))
        with pytest.raises(OfdError, match="fill_alpha"):
            document.add_path_object(0, "M 0 0 L 1 1", (0, 0, 10, 10), fill_alpha=256)
        document.save(path)
    with Document.open(path) as document:
        assert document.page_svg(0) == svg


def test_adds_text_watermark_annotation(tmp_path):
    path = tmp_path / "watermark.ofd"
    with Document.new() as document:
        document.add_text_page("first")
        mark_id = document.add_text_watermark(0, "水印 & Python", (10, 20, 60, 10), 5)
        assert document.annotations()[0]["id"] == str(mark_id)
        assert "水印 &amp; Python" in document.page_svg(0)
        with pytest.raises(IndexError):
            document.add_text_watermark(-1, "bad", (0, 0, 10, 10))
        with pytest.raises(ValueError):
            document.add_text_watermark(0, "bad", (float("nan"), 0, 10, 10))
        with pytest.raises(OfdError, match="watermark text"):
            document.add_text_watermark(0, "", (0, 0, 10, 10))
        document.add_text_page("second")
        document.save(path)
    with Document.open(path) as document:
        assert document.annotations()[0]["text"] == ["水印 & Python"]
        assert document.page_text(1) == ["second"]
    path = tmp_path / "existing_watermark.ofd"
    with Document.open(FIXTURES / "发票示例.ofd") as document:
        document.add_text_watermark(0, "appended", (10, 20, 60, 10))
        document.save(path)
    with Document.open(path) as document:
        assert len(document.annotations()) == 3
        assert document.annotations()[2]["text"] == ["appended"]


def test_text_stamp_annotation_round_trip(tmp_path):
    path = tmp_path / "text-stamp.ofd"
    with Document.new() as document:
        document.add_text_page("base")
        stamp_id = document.add_text_stamp_annotation(
            0, "批准 & <同意>", (10, 20, 60, 10), 5,
            color=(180, 20, 30), opacity=192)
        annotations = document.annotations()
        assert annotations[0]["id"] == str(stamp_id)
        assert annotations[0]["type"] == "Stamp"
        assert annotations[0]["text"] == ["批准 & <同意>"]
        assert "#b4141e" in document.page_svg(0)
        with pytest.raises(OfdError, match="stamp text"):
            document.add_text_stamp_annotation(0, "", (10, 20, 60, 10))
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.annotations()[0]["type"] == "Stamp"
        assert reopened.annotations()[0]["text"] == ["批准 & <同意>"]


def test_styled_text_watermark_round_trip(tmp_path):
    path = tmp_path / "styled-watermark.ofd"
    with Document.new() as document:
        document.add_text_page("body", 80, 50)
        mark_id = document.add_text_watermark(
            0, "DRAFT", (10, 10, 50, 20), 5,
            color=(255, 0, 0), opacity=96, angle_degrees=-30,
            bold=True, italic=True,
        )
        assert document.annotations()[0]["id"] == str(mark_id)
        svg = document.page_svg(0)
        assert "#ff0000" in svg and 'fill-opacity="0.376' in svg
        with pytest.raises(OfdError, match="opacity"):
            document.add_text_watermark(0, "bad", (10, 10, 50, 20), opacity=256)
        assert len(document.annotations()) == 1
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.annotations()[0]["text"] == ["DRAFT"]
        assert reopened.page_svg(0) == svg


def test_text_watermark_to_all_pages_uses_each_page_box(tmp_path):
    path = tmp_path / "all-page-watermark.ofd"
    with Document.new() as document:
        document.add_blank_page(40, 30)
        document.add_blank_page(60, 50)
        ids = document.add_text_watermark_to_all_pages(
            "COPY", color=(0, 0, 255), opacity=128,
            tile_spacing=(20, 15),
        )
        assert len(ids) == 2
        assert [item["id"] for item in document.annotations()] == [str(id_) for id_ in ids]
        assert [item["appearance_box"] for item in document.annotations()] == [
            [0, 0, 40, 30], [0, 0, 60, 50],
        ]
        assert [len(item["text"]) for item in document.annotations()] == [4, 12]
        with pytest.raises(OfdError, match="watermark text"):
            document.add_text_watermark_to_all_pages("")
        with pytest.raises(OfdError, match="tile spacing"):
            document.add_text_watermark_to_all_pages(
                "bad", tile_spacing=(0, 15))
        assert len(document.annotations()) == 2
        document.save(path)
    with Document.open(path) as reopened:
        assert len(reopened.annotations()) == 2
        assert "#0000ff" in reopened.page_svg(1)


def test_image_watermark_from_bytes_and_existing_resource(tmp_path):
    path = tmp_path / "image-watermark.ofd"
    with Document.open(FIXTURES / "发票示例.ofd") as invoice:
        image = invoice.page_image_data(0, 0)
    with Document.new() as document:
        document.add_blank_page(40, 30)
        document.add_blank_page(40, 30)
        first = document.add_image_watermark(
            0, (5, 5, 20, 15), data=image, format="png", opacity=128)
        resource_id = document.document_multimedia_resources(0)[0]["id"]
        second = document.add_image_watermark(
            1, (5, 5, 20, 15), resource_id=resource_id)
        assert first != second
        assert [item["type"] for item in document.annotations()] == ["Watermark"] * 2
        assert "<image" in document.page_svg(0)
        with pytest.raises(OfdError):
            document.add_image_watermark(0, (5, 5, 20, 15), data=b"bad", format="png")
        assert len(document.annotations()) == 2
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert "<image" in reopened.page_svg(1)
        assert len(reopened.annotations()) == 2


def test_image_stamp_annotation_from_bytes_and_resource(tmp_path):
    path = tmp_path / "image-stamp.ofd"
    with Document.open(FIXTURES / "发票示例.ofd") as invoice:
        image = invoice.page_image_data(0, 0)
    with Document.new() as document:
        document.add_blank_page(40, 30)
        document.add_blank_page(40, 30)
        first = document.add_image_stamp_annotation(
            0, (5, 5, 20, 15), data=image, format="png", opacity=128)
        resource_id = document.document_multimedia_resources(0)[0]["id"]
        second = document.add_image_stamp_annotation(
            1, (5, 5, 20, 15), resource_id=resource_id)
        assert first != second
        assert [item["type"] for item in document.annotations()] == ["Stamp"] * 2
        assert "<image" in document.page_svg(0)
        before = document.annotations()
        with pytest.raises(OfdError):
            document.add_image_stamp_annotation(0, (5, 5, 20, 15), data=b"bad", format="png")
        assert document.annotations() == before
        document.save(path)
    with Document.open(path) as reopened:
        assert [item["type"] for item in reopened.annotations()] == ["Stamp"] * 2
        assert "<image" in reopened.page_svg(1)


def test_image_watermark_to_all_pages_reuses_resources_and_rolls_back():
    with Document.open(FIXTURES / "发票示例.ofd") as invoice:
        image = invoice.page_image_data(0, 0)
    with Document.new() as document:
        document.add_blank_page(40, 30)
        document.add_blank_page(40, 30)
        assert document.append_document_body() == 1
        document.append_blank_page(1, 40, 30)
        ids = document.add_image_watermark_to_all_pages(
            image, "png", (5, 5, 20, 15), opacity=128)
        assert len(ids) == 3
        assert len(set(ids)) == 3
        assert len(document.document_multimedia_resources(0)) == 1
        assert len(document.document_multimedia_resources(1)) == 1
        assert all("<image" in document.page_svg(index) for index in range(3))
        with pytest.raises(OfdError):
            document.add_image_watermark_to_all_pages(b"bad", "png", (5, 5, 20, 15))
        assert len(document.annotations()) == 3
        assert len(document.document_multimedia_resources(0)) == 1
        assert len(document.document_multimedia_resources(1)) == 1
        assert document.validate_references() == []


def test_reports_missing_annotation_font():
    with Document.new() as document:
        document.add_text_page("body", 30, 20)
        document.add_text_watermark(0, "mark", (1, 1, 10, 5), 4)
        source = document.annotations()[0]["source_path"]
        xml = document.entry(source).decode()
        font = ElementTree.fromstring(xml).find(".//{*}TextObject").attrib["Font"]
        document.set_entry(source, xml.replace(f'Font="{font}"', 'Font="999999"', 1).encode())
        assert any(
            issue["path"] == source and issue["code"] == "missing_font"
            for issue in document.validate_references()
        )


def test_adds_path_annotation_to_blank_page():
    with Document.new() as document:
        document.add_blank_page(30, 20)
        annotation_id = document.add_path_annotation(
            0, "M 0 0 L 10 5", (2, 3, 10, 5), stroke_color=(255, 0, 0))
        annotations = document.annotations()
        assert annotations[0]["id"] == str(annotation_id)
        assert annotations[0]["type"] == "Path"
        assert "#ff0000" in document.page_svg(0)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        assert document.validate_references() == []
        shown_pdf = document.to_pdf()
        document.set_annotation_print(0, annotation_id, False)
        assert "#ff0000" in document.page_svg(0)
        hidden_pdf = document.to_pdf()
        assert hidden_pdf != shown_pdf
        document.set_annotation_visible(0, annotation_id, False)
        assert "data-ofd-annotation" not in document.page_svg(0)
        assert document.to_pdf() == hidden_pdf
        document.set_annotation_visible(0, annotation_id, True)
        document.set_annotation_print(0, annotation_id, True)
        assert document.to_pdf() == shown_pdf
        for flag in ("NoZoom", "NoRotate", "ReadOnly"):
            document.set_annotation_flag(0, annotation_id, flag, True)
            assert document.annotations()[0]["attributes"][flag] == "true"
        document.set_annotation_flag(0, annotation_id, "ReadOnly", False)
        assert document.annotations()[0]["attributes"]["ReadOnly"] == "false"
        with pytest.raises(ValueError):
            document.set_annotation_flag(0, annotation_id, "Unknown", True)
        with pytest.raises(TypeError):
            document.set_annotation_flag(0, annotation_id, "NoZoom", 1)
        with pytest.raises(OfdError):
            document.add_path_annotation(0, "", (2, 3, 10, 5))


def test_adds_editable_text_note_annotation():
    with Document.new() as document:
        document.add_blank_page(30, 20)
        note_id = document.add_text_note_annotation(
            0, "审核 & <通过>", (2, 3, 5, 5),
            color=(0, 100, 200), opacity=128, icon="Comment", open=True,
        )
        note = document.annotations()[0]
        assert note["id"] == str(note_id)
        assert note["type"] == "Path"
        assert note["subtype"] == "Text"
        assert note["remark"] == "审核 & <通过>"
        assert note["parameters"]["PDFTextIcon"] == "Comment"
        assert note["parameters"]["PDFTextOpen"] == "true"
        assert document.to_pdf().startswith(b"%PDF")
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.add_text_note_annotation(0, "bad", (2, 3, 5, 5), icon="Unknown")
        assert document.to_bytes() == before


def test_adds_editable_square_and_circle_annotations():
    with Document.new() as document:
        document.add_blank_page(30, 20)
        square = document.add_shape_annotation(
            0, "Square", (2, 3, 10, 5), stroke_color=(0, 0, 255),
            fill_color=(255, 0, 0), line_width=0.5,
            dash_sequence=[1, 2], opacity=128, insets=(0.1, 0.1, 0.1, 0.1),
        )
        circle = document.add_shape_annotation(
            0, "Circle", (15, 3, 8, 8), stroke_color=None,
            fill_color=(0, 255, 0),
        )
        shapes = document.annotations()
        assert shapes[0]["id"] == str(square)
        assert (shapes[0]["type"], shapes[0]["subtype"]) == ("Path", "Square")
        assert shapes[0]["parameters"]["PDFShapeDash"] == "1 2"
        assert shapes[1]["id"] == str(circle)
        assert (shapes[1]["type"], shapes[1]["subtype"]) == ("Path", "Circle")
        assert "PDFShapeStroke" not in shapes[1]["parameters"]
        assert document.to_pdf().startswith(b"%PDF")
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert len(reopened.annotations()) == 2
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.add_shape_annotation(0, "Triangle", (2, 3, 10, 5))
        assert document.to_bytes() == before


def test_adds_editable_line_annotation():
    with Document.new() as document:
        document.add_blank_page(30, 20)
        line_id = document.add_line_annotation(
            0, (2, 3, 10, 5), (0, 5, 10, 0), stroke_color=(255, 0, 0),
            line_width=0.5, dash_sequence=[1, 2], opacity=128,
        )
        line = document.annotations()[0]
        assert line["id"] == str(line_id)
        assert line["subtype"] == "Line"
        assert line["parameters"]["PDFLine"] == "0 1 1 0"
        assert document.to_pdf().startswith(b"%PDF")
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.annotations()[0]["subtype"] == "Line"
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.add_line_annotation(0, (2, 3, 10, 5), (0, 5, 11, 0))
        assert document.to_bytes() == before


def test_adds_multi_path_annotation_atomically():
    paths = [
        {"commands": "M 0 0 L 10 10", "boundary": [0, 0, 10, 10],
         "stroke_color": [255, 0, 0]},
        {"commands": "M 0 10 L 10 0", "boundary": [10, 0, 10, 10],
         "stroke_color": [0, 0, 255], "dash_sequence": [2, 1]},
    ]
    with Document.new() as document:
        document.add_blank_page(40, 30)
        annotation_id = document.add_multi_path_annotation(0, (3, 4, 25, 15), paths)
        annotation = document.annotations()[0]
        assert annotation["id"] == str(annotation_id)
        assert annotation["type"] == "Path"
        assert "#ff0000" in document.page_svg(0)
        assert "#0000ff" in document.page_svg(0)
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert len(reopened.annotations()) == 1
            assert reopened.to_pdf().startswith(b"%PDF")
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.add_multi_path_annotation(0, (3, 4, 25, 15), paths + [
                {"commands": "bad", "boundary": [0, 0, 10, 10]},
            ])
        assert document.to_bytes() == before


def test_adds_mixed_annotation_with_image_and_text_atomically():
    with Document.new() as document:
        document.add_blank_page(40, 30)
        png = document.page_png(0, 25.4)
        image_id = document.add_multimedia_resource(0, "Image", "PNG", png)
        objects = [
            {"kind": "path", "commands": "M 0 0 L 9 9", "boundary": [0, 0, 10, 10],
             "stroke_color": [255, 0, 0]},
            {"kind": "text", "text": "Mixed", "boundary": [10, 0, 12, 10],
             "size": 4, "color": [0, 0, 255]},
            {"kind": "image", "resource_id": str(image_id),
             "boundary": [22, 0, 8, 8], "opacity": 200},
        ]
        annotation_id = document.add_mixed_annotation(0, (2, 3, 32, 12), objects)
        assert document.annotations()[0]["id"] == str(annotation_id)
        appearance_objects = document.annotation_objects(0, annotation_id)
        assert [item["kind"] for item in appearance_objects] == [
            "PathObject", "TextObject", "ImageObject"]
        assert appearance_objects[0]["commands"] == "M 0 0 L 9 9"
        assert appearance_objects[1]["text"] == ["Mixed"]
        assert appearance_objects[2]["resource_id"] == str(image_id)
        with Document.from_bytes(document.to_bytes()) as reopened:
            svg = reopened.page_svg(0)
            assert "Mixed" in svg and "#ff0000" in svg
            assert reopened.to_pdf().startswith(b"%PDF")
            assert reopened.validate_references() == []
            assert reopened.annotation_objects(0, annotation_id) == appearance_objects
        stamp_id = document.add_mixed_stamp_annotation(0, (2, 15, 32, 12), objects)
        assert document.annotations()[1]["id"] == str(stamp_id)
        assert document.annotations()[1]["type"] == "Stamp"
        assert len(document.annotation_objects(0, stamp_id)) == 3
        with pytest.raises(OfdError):
            document.annotation_objects(0, "missing")
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.add_mixed_annotation(0, (2, 3, 32, 12), objects + [
                {"kind": "image", "resource_id": "999999", "boundary": [0, 0, 4, 4]},
            ])
        assert document.to_bytes() == before
        path_id = appearance_objects[0]["id"]
        document.update_annotation_object(0, annotation_id, path_id, {
            "boundary": [1, 1, 9, 9], "ctm": [1, 0, 0, 1, 2, 3],
            "visible": False, "alpha": 128,
        })
        edited = document.annotation_objects(0, annotation_id)
        assert edited[0]["boundary"] == [1, 1, 9, 9]
        assert edited[0]["ctm"] == [1, 0, 0, 1, 2, 3]
        assert edited[0]["visible"] is False and edited[0]["alpha"] == 128
        document.update_annotation_object(0, annotation_id, path_id, {"ctm": None})
        assert document.annotation_objects(0, annotation_id)[0]["ctm"] is None
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.update_annotation_object(0, annotation_id, path_id,
                                              {"boundary": [0, 0, 0, 9]})
        assert document.to_bytes() == before
        document.remove_annotation_object(0, annotation_id, appearance_objects[2]["id"])
        assert len(document.annotation_objects(0, annotation_id)) == 2
        assert len(document.annotation_objects(0, stamp_id)) == 3


def test_appends_annotation_objects_across_round_trip_and_rolls_back_errors():
    with Document.new() as document:
        document.add_blank_page(40, 30)
        annotation_id = document.add_mixed_stamp_annotation(0, (1, 1, 20, 10), [
            {"kind": "text", "text": "first", "boundary": [0, 0, 8, 5], "size": 3},
        ])
        ids = document.append_annotation_objects(0, annotation_id, [
            {"kind": "path", "commands": "M 0 0 L 5 5", "boundary": [0, 0, 5, 5]},
            {"kind": "text", "text": "second", "boundary": [5, 0, 8, 5], "size": 3},
        ])
        assert len(ids) == 2
        assert [item["id"] for item in document.annotation_objects(0, annotation_id)][1:] == ids
        assert len(document.annotations()) == 1
        before = document.to_bytes()
        with Document.from_bytes(before) as reopened:
            assert [item["id"] for item in reopened.annotation_objects(0, annotation_id)][1:] == ids
            assert "second" in reopened.page_svg(0)
            assert reopened.validate_references() == []
        with pytest.raises(OfdError):
            document.append_annotation_objects(0, annotation_id, [
                {"kind": "image", "resource_id": "999999", "boundary": [0, 0, 5, 5]},
            ])
        with pytest.raises(OfdError):
            document.append_annotation_objects(0, annotation_id, [])
        with pytest.raises(IndexError):
            document.append_annotation_objects(-1, annotation_id, [
                {"kind": "text", "text": "x", "boundary": [0, 0, 5, 5], "size": 3},
            ])
        assert document.to_bytes() == before


def test_composite_graphics_in_annotation_round_trip_and_validate_resources():
    with Document.new() as document:
        document.add_blank_page(40, 30)
        document.add_composite_path_object(
            0, "M 0 0 L 8 0 L 8 8 C", (0, 0, 8, 8),
            fill=True, stroke=False, fill_color=(255, 0, 0))
        resource_id = document.page_composite_objects(0)[0]["resource_id"]
        graphic = {"kind": "composite", "resource_id": resource_id,
                   "boundary": [0, 0, 8, 8], "ctm": [1, 0, 0, 1, 1, 2], "alpha": 180}
        annotation_id = document.add_mixed_stamp_annotation(0, (2, 2, 20, 12), [graphic])
        ids = document.append_annotation_objects(0, annotation_id, [graphic])
        with Document.from_bytes(document.to_bytes()) as reopened:
            objects = reopened.annotation_objects(0, annotation_id)
            assert [item["kind"] for item in objects] == ["CompositeObject"] * 2
            assert objects[0]["resource_id"] == resource_id
            assert objects[0]["alpha"] == 180
            assert objects[1]["id"] == ids[0]
            assert "#ff0000" in reopened.page_svg(0)
            assert reopened.validate_references() == []
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.append_annotation_objects(0, annotation_id, [
                {"kind": "composite", "resource_id": "999999", "boundary": [0, 0, 8, 8]},
            ])
        assert document.to_bytes() == before


def test_edits_annotation_path_commands_and_preserves_failed_edits():
    with Document.new() as document:
        document.add_blank_page(40, 30)
        annotation_id = document.add_mixed_annotation(0, (2, 2, 20, 12), [
            {"kind": "path", "commands": "M 0 0 L 8 8", "boundary": [0, 0, 8, 8]},
            {"kind": "text", "text": "keep", "boundary": [8, 0, 8, 8], "size": 3},
        ])
        objects = document.annotation_objects(0, annotation_id)
        document.set_annotation_path_data(0, annotation_id, objects[0]["id"], "M 0 8 L 8 0")
        document.set_annotation_path_stroke_style(0, annotation_id, objects[0]["id"],
                                                  {"line_width": 1.25, "cap": "round",
                                                   "dash_sequence": [1, 2, 3], "dash_offset": 0.5})
        before = document.to_bytes()
        with Document.from_bytes(before) as reopened:
            edited = reopened.annotation_objects(0, annotation_id)
            assert edited[0]["commands"] == "M 0 8 L 8 0"
            assert edited[0]["boundary"] == objects[0]["boundary"]
            assert edited[1] == objects[1]
            assert reopened.page_svg(0)
        with pytest.raises(OfdError):
            document.set_annotation_path_data(0, annotation_id, objects[0]["id"], "invalid")
        with pytest.raises(OfdError):
            document.set_annotation_path_data(0, annotation_id, objects[1]["id"], "M 0 0 L 8 8")
        with pytest.raises(OfdError):
            document.set_annotation_path_stroke_style(0, annotation_id, objects[0]["id"],
                                                      {"line_width": -1})
        with pytest.raises(OfdError):
            document.set_annotation_path_stroke_style(0, annotation_id, objects[1]["id"],
                                                      {"line_width": 1})
        with pytest.raises(IndexError):
            document.set_annotation_path_data(-1, annotation_id, objects[0]["id"], "M 0 0 L 8 8")
        assert document.to_bytes() == before


def test_edits_annotation_text_code_and_remeasures_advances():
    with Document.new() as document:
        document.add_blank_page(40, 30)
        annotation_id = document.add_mixed_annotation(0, (2, 2, 25, 12), [
            {"kind": "text", "text": "AB", "boundary": [0, 0, 15, 8], "size": 3},
            {"kind": "path", "commands": "M 0 0 L 8 8", "boundary": [15, 0, 8, 8]},
        ])
        objects = document.annotation_objects(0, annotation_id)
        document.set_annotation_text_code(0, annotation_id, objects[0]["id"], 0, "A & <B>")
        before = document.to_bytes()
        with Document.from_bytes(before) as reopened:
            edited = reopened.annotation_objects(0, annotation_id)
            assert edited[0]["text"] == ["A & <B>"]
            assert edited[1] == objects[1]
            assert "A &amp; &lt;B&gt;" in reopened.page_svg(0)
        with pytest.raises(OfdError):
            document.set_annotation_text_code(0, annotation_id, objects[1]["id"], 0, "wrong")
        with pytest.raises(OfdError):
            document.set_annotation_text_code(0, annotation_id, objects[0]["id"], 1, "wrong")
        with pytest.raises(IndexError):
            document.set_annotation_text_code(0, annotation_id, objects[0]["id"], -1, "wrong")
        assert document.to_bytes() == before


def test_edits_annotation_text_font():
    font = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
            "tests" / "fixtures" / "dejavu-serif.ttf").read_bytes()
    with Document.new() as document:
        document.add_blank_page(40, 30)
        font_id = document.add_embedded_font("DejaVu Serif", font)
        annotation_id = document.add_mixed_annotation(0, (2, 2, 20, 10), [
            {"kind": "text", "text": "AB", "boundary": [0, 0, 15, 8], "size": 3},
        ])
        object_id = document.annotation_objects(0, annotation_id)[0]["id"]
        document.set_annotation_text_font(0, annotation_id, object_id, font_id)
        assert document.annotation_objects(0, annotation_id)[0]["font_id"] == str(font_id)
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.set_annotation_text_font(0, annotation_id, object_id, "999999")
        assert document.to_bytes() == before


def test_removes_annotation_text_code_and_preserves_position():
    with Document.new() as document:
        document.add_blank_page(40, 30)
        annotation_id = document.add_mixed_annotation(0, (2, 2, 20, 10), [
            {"kind": "text", "text": "AB", "boundary": [0, 0, 15, 8], "size": 3},
        ])
        object_id = document.annotation_objects(0, annotation_id)[0]["id"]
        path = document.annotations()[0]["source_path"]
        source = document.entry(path)
        modified = source.replace(b"</ofd:TextObject>",
                                  b"<ofd:TextCode>CD</ofd:TextCode></ofd:TextObject>", 1)
        assert modified != source
        document.set_entry(path, modified)
        document.remove_annotation_text_code(0, annotation_id, object_id, 0)
        assert document.annotation_objects(0, annotation_id)[0]["text"] == ["CD"]
        xml = ElementTree.fromstring(document.entry(path))
        code = next(node for node in xml.iter() if node.tag.endswith("TextCode"))
        assert code.attrib["X"] == "0"
        assert code.attrib["Y"] == "3"
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.remove_annotation_text_code(0, annotation_id, object_id, 0)
        assert document.to_bytes() == before


def test_appends_annotation_text_code_with_layout():
    with Document.new() as document:
        document.add_blank_page(40, 30)
        annotation_id = document.add_mixed_annotation(0, (2, 2, 20, 10), [
            {"kind": "text", "text": "AB", "boundary": [0, 0, 15, 8], "size": 3},
        ])
        object_id = document.annotation_objects(0, annotation_id)[0]["id"]
        document.append_annotation_text_code(0, annotation_id, object_id, "C&D",
                                             {"delta_x": [2, 2.5]})
        assert document.annotation_objects(0, annotation_id)[0]["text"] == ["AB", "C&D"]
        path = document.annotations()[0]["source_path"]
        xml = ElementTree.fromstring(document.entry(path))
        codes = [node for node in xml.iter() if node.tag.endswith("TextCode")]
        assert codes[1].attrib["DeltaX"] == "2 2.5"
        assert "X" not in codes[1].attrib
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.append_annotation_text_code(0, annotation_id, object_id, "XY",
                                                 {"delta_x": [1, 2]})
        assert document.to_bytes() == before


def test_annotation_text_edits_preserve_embedded_nul():
    with Document.new() as document:
        document.add_blank_page(40, 30)
        annotation_id = document.add_mixed_annotation(0, (2, 2, 20, 10), [
            {"kind": "text", "text": "AB", "boundary": [0, 0, 15, 8], "size": 3},
        ])
        object_id = document.annotation_objects(0, annotation_id)[0]["id"]
        document.set_annotation_text_code(0, annotation_id, object_id, 0, "A\0B")
        document.append_annotation_text_code(0, annotation_id, object_id, "C\0D")
        assert document.annotation_objects(0, annotation_id)[0]["text"] == ["A\0B", "C\0D"]
        path = document.annotations()[0]["source_path"]
        assert b"A\\0000B" in document.entry(path)
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.annotation_objects(0, annotation_id)[0]["text"] == ["A\0B", "C\0D"]


def test_edits_annotation_text_code_layout():
    with Document.new() as document:
        document.add_blank_page(40, 30)
        annotation_id = document.add_mixed_annotation(0, (2, 2, 20, 10), [
            {"kind": "text", "text": "AB", "boundary": [0, 0, 15, 8], "size": 3},
        ])
        object_id = document.annotation_objects(0, annotation_id)[0]["id"]
        document.append_annotation_text_code(0, annotation_id, object_id, "CD",
                                             {"x": 12, "y": 6})
        document.set_annotation_text_code_layout(0, annotation_id, object_id, 1,
                                                 {"delta_x": [2.5]})
        path = document.annotations()[0]["source_path"]
        tree = ElementTree.fromstring(document.entry(path))
        codes = [node for node in tree.iter() if node.tag.endswith("TextCode")]
        assert codes[1].attrib == {"DeltaX": "2.5"}
        info = document.annotation_objects(0, annotation_id)[0]
        assert info["codes"][1] == {"x": None, "y": None, "delta_x": "2.5",
                                    "delta_y": None, "text": "CD"}
        assert info["cg_transforms"] == []
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.set_annotation_text_code_layout(0, annotation_id, object_id, 0, {})
        assert document.to_bytes() == before


def test_edits_annotation_text_typography():
    with Document.new() as document:
        document.add_blank_page(40, 30)
        annotation_id = document.add_mixed_annotation(0, (2, 2, 20, 10), [
            {"kind": "text", "text": "AB", "boundary": [0, 0, 15, 8], "size": 3},
        ])
        object_id = document.annotation_objects(0, annotation_id)[0]["id"]
        document.set_annotation_text_typography(0, annotation_id, object_id, {
            "size": 4, "h_scale": 0.8, "read_direction": 180,
            "char_direction": 90, "weight": 700, "italic": True,
        })
        path = document.annotations()[0]["source_path"]
        tree = ElementTree.fromstring(document.entry(path))
        obj = next(node for node in tree.iter() if node.tag.endswith("TextObject"))
        assert obj.attrib["Size"] == "4"
        assert obj.attrib["Weight"] == "700"
        assert obj.attrib["Italic"] == "true"
        info = document.annotation_objects(0, annotation_id)[0]
        assert info["h_scale"] == 0.8
        assert info["read_direction"] == 180
        assert info["char_direction"] == 90
        assert info["weight"] == 700
        assert info["italic"] is True
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.set_annotation_text_typography(0, annotation_id, object_id,
                                                   {"size": 4, "weight": 750})
        assert document.to_bytes() == before


def test_edits_annotation_text_paint_flags():
    with Document.new() as document:
        document.add_blank_page(40, 30)
        annotation_id = document.add_mixed_annotation(0, (2, 2, 20, 10), [
            {"kind": "text", "text": "AB", "boundary": [0, 0, 15, 8], "size": 3},
            {"kind": "path", "commands": "M 0 0 L 8 8", "boundary": [15, 0, 5, 8]},
        ])
        text_id, path_id = [item["id"] for item in document.annotation_objects(0, annotation_id)]
        initial = document.annotation_objects(0, annotation_id)
        assert (initial[0]["fill"], initial[0]["stroke"]) == (True, False)
        assert (initial[1]["fill"], initial[1]["stroke"]) == (False, True)
        path = document.annotations()[0]["source_path"]
        document.set_annotation_text_paint_flags(0, annotation_id, text_id,
                                                 fill=False, stroke=True)
        root = ElementTree.fromstring(document.entry(path))
        obj = next(node for node in root.iter() if node.tag.endswith("TextObject"))
        assert obj.attrib["Fill"] == "false"
        assert obj.attrib["Stroke"] == "true"
        edited = document.annotation_objects(0, annotation_id)[0]
        assert (edited["fill"], edited["stroke"]) == (False, True)
        before = document.to_bytes()
        with Document.from_bytes(before) as reopened:
            edited = reopened.annotation_objects(0, annotation_id)[0]
            assert (edited["fill"], edited["stroke"]) == (False, True)
        with pytest.raises(OfdError):
            document.set_annotation_text_paint_flags(0, annotation_id, path_id, fill=True)
        assert document.to_bytes() == before
        document.set_annotation_text_paint_flags(0, annotation_id, text_id)
        root = ElementTree.fromstring(document.entry(path))
        obj = next(node for node in root.iter() if node.tag.endswith("TextObject"))
        assert "Fill" not in obj.attrib and "Stroke" not in obj.attrib
        reset = document.annotation_objects(0, annotation_id)[0]
        assert (reset["fill"], reset["stroke"]) == (True, False)


def test_edits_annotation_object_name_and_blend_mode():
    with Document.new() as document:
        document.add_blank_page(40, 30)
        annotation_id = document.add_mixed_annotation(0, (2, 2, 20, 10), [
            {"kind": "text", "text": "AB", "boundary": [0, 0, 15, 8], "size": 3},
        ])
        object_id = document.annotation_objects(0, annotation_id)[0]["id"]
        document.set_annotation_object_name(0, annotation_id, object_id, '标题 & "注释"')
        document.set_annotation_object_blend_mode(0, annotation_id, object_id, "multiply")
        draw_param = document.add_document_draw_param(0, {"line_width": 1.25})
        document.set_annotation_object_draw_param(0, annotation_id, object_id, draw_param)
        info = document.annotation_objects(0, annotation_id)[0]
        assert info["name"] == '标题 & "注释"'
        assert info["blend_mode"] == "Multiply"
        assert info["draw_param"] == str(draw_param)
        assert "mix-blend-mode:multiply" in document.page_svg(0)
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.set_annotation_object_blend_mode(0, annotation_id, object_id, "unknown")
        with pytest.raises(OfdError):
            document.set_annotation_object_draw_param(0, annotation_id, object_id, "999999")
        assert document.to_bytes() == before
        with Document.from_bytes(before) as reopened:
            assert reopened.annotation_objects(0, annotation_id)[0]["name"] == '标题 & "注释"'
        document.set_annotation_object_name(0, annotation_id, object_id, None)
        document.set_annotation_object_blend_mode(0, annotation_id, object_id, None)
        document.set_annotation_object_draw_param(0, annotation_id, object_id, None)
        info = document.annotation_objects(0, annotation_id)[0]
        assert info["name"] is None and info["blend_mode"] is None
        assert info["draw_param"] is None


def test_edits_annotation_image_resource_atomically():
    with Document.new() as document:
        document.add_blank_page(40, 30)
        png = document.page_png(0, 25.4)
        first = document.add_multimedia_resource(0, "Image", "PNG", png)
        second = document.add_multimedia_resource(0, "Image", "PNG", png)
        annotation_id = document.add_mixed_annotation(0, (2, 2, 20, 12), [
            {"kind": "image", "resource_id": str(first), "boundary": [0, 0, 8, 8]},
            {"kind": "path", "commands": "M 0 0 L 8 8", "boundary": [8, 0, 8, 8]},
        ])
        objects = document.annotation_objects(0, annotation_id)
        document.set_annotation_graphic_resource(0, annotation_id, objects[0]["id"], second)
        before = document.to_bytes()
        with Document.from_bytes(before) as reopened:
            edited = reopened.annotation_objects(0, annotation_id)
            assert edited[0]["resource_id"] == str(second)
            assert edited[0]["boundary"] == objects[0]["boundary"]
            assert edited[1] == objects[1]
            assert reopened.validate_references() == []
        with pytest.raises(OfdError):
            document.set_annotation_graphic_resource(0, annotation_id, objects[0]["id"], "999999")
        with pytest.raises(OfdError):
            document.set_annotation_graphic_resource(0, annotation_id, objects[1]["id"], second)
        assert document.to_bytes() == before


def test_adds_highlight_annotation_to_blank_page(tmp_path):
    path = tmp_path / "highlight.ofd"
    with Document.new() as document:
        document.add_blank_page(30, 20)
        annotation_id = document.add_highlight_annotation(
            0, (2, 3, 10, 5), (255, 220, 0), 96)
        assert document.annotations()[0]["id"] == str(annotation_id)
        assert document.annotations()[0]["type"] == "Highlight"
        assert document.annotations()[0]["last_mod_date"] is not None
        document.set_annotation_creator(0, annotation_id, "张三 & Team")
        document.set_annotation_subtype(0, annotation_id, "审阅 & <校对>")
        document.set_annotation_last_mod_date(0, annotation_id, "2024-02-29")
        with pytest.raises(OfdError):
            document.set_annotation_last_mod_date(0, annotation_id, "2023-02-29")
        document.set_annotation_remark(0, annotation_id, "审核 & 通过")
        document.set_annotation_parameter(0, annotation_id, "分类 & <键>", "甲 & <乙>")
        document.set_annotation_parameter(0, annotation_id, "状态", "草稿")
        assert document.annotations()[0]["attributes"]["Creator"] == "张三 & Team"
        assert document.annotations()[0]["subtype"] == "审阅 & <校对>"
        assert document.annotations()[0]["last_mod_date"] == "2024-02-29"
        assert document.annotations()[0]["remark"] == "审核 & 通过"
        assert document.annotations()[0]["parameters"]["分类 & <键>"] == "甲 & <乙>"
        svg = document.page_svg(0)
        assert "#ffdc00" in svg
        assert 'fill-opacity="0.3764705882352941"' in svg
        assert document.to_pdf().startswith(b"%PDF")
        document.set_annotation_appearance_boundary(0, annotation_id, (12, 8, 10, 5))
        assert document.annotations()[0]["appearance_box"] == [12.0, 8.0, 10.0, 5.0]
        with pytest.raises(OfdError):
            document.set_annotation_appearance_boundary(0, annotation_id, (12, 8, 0, 5))
        with pytest.raises(OfdError):
            document.add_highlight_annotation(0, (2, 3, 0, 5))
        assert len(document.annotations()) == 1
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.annotations()[0]["type"] == "Highlight"
        assert reopened.annotations()[0]["subtype"] == "审阅 & <校对>"
        assert reopened.annotations()[0]["last_mod_date"] == "2024-02-29"
        assert reopened.annotations()[0]["appearance_box"] == [12.0, 8.0, 10.0, 5.0]
        assert reopened.annotations()[0]["remark"] == "审核 & 通过"
        assert reopened.annotations()[0]["parameters"]["状态"] == "草稿"
        reopened.set_annotation_parameter(0, annotation_id, "状态", "通过")
        reopened.remove_annotation_parameter(0, annotation_id, "分类 & <键>")
        parameters = reopened.annotations()[0]["parameters"]
        assert parameters["状态"] == "通过"
        assert "分类 & <键>" not in parameters
        assert parameters["PDFMarkupQuads"] == "0 0 1 0 1 1 0 1"
        reopened.set_annotation_remark(0, annotation_id, None)
        assert reopened.annotations()[0]["remark"] is None
        reopened.set_annotation_subtype(0, annotation_id, None)
        assert reopened.annotations()[0]["subtype"] is None


def test_adds_uri_link_annotation_to_blank_page():
    with Document.new() as document:
        document.add_blank_page(30, 20)
        before = document.page_png(0, 25.4)
        uri = "https://example.com/?a=1&b=2"
        annotation_id = document.add_uri_link_annotation(0, uri, (2, 3, 10, 5))
        annotations = document.annotations()
        assert len(annotations) == 1
        assert annotations[0]["id"] == str(annotation_id)
        assert annotations[0]["type"] == "Link"
        assert annotations[0]["actions"][0]["kind"] == "URI"
        assert annotations[0]["actions"][0]["attributes"]["URI"] == uri
        assert document.page_png(0, 25.4) == before
        assert b"/Subtype /Link" in document.to_pdf()
        assert document.validate_references() == []
        with pytest.raises(OfdError):
            document.add_uri_link_annotation(0, "", (2, 3, 10, 5))


def test_writes_quadrilateral_link_regions():
    with Document.new() as document:
        document.add_blank_page(30, 20)
        document.add_blank_page(30, 20)
        quad = ((3, 4), (6, 4), (7, 6), (4, 6))
        invalid = ((3, 4), (7, 6), (6, 4), (4, 6))
        with pytest.raises(OfdError):
            document.add_uri_link_annotation(0, "https://example.org", (2, 3, 10, 5), quad_points=[invalid])
        assert document.annotations() == []
        document.add_uri_link_annotation(0, "https://example.org", (2, 3, 10, 5), quad_points=[quad])
        document.add_goto_link_annotation(0, document.pages()[1]["id"], (2, 3, 10, 5), kind="Fit", quad_points=[quad])
        annotations = document.annotations()
        assert len(annotations) == 2
        assert annotations[0]["actions"][0]["region"]["areas"][0]["start"] == [3, 4]
        assert annotations[1]["actions"][0]["region"]["areas"][0]["start"] == [3, 4]
        assert document.to_pdf().count(b"/QuadPoints") == 2


def test_adds_goto_and_bookmark_link_annotations():
    with Document.new() as document:
        document.add_blank_page(30, 20)
        document.add_blank_page(40, 25)
        target = document.pages()[1]["id"]
        document.add_bookmark("chapter & 2", target, kind="Fit")
        goto_id = document.add_goto_link_annotation(
            0, target, (2, 3, 10, 5), left=4, top=6)
        bookmark_id = document.add_bookmark_link_annotation(
            0, "chapter & 2", (13, 3, 10, 5))
        annotations = document.annotations()
        assert [item["id"] for item in annotations] == [str(goto_id), str(bookmark_id)]
        assert annotations[0]["actions"][0]["destination"]["page_id"] == target
        assert annotations[1]["actions"][0]["bookmark"] == "chapter & 2"
        assert document.to_pdf().count(b"/Subtype /Link") == 2
        assert document.validate_references() == []
        with pytest.raises(OfdError):
            document.add_bookmark_link_annotation(0, "missing", (1, 1, 2, 2))
        with pytest.raises(OfdError):
            document.add_goto_link_annotation(0, "missing", (1, 1, 2, 2))


def test_numeric_page_id_aliases_for_navigation_and_annotation_records():
    with Document.new() as document:
        document.add_blank_page(40, 30)
        document.add_blank_page(40, 30)
        pages = document.pages()
        target_alias = f'0{pages[1]["id"]}'
        document.add_bookmark("alias", target_alias, kind="Fit")
        document.add_outline("alias", destination={"page_id": target_alias, "kind": "Fit"})
        document.add_goto_link_annotation(0, target_alias, (2, 2, 8, 8), kind="Fit")
        assert document.to_pdf().startswith(b"%PDF-")

        document.add_text_note_annotation(0, "first", (12, 2, 8, 8))
        list_path = "Doc_0/Annots/Annotations.xml"
        page_id = pages[0]["id"]
        source = document.entry(list_path)
        aliased = source.replace(
            f'PageID="{page_id}"'.encode(), f'PageID="0{page_id}"'.encode(), 1,
        )
        assert aliased != source
        document.set_entry(list_path, aliased)
        document.add_text_note_annotation(0, "second", (22, 2, 8, 8))
        assert len(document.annotations()) == 3
        current = document.entry(list_path)
        assert current.count(b"<ofd:Page PageID=") == 1
        assert document.validate_references() == []

        duplicate = (
            f'<ofd:Page PageID="{page_id}">'
            '<ofd:FileLoc>Page_0/Annotation.xml</ofd:FileLoc></ofd:Page>'
        ).encode()
        document.set_entry(
            list_path, current.replace(b"</ofd:Annotations>", duplicate + b"</ofd:Annotations>"),
        )
        assert any(
            issue["code"] == "duplicate_annotation_page_id"
            for issue in document.validate_references()
        )


def test_adds_attachment_actions_and_validates_targets(tmp_path):
    path = tmp_path / "attachment-action.ofd"
    with Document.new() as document:
        document.add_blank_page(40, 30)
        document.add_attachment("note.txt", b"hello")
        attachment_id = document.attachments()[0]["id"]
        object_id = document.add_path_object(0, "M 0 0 L 5 0 L 5 5 C", (1, 1, 5, 5))
        document.add_page_object_attachment_action(0, object_id, attachment_id, new_window=True)
        annotation_id = document.add_attachment_link_annotation(
            0, attachment_id, (10, 1, 5, 5), new_window=False)
        assert document.page_object_actions(0, object_id)[0]["attributes"] == {
            "AttachID": attachment_id, "NewWindow": "true"}
        annotation = next(item for item in document.annotations() if item["id"] == str(annotation_id))
        assert annotation["actions"][0]["attributes"]["NewWindow"] == "false"
        assert document.validate_references() == []
        with pytest.raises(OfdError):
            document.add_page_object_attachment_action(0, object_id, "missing")
        with pytest.raises(OfdError):
            document.add_attachment_link_annotation(0, "missing", (1, 1, 2, 2))
        with pytest.raises(TypeError):
            document.add_page_object_attachment_action(0, object_id, attachment_id, new_window=1)
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_object_actions(0, object_id)[0]["kind"] == "GotoA"
        assert reopened.validate_references() == []


def test_adds_audio_video_resources_and_playback_actions(tmp_path):
    path = tmp_path / "media-actions.ofd"
    with Document.new() as document:
        document.add_blank_page(50, 40)
        audio_id = document.add_multimedia_resource(0, "Audio", "MP3", b"audio bytes")
        video_id = document.add_multimedia_resource(0, "Video", "MP4", b"video bytes")
        resources = document.page_multimedia_resources(0)
        assert document.document_multimedia_resources(0) == resources
        assert document.document_multimedia_data(0, audio_id) == b"audio bytes"
        with pytest.raises(OfdError):
            document.document_multimedia_data(0, "missing")
        with pytest.raises(IndexError):
            document.document_multimedia_resources(-1)
        assert [(item["id"], item["media_type"], item["format"], item["size"])
                for item in resources] == [(str(audio_id), "Audio", "MP3", 11),
                                          (str(video_id), "Video", "MP4", 11)]
        assert document.page_multimedia_data(0, audio_id) == b"audio bytes"
        assert document.page_multimedia_data(0, video_id) == b"video bytes"
        with pytest.raises(OfdError):
            document.page_multimedia_data(0, "missing")
        with pytest.raises(IndexError):
            document.page_multimedia_resources(-1)
        object_id = document.add_path_object(0, "M 0 0 L 5 0 L 5 5 C", (1, 1, 5, 5))
        document.add_page_object_sound_action(0, object_id, audio_id,
                                              volume=75, repeat=True, synchronous=False)
        document.add_page_object_movie_action(0, object_id, video_id, operator="Pause")
        document.add_sound_link_annotation(0, audio_id, (10, 1, 5, 5), volume=50)
        document.add_movie_link_annotation(0, video_id, (20, 1, 5, 5))
        actions = document.page_object_actions(0, object_id)
        assert [action["kind"] for action in actions] == ["Sound", "Movie"]
        assert actions[0]["attributes"]["Volume"] == "75"
        assert actions[1]["attributes"]["Operator"] == "Pause"
        assert document.validate_references() == []
        with pytest.raises(OfdError):
            document.remove_document_multimedia_resource(0, audio_id)
        with pytest.raises(OfdError):
            document.add_page_object_sound_action(0, object_id, video_id)
        with pytest.raises(OfdError):
            document.add_page_object_sound_action(0, object_id, audio_id, volume=101)
        with pytest.raises(OfdError):
            document.add_page_object_movie_action(0, object_id, audio_id)
        with pytest.raises(OfdError):
            document.add_movie_link_annotation(0, video_id, (1, 1, 2, 2), operator="Bad")
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_multimedia_resources(0) == resources
        assert reopened.document_multimedia_resources(0) == resources
        assert reopened.page_multimedia_data(0, audio_id) == b"audio bytes"
        assert [action["kind"] for action in reopened.page_object_actions(0, object_id)] == ["Sound", "Movie"]
        assert reopened.validate_references() == []


def test_adds_standalone_image_multimedia_resource(tmp_path):
    path = tmp_path / "image-resource.ofd"
    with Document.new() as document:
        document.add_blank_page(10, 10)
        png = document.page_png(0, 25.4)
        resource_id = document.add_multimedia_resource(0, "Image", "pNg", png)
        resources = document.document_multimedia_resources(0)
        assert len(resources) == 1
        assert resources[0]["id"] == str(resource_id)
        assert resources[0]["media_type"] == "Image"
        assert resources[0]["format"] == "PNG"
        assert document.page_multimedia_data(0, resource_id) == png
        with pytest.raises(OfdError):
            document.add_multimedia_resource(0, "Image", "PNG", b"bad")
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.document_multimedia_resources(0) == resources
        assert reopened.document_multimedia_data(0, resource_id) == png


def test_replaces_document_multimedia_data(tmp_path):
    path = tmp_path / "replaced-media.ofd"
    with Document.new() as document:
        document.add_blank_page(10, 10)
        resource_id = document.add_multimedia_resource(0, "Audio", "MP3", b"original")
        old_path = document.document_multimedia_resources(0)[0]["path"]
        document.replace_document_multimedia_data(0, resource_id, b"updated")
        assert document.document_multimedia_data(0, resource_id) == b"updated"
        assert document.entry(old_path) == b"original"
        assert document.document_multimedia_resources(0)[0]["id"] == str(resource_id)
        with pytest.raises(OfdError):
            document.replace_document_multimedia_data(0, resource_id, b"")
        with pytest.raises(IndexError):
            document.replace_document_multimedia_data(-1, resource_id, b"bad")
        document.save(path)
    with Document.open(path) as reopened:
        reopened.replace_document_multimedia_data(0, resource_id, b"again")
        assert reopened.document_multimedia_data(0, resource_id) == b"again"
        reopened.replace_page_multimedia_data(0, resource_id, b"page update")
        assert reopened.page_multimedia_data(0, resource_id) == b"page update"
        assert reopened.document_multimedia_data(0, resource_id) == b"page update"
        with pytest.raises(OfdError):
            reopened.replace_page_multimedia_data(0, "missing", b"bad")
        with pytest.raises(IndexError):
            reopened.replace_page_multimedia_data(-1, resource_id, b"bad")
        old_path = reopened.document_multimedia_resources(0)[0]["path"]
        reopened.remove_document_multimedia_resource(0, resource_id)
        assert reopened.document_multimedia_resources(0) == []
        assert reopened.entry(old_path) == b"page update"
        with pytest.raises(OfdError):
            reopened.document_multimedia_data(0, resource_id)
        with pytest.raises(IndexError):
            reopened.remove_document_multimedia_resource(-1, resource_id)


def test_adds_page_local_multimedia_resource(tmp_path):
    path = tmp_path / "page-media.ofd"
    with Document.new() as document:
        document.add_blank_page(10, 10)
        document.add_blank_page(10, 10)
        resource_id = document.add_page_multimedia_resource(0, "Audio", "WAV", b"page audio")
        assert document.document_multimedia_resources(0) == []
        assert document.page_multimedia_resources(0)[0]["id"] == str(resource_id)
        assert document.page_multimedia_data(0, resource_id) == b"page audio"
        assert document.page_multimedia_resources(1) == []
        with pytest.raises(OfdError):
            document.page_multimedia_data(1, resource_id)
        with pytest.raises(OfdError):
            document.add_page_multimedia_resource(1, "Image", "PNG", b"bad")
        with pytest.raises(IndexError):
            document.add_page_multimedia_resource(-1, "Audio", "WAV", b"bad")
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_multimedia_data(0, resource_id) == b"page audio"
        assert reopened.page_multimedia_resources(1) == []
        media_path = reopened.page_multimedia_resources(0)[0]["path"]
        with pytest.raises(IndexError):
            reopened.remove_page_multimedia_resource(-1, resource_id)
        with pytest.raises(OfdError):
            reopened.remove_page_multimedia_resource(1, resource_id)
        reopened.remove_page_multimedia_resource(0, resource_id)
        assert reopened.page_multimedia_resources(0) == []
        assert reopened.entry(media_path) == b"page audio"
        with pytest.raises(OfdError):
            reopened.page_multimedia_data(0, resource_id)


def test_replacing_shared_page_multimedia_isolated_by_copy(tmp_path):
    path = tmp_path / "shared-page-media.ofd"
    with Document.new() as document:
        document.add_blank_page(10, 10)
        document.add_blank_page(10, 10)
        resource_id = document.add_page_multimedia_resource(0, "Audio", "WAV", b"original")
        pages = document.pages()
        first_dir = pages[0]["path"].rsplit("/", 1)[0]
        second_path = pages[1]["path"]
        resource_name = f"RustPageRes_{resource_id}.xml"
        relative = f"../{first_dir.rsplit('/', 1)[1]}/{resource_name}"
        second_xml = document.entry(second_path).decode()
        document.set_entry(
            second_path,
            second_xml.replace(
                "<ofd:Content>",
                f"<ofd:PageRes>{relative}</ofd:PageRes><ofd:Content>",
            ).encode(),
        )
        assert document.page_multimedia_data(1, resource_id) == b"original"
        document.replace_page_multimedia_data(0, resource_id, b"first only")
        assert document.page_multimedia_data(0, resource_id) == b"first only"
        assert document.page_multimedia_data(1, resource_id) == b"original"
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_multimedia_data(0, resource_id) == b"first only"
        assert reopened.page_multimedia_data(1, resource_id) == b"original"


def test_removes_annotation_without_removing_other_marks(tmp_path):
    path = tmp_path / "removed-annotation.ofd"
    with Document.new() as document:
        document.add_text_page("body", 210, 297)
        first = document.add_text_watermark(0, "first", (10, 20, 40, 8), 4)
        second = document.add_text_watermark(0, "second", (20, 30, 40, 8), 4)
        with pytest.raises(IndexError):
            document.remove_annotation(-1, first)
        with pytest.raises(OfdError):
            document.remove_annotation(0, "missing")
        document.remove_annotation(0, first)
        assert [item["id"] for item in document.annotations()] == [str(second)]
        with pytest.raises(OfdError):
            document.remove_annotation(0, first)
        document.save(path)
    with Document.open(path) as reopened:
        assert [item["text"] for item in reopened.annotations()] == [["second"]]
        assert reopened.validate_references() == []


def test_adds_uri_actions_to_page_object(tmp_path):
    path = tmp_path / "uri-action.ofd"
    with Document.new() as document:
        document.add_text_page("link", 100, 60)
        object_id = document.page_objects(0)[0]["id"]
        assert document.page_object_actions(0, object_id) == []
        with pytest.raises(IndexError):
            document.add_page_object_uri_action(-1, object_id, "https://example.org")
        with pytest.raises(IndexError):
            document.page_object_actions(-1, object_id)
        with pytest.raises(OfdError):
            document.add_page_object_uri_action(0, object_id, "")
        document.add_page_object_uri_action(0, object_id, "https://example.org/?a=1&b=2")
        document.add_page_object_uri_action(0, object_id, "mailto:test@example.org")
        actions = document.page_object_actions(0, object_id)
        assert [item["kind"] for item in actions] == ["URI", "URI"]
        assert actions[0]["attributes"]["URI"] == "https://example.org/?a=1&b=2"
        assert document.to_pdf().count(b"/Subtype /Link") == 1
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_object_actions(0, object_id) == actions
        assert reopened.validate_references() == []


def test_action_regions_roundtrip_and_pdf_links(tmp_path):
    path = tmp_path / "regions.ofd"
    with Document.new() as document:
        document.add_text_page("link", 100, 60)
        object_id = document.page_objects(0)[0]["id"]
        document.add_page_object_uri_action(0, object_id, "https://example.org")
        with pytest.raises(IndexError):
            document.set_page_object_action_regions(0, object_id, -1, [])
        with pytest.raises(OfdError):
            document.set_page_object_action_regions(0, object_id, 0, [(2, 3, -1, 5)])
        document.set_page_object_action_regions(0, object_id, 0, [(2, 3, 4, 5), (10, 12, 3, 2)])
        action = document.page_object_actions(0, object_id)[0]
        assert len(action["region"]["areas"]) == 2
        assert action["region"]["areas"][0]["start"] == [2, 3]
        assert document.to_pdf().count(b"/Subtype /Link") == 1
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_object_actions(0, object_id)[0] == action
        reopened.set_page_object_action_regions(0, object_id, 0, [])
        assert reopened.page_object_actions(0, object_id)[0]["region"] is None


def test_adds_goto_action_with_page_destination(tmp_path):
    path = tmp_path / "goto-action.ofd"
    with Document.new() as document:
        document.add_text_page("jump", 100, 60)
        document.add_blank_page(100, 60)
        object_id = document.page_objects(0)[0]["id"]
        page_id = document.pages()[1]["id"]
        with pytest.raises(IndexError):
            document.add_page_object_goto_action(-1, object_id, page_id)
        with pytest.raises(ValueError):
            document.add_page_object_goto_action(0, object_id, page_id, left=float("nan"))
        with pytest.raises(OfdError):
            document.add_page_object_goto_action(0, object_id, "missing")
        document.add_page_object_goto_action(0, object_id, page_id, left=12.5, top=21, zoom=1.5)
        action = document.page_object_actions(0, object_id)[0]
        assert action["kind"] == "Goto"
        assert action["destination"]["page_id"] == str(page_id)
        assert action["destination"]["left"] == 12.5
        assert document.to_pdf().count(b"/Subtype /Link") == 1
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_object_actions(0, object_id)[0] == action
        assert reopened.validate_references() == []


def test_adds_bookmark_action_and_removes_selected_action(tmp_path):
    path = tmp_path / "bookmark-action.ofd"
    with Document.new() as document:
        document.add_text_page("click", 100, 60)
        object_id = document.page_objects(0)[0]["id"]
        document.add_bookmark("Section & One", document.pages()[0]["id"], left=10, top=20)
        with pytest.raises(OfdError):
            document.add_page_object_bookmark_action(0, object_id, "missing")
        with pytest.raises(IndexError):
            document.add_page_object_bookmark_action(-1, object_id, "Section & One")
        document.add_page_object_uri_action(0, object_id, "https://example.org")
        document.add_page_object_bookmark_action(0, object_id, "Section & One")
        assert document.page_object_actions(0, object_id)[1]["bookmark"] == "Section & One"
        with pytest.raises(IndexError):
            document.remove_page_object_action(0, object_id, -1)
        with pytest.raises(OfdError):
            document.remove_page_object_action(0, object_id, 9)
        document.remove_page_object_action(0, object_id, 0)
        assert [a["kind"] for a in document.page_object_actions(0, object_id)] == ["Goto"]
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_object_actions(0, object_id)[0]["bookmark"] == "Section & One"
        assert reopened.validate_references() == []


def test_attachment_creation(tmp_path):
    path = tmp_path / "attachment.ofd"
    with Document.new() as document:
        document.add_attachment("note.txt", b"hello")
        old_path = document.attachments()[0]["path"]
        with pytest.raises(IndexError):
            document.remove_attachment(-1)
        document.remove_attachment(0)
        assert document.entry(old_path) == b"hello"
        assert document.attachments() == []
        document.add_attachment("note.txt", b"hello")
        assert document.attachments()[0]["path"] != old_path
        document.set_attachment_metadata(
            0, format="text/plain", creation_date="2024-02-29T12:30:59",
            mod_date="2024-03-01T08:00:00", visible=False,
            usage="source & <record>")
        with pytest.raises(OfdError):
            document.set_attachment_metadata(0, creation_date="2023-02-29T12:30:59")
        replaced_path = document.attachments()[0]["path"]
        document.replace_attachment_data(0, b"updated")
        assert document.attachment_data(0) == b"updated"
        assert document.entry(replaced_path) == b"hello"
        with pytest.raises(OfdError):
            document.replace_attachment_data(2, b"bad")
        with pytest.raises(OfdError):
            document.rename_attachment(0, " ")
        document.rename_attachment(0, "新名称 & <附件>.txt")
        document.add_attachment("second.txt", b"two")
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.attachments()[0]["name"] == "新名称 & <附件>.txt"
        assert reopened.attachment_data(0) == b"updated"
        assert reopened.attachments()[0]["format"] == "text/plain"
        assert reopened.attachments()[0]["creation_date"] == "2024-02-29T12:30:59"
        assert reopened.attachments()[0]["mod_date"] == "2024-03-01T08:00:00"
        assert reopened.attachments()[0]["visible"] is False
        assert reopened.attachments()[0]["usage"] == "source & <record>"
        assert len(reopened.attachments()) == 2


def test_adds_attachment_to_existing_document(tmp_path):
    path = tmp_path / "existing-attachment.ofd"
    with Document.open(FIXTURES / "AddAttachment.ofd") as document:
        with pytest.raises(IndexError):
            document.add_attachment_to_document(-1, "note", b"data")
        with pytest.raises(OfdError, match="attachment name"):
            document.add_attachment_to_document(0, " ", b"data")
        document.add_attachment_to_document(0, "Rust & note", b"new data")
        assert [item["name"] for item in document.attachments()] == ["Gao", "Rust & note"]
        assert document.attachment_data(1) == b"new data"
        document.remove_attachment(1)
        assert [item["name"] for item in document.attachments()] == ["Gao"]
        document.add_attachment_to_document(0, "Rust & note", b"new data")
        document.save(path)
    with Document.open(path) as document:
        assert document.attachment_data(1) == b"new data"
        assert document.validate_references() == []


def test_attachment_references_use_unique_archive_case():
    with Document.open(FIXTURES / "AddAttachment.ofd") as document:
        root = "Doc_0/Document.xml"
        document.set_entry(
            root,
            document.entry(root).replace(b"Attachments.xml", b"ATTACHMENTS.XML"),
        )
        list_path = "Doc_0/Attachments.xml"
        document.set_entry(
            list_path,
            document.entry(list_path).replace(b"/Doc_0/Res/eg_tulip.jpg", b"/DOC_0/RES/EG_TULIP.JPG"),
        )
        assert document.attachments()[0]["path"] == "Doc_0/Res/eg_tulip.jpg"
        assert len(document.attachment_data(0)) == 36_494
        document.rename_attachment(0, "renamed")
        assert document.attachments()[0]["name"] == "renamed"
        assert document.validate_references() == []


def test_icc_profile_converts_page_color():
    profile = (
        Path(__file__).resolve().parents[3]
        / "crates/ofdrw/tests/fixtures/swapped-rg.icc"
    ).read_bytes()
    with Document.new() as document:
        document.add_path_page(
            "M 0 0 L 20 0 L 20 20 L 0 20 C", (0, 0, 20, 20), 20, 20, fill=True, stroke=False
        )
        resource_path = "Doc_0/PublicRes.xml"
        resources = document.entry(resource_path)
        color_space = (
            b'<ofd:ColorSpaces><ofd:ColorSpace ID="100" Type="RGB" '
            b'Profile="SWAPPED-RG.ICC"/></ofd:ColorSpaces>'
        )
        document.set_entry(
            resource_path, resources.replace(b"</ofd:Res>", color_space + b"</ofd:Res>")
        )
        document.set_entry("Doc_0/Res/swapped-rg.icc", profile)
        page_path = document.pages()[0]["path"]
        page = document.entry(page_path)
        document.set_entry(
            page_path,
            page.replace(
                b"</ofd:PathObject>",
                b'<ofd:FillColor ColorSpace="100" Value="255 0 0"/></ofd:PathObject>',
            ),
        )
        assert 'fill="#00ff00"' in document.page_svg(0)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        assert document.to_pdf().startswith(b"%PDF-")


def test_reads_invoice_graphics():
    with Document.open(FIXTURES / "发票示例.ofd") as document:
        sources = document.page_sources(0)
        assert sources[0]["template_id"] == "1"
        assert sources[1]["template_id"] is None
        assert len(document.page_text_with_templates(0)) > len(document.page_text(0))
        paths = document.page_paths(0)
        assert paths
        assert paths[0]["abbreviated_data"].startswith("M ")
        images = document.page_images(0)
        assert images[0]["resource_id"] == "6920"
        assert images[0]["media_path"] == "Doc_0/Res/image_6920.png"
        assert len(document.page_image_data(0, 0)) == 2024
        with pytest.raises(OfdError, match="out of bounds"):
            document.page_image_data(0, 1)
        with pytest.raises(IndexError):
            document.page_paths(-1)
        with pytest.raises(IndexError):
            document.page_images(-1)
        with pytest.raises(IndexError):
            document.page_image_data(-1, 0)
        with pytest.raises(IndexError):
            document.page_sources(-1)
        with pytest.raises(IndexError):
            document.page_text_with_templates(-1)


def test_creates_image_page(tmp_path):
    with Document.open(FIXTURES / "发票示例.ofd") as source:
        png = source.page_image_data(0, 0)
    path = tmp_path / "image.ofd"
    with Document.new() as document:
        with pytest.raises(OfdError, match="image format"):
            document.add_image_page(b"bad", "png")
        with pytest.raises(OfdError):
            document.add_image_page(b"\x89PNG\r\n\x1a\n", "png")
        with pytest.raises(OfdError):
            document.add_image_page(b"\xff\xd8\xff", "jpeg")
        assert document.pages() == []
        document.add_image_page(png, "png", 210, 140)
        document.save(path)
    with Document.open(path) as document:
        assert document.page_images(0)[0]["format"] == "PNG"
        assert document.page_images(0)[0]["ctm"] == pytest.approx([210, 0, 0, 140, 0, 0])
        assert document.page_image_data(0, 0) == png


def test_writes_gif_image_from_java_fixture(tmp_path):
    source_path = (Path(__file__).resolve().parents[4] / "ofdrw-converter" / "src" /
                   "test" / "resources" / "不规范资源路径.ofd")
    with Document.open(source_path) as source:
        gif = source.page_image_data(0, 0)
    path = tmp_path / "gif.ofd"
    with Document.new() as document:
        with pytest.raises(OfdError):
            document.add_image_page(b"GIF89a", "gif", 30, 20)
        document.add_image_page(gif, "gif", 30, 20)
        assert document.page_images(0)[0]["format"] == "GIF"
        assert document.page_image_data(0, 0) == gif
        assert "data:image/png;base64," in document.page_svg(0)
        assert document.to_pdf().startswith(b"%PDF")
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_image_data(0, 0) == gif


def test_creates_jpeg2000_image_page(tmp_path):
    sample = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/red-32x32.jp2"
    jp2 = sample.read_bytes()
    path = tmp_path / "jpeg2000.ofd"
    with Document.new() as document:
        with pytest.raises(OfdError):
            document.add_image_page(jp2[:20], "jp2", 32, 32)
        document.add_image_page(jp2, "jp2", 32, 32)
        assert document.page_images(0)[0]["format"] == "JPEG2000"
        assert document.page_image_data(0, 0) == jp2
        assert "data:image/png;base64," in document.page_svg(0)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_image_data(0, 0) == jp2
        assert reopened.validate_references() == []


def test_imports_pdf_jpxdecode_image():
    fixtures = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures"
    with Document.from_pdf_bytes((fixtures / "red-32x32-jpx.pdf").read_bytes()) as document:
        assert document.page_images(0)[0]["format"] == "JPEG2000"
        assert document.page_image_data(0, 0) == (fixtures / "red-32x32.jp2").read_bytes()
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        assert document.validate_references() == []
    with Document.from_pdf_bytes((fixtures / "red-premultiplied-jpx.pdf").read_bytes()) as document:
        assert document.page_images(0)[0]["format"] == "PNG"
        assert document.page_image_data(0, 0).startswith(b"\x89PNG")
        assert document.validate_references() == []
    with Document.from_pdf_bytes((fixtures / "red-blue-jpx-mask.pdf").read_bytes()) as document:
        assert document.page_images(0)[0]["format"] == "PNG"
        assert document.page_image_data(0, 0).startswith(b"\x89PNG")
        assert document.validate_references() == []


def test_adds_image_object(tmp_path):
    with Document.open(FIXTURES / "发票示例.ofd") as source:
        png = source.page_image_data(0, 0)
    path = tmp_path / "image_object.ofd"
    with Document.new() as document:
        document.add_text_page("before")
        object_id = document.add_image_object(0, png, "png", (10, 20, 30, 40))
        assert document.page_images(0)[0]["id"] == str(object_id)
        assert document.page_images(0)[0]["ctm"] == pytest.approx([30, 0, 0, 40, 0, 0])
        assert document.page_image_data(0, 0) == png
        with pytest.raises(IndexError):
            document.add_image_object(-1, png, "png", (0, 0, 10, 10))
        with pytest.raises(ValueError):
            document.add_image_object(0, png, "png", (0, 0, 10))
        with pytest.raises(OfdError, match="image format"):
            document.add_image_object(0, b"bad", "png", (0, 0, 10, 10))
        document.save(path)
    with Document.open(path) as document:
        assert document.page_images(0)[0]["id"] == str(object_id)
        assert document.page_image_data(0, 0) == png
    path = tmp_path / "existing_image_object.ofd"
    with Document.open(FIXTURES / "helloworld.ofd") as document:
        original_text = document.page_text(0)
        object_id = document.add_image_object(0, png, "png", (10, 20, 30, 40))
        document.save(path)
    with Document.open(path) as document:
        assert document.page_text(0) == original_text
        assert document.page_images(0)[0]["id"] == str(object_id)
        assert document.page_image_data(0, 0) == png


def test_creates_path_page(tmp_path):
    path = tmp_path / "path.ofd"
    with Document.new() as document:
        with pytest.raises(ValueError, match="boundary"):
            document.add_path_page("M 0 0", (0, 0, 10))
        with pytest.raises(OfdError):
            document.add_path_page("M 0", (10, 10, 20, 10))
        with pytest.raises(OfdError):
            document.add_path_page("L 20 10", (10, 10, 20, 10))
        document.add_path_page("M 0 0 L 20 10", (10, 10, 20, 10))
        document.save(path)
    with Document.open(path) as document:
        obj = document.page_paths(0)[0]
        assert obj["boundary"] == [10, 10, 20, 10]
        assert obj["abbreviated_data"] == "M 0 0 L 20 10"
        page_path = document.pages()[0]["path"]
        original = document.entry(page_path)
        document.set_entry(page_path, original.replace(b"M 0 0 L 20 10", b"X 0 0"))
        assert any(issue["code"] == "invalid_path_data" and issue["path"] == page_path
                   for issue in document.validate_references())
        document.set_entry(page_path, original.replace(b"M 0 0 L 20 10", b"L 20 10"))
        assert any(issue["code"] == "invalid_path_data" and issue["path"] == page_path
                   for issue in document.validate_references())
        document.set_entry(page_path, original)
        assert document.validate_references() == []
        clip = (b'<ofd:Clips><ofd:Clip><ofd:Area>'
                b'<ofd:Path Boundary="0 0 10 10">'
                b'<ofd:AbbreviatedData>X 0 0</ofd:AbbreviatedData>'
                b'</ofd:Path></ofd:Area></ofd:Clip></ofd:Clips>')
        document.set_entry(page_path, original.replace(b'</ofd:PathObject>',
                                                       clip + b'</ofd:PathObject>', 1))
        assert any(issue["code"] == "invalid_path_data" and "clip Path" in issue["message"]
                   for issue in document.validate_references())
        document.set_entry(page_path, original)


def test_creates_and_appends_blank_pages(tmp_path):
    output = tmp_path / "blank.ofd"
    with Document.new() as document:
        document.add_blank_page(40, 30)
        assert document.page_text_objects(0) == []
        assert document.page_text(0) == []
        document.append_blank_page(0, 20, 20)
        assert document.page_count == 2
        with pytest.raises(OfdError):
            document.add_blank_page(0, 20)
        with pytest.raises(IndexError):
            document.append_blank_page(-1, 20, 20)
        document.save(output)
    with Document.open(output) as reopened:
        assert reopened.page_count == 2
        assert reopened.page_text_objects(1) == []
        assert reopened.validate_references() == []

    with Document.open(FIXTURES / "helloworld.ofd") as existing:
        existing.append_blank_page(0, 40, 30)
        assert existing.page_text(1) == []
        with pytest.raises(OfdError):
            existing.add_blank_page(40, 30)


def test_appends_document_bodies_and_round_trips(tmp_path):
    path = tmp_path / "multiple-bodies.ofd"
    with Document.new() as document:
        document.add_text_page("first", 40, 30)
        assert document.append_document_body() == 1
        document.append_text_page(1, "second", 50, 40)
        assert document.append_document_body() == 2
        document.append_blank_page(2, 30, 30)
        assert [page["document_index"] for page in document.pages()] == [0, 1, 2]
        assert len({body["doc_id"] for body in document.documents()}) == 3
        original_roots = [body["root"] for body in document.documents()]
        document.move_document_body(0, 2)
        assert [document.page_text(i) for i in range(3)] == [["second"], [], ["first"]]
        assert document.documents()[2]["root"] == original_roots[0]
        removed_root = document.documents()[1]["root"]
        retained_bytes = document.entry(removed_root)
        document.remove_document_body(1)
        assert [document.page_text(i) for i in range(2)] == [["second"], ["first"]]
        assert document.entry(removed_root) == retained_bytes
        with pytest.raises(IndexError):
            document.move_document_body(-1, 0)
        with pytest.raises(OfdError, match="out of bounds"):
            document.move_document_body(0, 2)
        with pytest.raises(IndexError):
            document.remove_document_body(-1)
        with pytest.raises(OfdError, match="out of bounds"):
            document.remove_document_body(2)
        assert document.validate_references() == []
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["second"]
        assert reopened.page_text(1) == ["first"]
        assert len(reopened.documents()) == 2


def test_imports_document_bodies_with_resources(tmp_path):
    path = tmp_path / "imported-bodies.ofd"
    with Document.new() as source, Document.new() as target:
        source.add_text_page("source", 40, 30)
        source.add_attachment("note.txt", b"payload")
        target.add_text_page("target", 40, 30)
        assert target.append_document_bodies_from(source) == [1]
        assert target.page_text(0) == ["target"]
        assert target.page_text(1) == ["source"]
        assert len(target.attachments()) == 1
        assert target.validate_references() == []
        target.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(1) == ["source"]
        assert reopened.validate_references() == []


def test_foreign_namespace_paths_do_not_break_import_or_snapshot():
    with Document.new() as source, Document.new() as target:
        source.set_entry("Doc_0/Payload.bin", b"payload")
        source.set_entry(
            "Doc_0/Foreign.xml",
            b'<x:Refs xmlns:x="urn:other"><x:FileLoc>/Doc_0/Payload.bin</x:FileLoc></x:Refs>',
        )
        source_before = source.to_bytes()
        target_before = target.to_bytes()
        with pytest.raises(OfdError):
            target.append_document_body_from(source, 0)
        with pytest.raises(OfdError):
            source.add_version_snapshot("foreign")
        assert source.to_bytes() == source_before
        assert target.to_bytes() == target_before

        literal = b'<x:Refs xmlns:x="urn:other"><x:FileLoc>/outside/value.xml</x:FileLoc></x:Refs>'
        source.set_entry("Doc_0/Foreign.xml", literal)
        assert target.append_document_body_from(source, 0) == 1
        assert target.entry("Imported_0/Doc_0/Foreign.xml") == literal
        source.add_version_snapshot("foreign")
        assert source.entry("Versions/foreign/Snapshot/Doc_0/Foreign.xml") == literal


def test_imports_one_selected_document_body(tmp_path):
    output = tmp_path / "selected-body.ofd"
    with Document.new() as source, Document.new() as target:
        source.add_text_page("first", 40, 30)
        assert source.append_document_body() == 1
        source.append_text_page(1, "selected", 40, 30)
        source.add_attachment_to_document(1, "selected.txt", b"payload")
        target.add_text_page("target", 40, 30)
        before = target.to_bytes()
        with pytest.raises(IndexError):
            target.append_document_body_from(source, -1)
        with pytest.raises(OfdError, match="out of bounds"):
            target.append_document_body_from(source, 2)
        assert target.to_bytes() == before
        assert target.append_document_body_from(source, 1) == 1
        assert [target.page_text(i) for i in range(2)] == [["target"], ["selected"]]
        assert target.attachment_data(0) == b"payload"
        assert target.validate_references() == []
        target.save(output)
    with Document.open(output) as reopened:
        assert reopened.page_text(1) == ["selected"]
        assert len(reopened.documents()) == 2


def test_extracts_one_document_body_to_independent_package():
    with Document.new() as source:
        source.add_text_page("first", 40, 30)
        assert source.append_document_body() == 1
        source.append_text_page(1, "selected", 40, 30)
        original = source.to_bytes()
        with pytest.raises(IndexError):
            source.extract_document_body(-1)
        with pytest.raises(OfdError, match="out of bounds"):
            source.extract_document_body(2)
        with source.extract_document_body(1) as extracted:
            assert len(extracted.documents()) == 1
            assert extracted.page_text(0) == ["selected"]
            assert extracted.validate_references() == []
            data = extracted.to_bytes()
        assert source.to_bytes() == original
    with Document.from_bytes(data) as reopened:
        assert reopened.page_text(0) == ["selected"]
        assert len(reopened.documents()) == 1


def test_flows_text_across_pages_and_round_trips(tmp_path):
    output = tmp_path / "flowed.ofd"
    with Document.new() as document:
        pages = document.add_flowed_text(
            0, "A\nB\nC", page_size=(40, 22), margins=(5, 5, 5, 5),
            font_size=5, line_height=6,
        )
        assert pages == [0, 1]
        assert document.page_text(0) == ["A", "B"]
        assert document.page_text(1) == ["C"]
        before = document.page_count
        with pytest.raises(ValueError):
            document.add_flowed_text(0, "bad", margins=(1, 2))
        with pytest.raises(OfdError):
            document.add_flowed_text(0, "bad", font_id="missing")
        with pytest.raises(OfdError):
            document.add_flowed_text(0, "bad", align="justify")
        assert document.page_count == before
        aligned = document.add_flowed_text(
            0, "A\nB", page_size=(40, 22), margins=(5, 5, 5, 5),
            align="right", first_line_indent=4,
        )
        assert aligned == [2]
        for obj in document.page_text_objects(2):
            x, _, width, _ = obj["boundary"]
            assert x + width == pytest.approx(35)
        assert document.add_flowed_text(
            0, "A\n", page_size=(40, 22), margins=(5, 5, 5, 5),
        ) == [3]
        assert document.page_text(3) == ["A"]
        document.save(output)
    with Document.open(output) as reopened:
        assert reopened.page_text(1) == ["C"]
        assert reopened.validate_references() == []

    with Document.open(FIXTURES / "helloworld.ofd") as existing:
        pages = existing.add_flowed_text(0, "A\nB", page_size=(40, 30),
                                         margins=(5, 5, 5, 5))
        assert pages == [1]
        assert existing.page_text(1) == ["A", "B"]


def test_plain_and_styled_flow_keep_joined_emoji_together(tmp_path):
    font = (Path(__file__).resolve().parents[4] / "ofdrw-layout" / "src" /
            "test" / "resources" / "simhei-cut1.ttf").read_bytes()
    path = tmp_path / "emoji-flow.ofd"
    style = dict(page_size=(5, 8), margins=(1, 1, 1, 1),
                 font_size=1, line_height=1.5)
    with Document.new() as document:
        font_id = document.add_embedded_font("SimHei", font)
        assert document.add_flowed_text(0, "X👩\u200d💻", font_id=font_id, **style) == [0]
        assert document.page_text(0) == ["X", "👩\u200d💻"]
        assert document.add_flowed_spans(
            0, [{"text": "X👩\u200d💻"}], font_id=font_id, **style
        ) == [1]
        assert document.page_text(1) == ["X", "👩\u200d💻"]
        word_style = {**style, "page_size": (6, 8)}
        assert document.add_flowed_text(0, "hello world", font_id=font_id, **word_style) == [2]
        assert document.page_text(2) == ["hello ", "world"]
        assert document.add_flowed_spans(
            0, [{"text": "hello "}, {"text": "world"}], font_id=font_id, **word_style
        ) == [3]
        assert document.page_text(3) == ["hello ", "world"]
        assert document.add_flowed_text(
            0, "A\r\nB\u2028C", font_id=font_id, **word_style
        ) == [4]
        assert document.page_text(4) == ["A", "B", "C"]
        assert document.add_flowed_spans(
            0, [{"text": "A\r"}, {"text": "\nB\u2028C"}], font_id=font_id, **word_style
        ) == [5]
        assert document.page_text(5) == ["A", "B", "C"]
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["X", "👩\u200d💻"]
        assert reopened.page_text(1) == ["X", "👩\u200d💻"]
        assert reopened.page_text(2) == ["hello ", "world"]
        assert reopened.page_text(3) == ["hello ", "world"]
        assert reopened.page_text(4) == ["A", "B", "C"]
        assert reopened.page_text(5) == ["A", "B", "C"]


def test_flowed_rest_line_indent_for_plain_shaped_and_styled_text():
    font = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" /
            "fixtures" / "dejavu-sans.ttf").read_bytes()
    with Document.new() as document:
        font_id = document.add_embedded_font("DejaVu Sans", font)
        options = dict(page_size=(25, 80), margins=(2, 2, 2, 2),
                       font_size=5, line_height=6, font_id=font_id,
                       rest_line_indent=4)
        for shape in (False, True):
            for spans in (False, True):
                if spans:
                    pages = document.add_flowed_spans(
                        0, [{"text": "AAAAAAAAAAAA"}], shape=shape, **options,
                    )
                else:
                    pages = document.add_flowed_text(
                        0, "AAAAAAAAAAAA", shape=shape, **options,
                    )
                objects = document.page_text_objects(pages[0])
                assert len(objects) >= 2
                assert objects[0]["boundary"][0] == 2
                assert all(obj["boundary"][0] == 6 for obj in objects[1:])
        with pytest.raises(OfdError, match="geometry"):
            document.add_flowed_text(
                0, "A", **{**options, "rest_line_indent": 21},
            )
        assert document.page_count == 4


def test_flowed_text_language_controls_wrapping_and_span_override():
    font = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" /
            "fixtures" / "dejavu-sans.ttf").read_bytes()
    with Document.new() as document:
        font_id = document.add_embedded_font("DejaVu Sans", font)
        options = dict(page_size=(8.14, 20), margins=(1, 1, 1, 1),
                       font_size=5, line_height=6, font_id=font_id, shape=True)
        assert document.add_flowed_text(0, "бб", language="ru", **options) == [0]
        assert document.page_text(0) == ["б", "б"]
        assert document.add_flowed_text(0, "бб", language="sr", **options) == [1]
        assert document.page_text(1) == ["бб"]
        assert document.add_flowed_spans(
            0, [{"text": "б"}, {"text": "б", "language": "sr"}],
            language="ru", **{**options, "page_size": (40, 20)},
        ) == [2]
        objects = document.page_text_objects(2)
        assert len(objects) == 2
        assert objects[0]["cg_transforms"][0]["glyphs"] != objects[1]["cg_transforms"][0]["glyphs"]
        with pytest.raises(OfdError, match="requires shaping"):
            document.add_flowed_text(0, "б", language="sr",
                                     **{**options, "shape": False})
        with pytest.raises(OfdError, match="invalid shaping language"):
            document.add_flowed_spans(
                0, [{"text": "б", "language": "sr__invalid"}],
                language="ru", **options,
            )
        assert document.page_count == 3


def test_flows_shaped_ligatures_with_embedded_font(tmp_path):
    font = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" /
            "fixtures" / "dejavu-serif.ttf").read_bytes()
    path = tmp_path / "shaped-flow.ofd"
    with Document.new() as document:
        font_id = document.add_embedded_font("DejaVu Serif", font)
        pages = document.add_flowed_text(
            0, "fifi", page_size=(14.5, 22), margins=(5, 5, 5, 5),
            font_size=5, line_height=6, font_id=font_id, shape=True,
        )
        assert pages == [0]
        assert document.page_text(0) == ["fi", "fi"]
        objects = document.page_text_objects(0)
        assert len(objects) == 2
        assert all(object["cg_transforms"][0]["code_count"] == 2 and
                   object["cg_transforms"][0]["glyph_count"] == 1
                   for object in objects)
        assert document.page_svg(0).count("data-ofd-glyph=") == 2
        before = document.page_count
        with pytest.raises(OfdError):
            document.add_flowed_text(
                0, "a\u0301", page_size=(11, 22), margins=(5, 5, 5, 5),
                font_size=5, line_height=6, font_id=font_id, shape=True,
            )
        assert document.page_count == before
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_text(0) == ["fi", "fi"]
        assert reopened.page_text_objects(0)[0]["cg_transforms"][0]["glyph_count"] == 1


def test_flows_shaped_styled_spans_across_pages():
    font = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" /
            "fixtures" / "dejavu-serif.ttf").read_bytes()
    with Document.new() as document:
        font_id = document.add_embedded_font("DejaVu Serif", font)
        pages = document.add_flowed_spans(
            0,
            [{"text": "fi", "fill_color": (255, 0, 0), "underline": True},
             {"text": "fi", "italic": True}],
            page_size=(14.5, 16), margins=(5, 5, 5, 5),
            font_size=5, line_height=6, font_id=font_id, shape=True,
        )
        assert pages == [0, 1]
        assert [document.page_text(page) for page in pages] == [["fi"], ["fi"]]
        assert all(document.page_text_objects(page)[0]["cg_transforms"][0]
                   ["glyph_count"] == 1 for page in pages)
        assert len(document.page_paths(0)) == 1
        assert "#ff0000" in document.page_svg(0)


def test_page_layers_follow_type_order():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        path = document.pages()[0]["path"]
        xml = document.entry(path)
        xml = xml.replace(b"<ofd:Layer ", b'<ofd:Layer Type="Foreground" ', 1)
        xml = xml.replace(b"</ofd:Content>",
                          b'<ofd:Layer ID="80" Type="Background"/></ofd:Content>')
        document.set_entry(path, xml)
        layers = document.page_layers(0)
        assert [layer["layer_type"] for layer in layers] == ["Background", "Foreground"]
        assert [layer["source_layer_index"] for layer in layers] == [1, 0]
        assert all(layer["source_path"] == path for layer in layers)
        svg = document.page_svg(0)
        assert svg.find('data-ofd-layer="80"') < svg.find(
            f'data-ofd-layer="{layers[1]["id"]}"')
        with pytest.raises(IndexError):
            document.page_layers(-1)


def test_flowed_character_indent_matches_span_font_size():
    with Document.new() as document:
        style = {"page_size": (40, 22), "margins": (5, 5, 5, 5),
                 "font_size": 5, "line_height": 6,
                 "first_line_indent_chars": 2}
        assert document.add_flowed_text(0, "A\nB", **style) == [0]
        assert [o["boundary"][0] for o in document.page_text_objects(0)] == [15, 5]
        assert document.add_flowed_spans(
            0, [{"text": "A\nB", "font_size": 3, "letter_spacing": 0.5}],
            **style,
        ) == [1]
        assert [o["boundary"][0] for o in document.page_text_objects(1)] == [12, 5]
        assert document.add_flowed_text(0, "A", first_line_indent=3, **style) == [2]
        assert document.page_text_objects(2)[0]["boundary"][0] == 8
        repeated = dict(style, indent_each_paragraph=True)
        assert document.add_flowed_text(0, "A\nB", **repeated) == [3]
        assert [o["boundary"][0] for o in document.page_text_objects(3)] == [15, 15]
        assert document.add_flowed_spans(
            0, [{"text": "A\nB", "font_size": 3, "letter_spacing": 0.5}],
            **repeated,
        ) == [4]
        assert [o["boundary"][0] for o in document.page_text_objects(4)] == [12, 12]
        before = document.page_count
        with pytest.raises(OfdError):
            document.add_flowed_text(0, "A", first_line_indent_chars=-1)
        with pytest.raises(OfdError):
            document.add_flowed_text(0, "A", indent_each_paragraph="yes")
        assert document.page_count == before


def test_flowed_paragraph_gap_moves_lines_and_collapses_at_page_top():
    style = dict(page_size=(30, 22), margins=(2, 2, 2, 2),
                 font_size=4, line_height=5, paragraph_gap=3)
    with Document.new() as document:
        assert document.add_flowed_text(0, "A\nB\nC", **style) == [0, 1]
        assert [obj["boundary"][1] for obj in document.page_text_objects(0)] == [2, 10]
        assert document.page_text_objects(1)[0]["boundary"][1] == 2
        assert document.add_flowed_spans(0, [{"text": "A\nB\nC"}], **style) == [2, 3]
        assert [obj["boundary"][1] for obj in document.page_text_objects(2)] == [2, 10]
        assert document.page_text_objects(3)[0]["boundary"][1] == 2
        with pytest.raises(OfdError):
            document.add_flowed_text(0, "A", paragraph_gap=-1)
        with pytest.raises(OfdError):
            document.add_flowed_spans(0, [{"text": "A"}], paragraph_gap="bad")
        assert document.page_count == 4


def test_flowed_paragraphs_can_stay_on_one_page():
    font = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
            "tests" / "fixtures" / "dejavu-serif.ttf").read_bytes()
    with Document.new() as document:
        font_id = document.add_embedded_font("DejaVu Serif", font)
        style = dict(page_size=(14, 18), margins=(2, 2, 2, 2),
                     font_size=4, line_height=5, paragraph_gap=2,
                     keep_paragraphs_together=True, font_id=font_id)
        assert document.add_flowed_text(0, "A\nBBBBB", **style) == [0, 1]
        assert document.page_text(0) == ["A"]
        assert len(document.page_text_objects(1)) == 2
        assert document.add_flowed_spans(0, [{"text": "A\nBBBBB"}], **style) == [2, 3]
        assert document.page_text(2) == ["A"]
        assert len(document.page_text_objects(3)) == 2
        with pytest.raises(OfdError):
            document.add_flowed_text(0, "B" * 40, **style)
        assert document.page_count == 4


def test_flows_styled_spans_across_pages(tmp_path):
    output = tmp_path / "styled-flow.ofd"
    with Document.new() as document:
        pages = document.add_flowed_spans(
            0,
            [
                {"text": "AA", "bold": True, "fill_color": (255, 0, 0),
                 "letter_spacing": 0.5, "underline": True,
                 "underline_offset": 0.5, "underline_width": 0.2,
                 "line_break_after": True},
                {"text": "B\nC", "font_size": 3, "italic": True,
                 "fill_color": (0, 0, 255), "keep_together": True},
            ],
            page_size=(40, 16), margins=(5, 5, 5, 5),
        )
        assert pages == [0, 1, 2]
        assert document.page_text(0) == ["AA"]
        assert document.page_text(1) == ["B"]
        assert document.page_text(2) == ["C"]
        assert document.page_text_objects(0)[0]["size"] == 5
        assert document.page_text_objects(1)[0]["size"] == 3
        assert document.page_text_objects(0)[0]["codes"][0]["delta_x"] is not None
        assert len(document.page_paths(0)) == 1
        svg = document.page_svg(0)
        assert "#ff0000" in svg
        assert "#0000ff" in document.page_svg(1)
        before = document.page_count
        with pytest.raises(OfdError):
            document.add_flowed_spans(0, [{"text": "bad", "font_size": 9}],
                                      page_size=(40, 16), margins=(5, 5, 5, 5))
        assert document.page_count == before
        assert document.add_flowed_spans(
            0, [{"text": "D\n"}], page_size=(40, 16), margins=(5, 5, 5, 5),
        ) == [3]
        assert document.page_text(3) == ["D"]
        document.save(output)
    with Document.open(output) as reopened:
        assert reopened.page_text(2) == ["C"]
        assert len(reopened.page_paths(0)) == 1
        assert reopened.validate_references() == []


def test_flowed_text_and_spans_preserve_spaces(tmp_path):
    output = tmp_path / "flowed-spaces.ofd"
    with Document.new() as document:
        style = {"page_size": (40, 22), "margins": (5, 5, 5, 5)}
        assert document.add_flowed_text(0, " A \n B ", **style) == [0]
        assert document.add_flowed_spans(0, [{"text": " A \n B "}], **style) == [1]
        assert document.page_text(0) == [" A ", " B "]
        assert document.page_text(1) == [" A ", " B "]
        document.save(output)
    with Document.open(output) as reopened:
        assert reopened.page_text(0) == [" A ", " B "]
        assert reopened.page_text(1) == [" A ", " B "]


def test_flowed_spans_use_variable_line_heights(tmp_path):
    output = tmp_path / "variable-line-height.ofd"
    with Document.new() as document:
        pages = document.add_flowed_spans(
            0,
            [{"text": "A"}, {"text": "x\n", "font_size": 3},
             {"text": "B\n", "font_size": 3},
             {"text": "C", "font_size": 3}],
            page_size=(40, 20), margins=(5, 5, 5, 5),
        )
        assert pages == [0, 1]
        assert document.page_text(0) == ["A", "x", "B"]
        assert document.page_text(1) == ["C"]
        objects = document.page_text_objects(0)
        assert [item["boundary"][3] for item in objects] == [6, 6, 4]
        assert objects[2]["boundary"][1] == 11
        assert document.page_text_objects(1)[0]["boundary"][3] == 4
        document.save(output)
    with Document.open(output) as reopened:
        assert reopened.page_text(0) == ["A", "x", "B"]
        assert reopened.page_text(1) == ["C"]


def test_flowed_text_rejects_glyph_wider_than_content_area():
    with Document.new() as document:
        style = {"page_size": (11, 20), "margins": (5, 5, 5, 5)}
        with pytest.raises(OfdError):
            document.add_flowed_text(0, "中", **style)
        with pytest.raises(OfdError):
            document.add_flowed_spans(0, [{"text": "中"}], **style)
        assert document.page_count == 0


def test_flowed_spans_weight_fill_and_stroke(tmp_path):
    output = tmp_path / "styled-outline.ofd"
    with Document.new() as document:
        assert document.add_flowed_spans(
            0,
            [{"text": "描边", "weight": 600, "bold": True, "fill": False,
              "stroke": True, "stroke_color": (20, 40, 60),
              "stroke_width": 0.2}],
            page_size=(40, 20), margins=(5, 5, 5, 5),
        ) == [0]
        svg = document.page_svg(0)
        assert 'font-weight="600"' in svg
        assert 'fill="none"' in svg
        assert 'stroke="#14283c"' in svg
        with pytest.raises(OfdError):
            document.add_flowed_spans(0, [{"text": "bad", "weight": 650}])
        assert document.page_count == 1
        document.save(output)
    with Document.open(output) as reopened:
        assert reopened.page_text(0) == ["描边"]
        assert 'stroke="#14283c"' in reopened.page_svg(0)


def test_flowed_text_uses_embedded_font_pair_kerning(tmp_path):
    font = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" /
            "fixtures" / "kerning-simhei.ttf").read_bytes()
    output = tmp_path / "kerning.ofd"
    with Document.new() as document:
        font_id = document.add_embedded_font("Kerning Test", font)
        style = {"page_size": (14.5, 20), "margins": (5, 5, 5, 5),
                 "font_id": font_id}
        assert document.add_flowed_text(0, "HW", **style) == [0]
        assert document.add_flowed_spans(
            0, [{"text": "HW", "keep_together": True}], **style,
        ) == [1]
        for page in (0, 1):
            obj = document.page_text_objects(page)[0]
            assert obj["boundary"][2] == 3.75
            assert obj["codes"][0]["delta_x"] == "1.25"
        assert document.add_flowed_spans(
            0, [{"text": "H"}, {"text": "W"}], **style,
        ) == [2, 3]
        assert document.page_text(2) == ["H"]
        assert document.page_text(3) == ["W"]
        document.save(output)
    with Document.open(output) as reopened:
        assert reopened.page_text(0) == ["HW"]
        assert reopened.page_text_objects(1)[0]["codes"][0]["delta_x"] == "1.25"


def test_appends_page_to_existing_document(tmp_path):
    path = tmp_path / "extended.ofd"
    with Document.open(FIXTURES / "helloworld.ofd") as document:
        with pytest.raises(IndexError):
            document.append_text_page(-1, "invalid")
        document.append_text_page(0, "追加 & 页面", 180, 250)
        document.save(path)
    with Document.open(path) as document:
        assert document.page_count == 2
        assert document.page_text(1) == ["追加 & 页面"]


def test_appends_path_page_to_existing_document(tmp_path):
    path = tmp_path / "path-extended.ofd"
    with Document.open(FIXTURES / "helloworld.ofd") as document:
        with pytest.raises(IndexError):
            document.append_path_page(-1, "M 0 0 L 10 10", (0, 0, 10, 10))
        with pytest.raises(ValueError, match="boundary"):
            document.append_path_page(0, "M 0 0", (0, 0, 10))
        document.append_path_page(0, "M 0 0 L 10 10", (5, 5, 20, 20), 80, 60)
        assert document.page_count == 2
        assert document.page_paths(1)[0]["boundary"] == [5, 5, 20, 20]
        assert document.page_text(0) == ["你好呀，OFD Reader&Writer！"]
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_paths(1)[0]["abbreviated_data"] == "M 0 0 L 10 10"


def test_appends_image_page_to_existing_document(tmp_path):
    with Document.open(FIXTURES / "发票示例.ofd") as source:
        png = source.page_image_data(0, 0)
    path = tmp_path / "image-extended.ofd"
    with Document.open(FIXTURES / "helloworld.ofd") as document:
        with pytest.raises(IndexError):
            document.append_image_page(-1, png, "png", 50, 40)
        with pytest.raises(OfdError, match="invalid image format"):
            document.append_image_page(0, b"bad", "png", 50, 40)
        document.append_image_page(0, png, "png", 50, 40)
        assert document.page_count == 2
        assert document.page_image_data(1, 0) == png
        assert document.page_text(0) == ["你好呀，OFD Reader&Writer！"]
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.page_images(1)[0]["format"] == "PNG"
        assert reopened.page_image_data(1, 0) == png


def test_reads_document_versions():
    source = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/versioned.ofd"
    with Document.open(source) as document:
        versions = document.versions()
        assert len(versions) == 1
        assert versions[0]["id"] == "v1"
        assert versions[0]["current"] is True
        assert versions[0]["description_path"] == "Versions/V1.xml"
        assert len(versions[0]["files"]) == 3
        assert document.validate_references() == []


def test_creates_independent_version_snapshots(tmp_path):
    target = tmp_path / "versioned.ofd"
    with Document.new() as document:
        document.add_text_page("original", 80, 50)
        first = document.add_version_snapshot("v1", name="First")
        first_root = document.entry(first["doc_root"])
        assert first["index"] == 1
        assert first["current"] is True
        document.add_text_page("later", 80, 50)
        second = document.add_version_snapshot("v2")
        assert second["index"] == 2
        assert document.versions()[0]["current"] is False
        assert document.entry(first["doc_root"]) == first_root
        assert document.validate_references() == []
        with pytest.raises(OfdError):
            document.add_version_snapshot("v2")
        with pytest.raises(IndexError):
            document.add_version_snapshot("v3", document_index=-1)
        document.save(target)
    with Document.open(target) as reopened:
        assert [version["id"] for version in reopened.versions()] == ["v1", "v2"]
        assert reopened.entry(first["doc_root"]) == first_root
        before = reopened.to_bytes()
        with reopened.extract_version_snapshot(0) as historical:
            assert historical.page_text(0) == ["original"]
            assert historical.versions() == []
            assert historical.validate_references() == []
            archived = historical.to_bytes()
        with Document.from_bytes(archived) as standalone:
            assert standalone.page_text(0) == ["original"]
        with pytest.raises(IndexError):
            reopened.extract_version_snapshot(-1)
        with pytest.raises(OfdError, match="out of bounds"):
            reopened.extract_version_snapshot(2)
        assert reopened.to_bytes() == before


def test_extracts_version_without_listed_ofd_root():
    with Document.new() as document:
        document.add_text_page("historical", 40, 30)
        version = document.add_version_snapshot("no-root")
        document.add_text_page("current", 40, 30)
        description = ElementTree.fromstring(document.entry(version["description_path"]))
        file_list = next(child for child in description if child.tag.endswith("FileList"))
        standalone = next(
            child for child in file_list
            if child.tag.endswith("File") and child.text == "Snapshot/OFD.xml"
        )
        file_list.remove(standalone)
        document.set_entry(
            version["description_path"], ElementTree.tostring(description, encoding="utf-8")
        )
        before = document.to_bytes()
        with document.extract_version_snapshot(0) as historical:
            assert historical.page_text(0) == ["historical"]
            assert historical.validate_references() == []
            with Document.from_bytes(historical.to_bytes()) as reopened:
                assert reopened.page_text(0) == ["historical"]
        assert document.to_bytes() == before


def test_reads_and_edits_document_metadata(tmp_path):
    path = tmp_path / "metadata.ofd"
    with Document.open(FIXTURES / "发票示例.ofd") as document:
        original = document.documents()[0]
        assert original["author"] == "China Tax"
        assert any(item["name"] == "合计金额" for item in original["custom_data"])
        with pytest.raises(OfdError, match="DocID"):
            document.set_document_info_field(0, "DocID", "bad")
        with pytest.raises(OfdError):
            document.remove_document_info_field(0, "DocID")
        doc_id = "0123456789abcdef0123456789abcdef"
        document.set_document_info_field(0, "DocID", doc_id)
        with pytest.raises(IndexError):
            document.set_document_info_field(-1, "Title", "bad")
        with pytest.raises(OfdError, match="unsupported"):
            document.set_document_info_field(0, "DocRoot", "bad")
        with pytest.raises(OfdError, match="missing entry"):
            document.set_document_info_field(0, "Cover", "missing.png")
        cover = (Path(__file__).resolve().parents[4] / "ofdrw-layout/src/test/resources/testimg.png").read_bytes()
        document.set_entry("Doc_0/Res/cover.png", cover)
        document.set_document_info_field(0, "Cover", "/Doc_0/Res/cover.png")
        document.set_document_info_field(0, "Cover", "/DOC_0/RES/COVER.PNG")
        assert not any(issue["code"] == "cover_reference"
                       for issue in document.validate_references())
        document.set_document_info_field(0, "Cover", "/Doc_0/Res/cover.png")
        document.set_document_info_field(0, "Title", "Rust & OFD")
        with pytest.raises(OfdError):
            document.set_document_info_field(0, "CreationDate", "2024-02-30")
        with pytest.raises(OfdError):
            document.set_document_info_field(0, "DocUsage", "Magazine")
        document.set_document_info_field(0, "CreationDate", "2024-02-29")
        document.set_document_info_field(0, "DocUsage", "EBook")
        document.save(path)
    with Document.open(path) as document:
        edited = document.documents()[0]
        assert edited["title"] == "Rust & OFD"
        assert edited["creation_date"] == "2024-02-29"
        assert edited["doc_usage"] == "EBook"
        assert edited["doc_id"] == doc_id
        assert edited["cover"] == "/Doc_0/Res/cover.png"
        assert document.entry(edited["cover"].lstrip("/")) == cover
        assert edited["custom_data"] == original["custom_data"]
        ofd_xml = document.entry("OFD.xml")
        document.set_entry("OFD.xml", ofd_xml.replace(doc_id.encode(), b"bad", 1))
        assert any(issue["code"] == "invalid_doc_id"
                   for issue in document.validate_references())
        document.set_entry("OFD.xml", ofd_xml)
        assert document.validate_references() == []


def test_legacy_missing_document_id_remains_readable():
    with Document.new() as document:
        original = document.entry("OFD.xml")
        doc_id = document.documents()[0]["doc_id"]
        missing = original.replace(
            f"<ofd:DocID>{doc_id}</ofd:DocID>".encode(), b"", 1)
        assert missing != original
        document.set_entry("OFD.xml", missing)
        assert document.documents()[0]["doc_id"] == ""
        assert document.validate_references() == []
        data = document.to_bytes()
    with Document.from_bytes(data) as reopened:
        assert reopened.documents()[0]["doc_id"] == ""


def test_document_permissions_round_trip_and_validation(tmp_path):
    path = tmp_path / "permissions.ofd"
    with Document.new() as document:
        document.add_text_page("permissions", 80, 40)
        permissions = document.document_permissions(0)
        assert permissions["edit"] is True
        assert permissions["print"] == {"printable": True, "copies": -1}
        assert permissions["valid_period"] is None
        with pytest.raises(IndexError):
            document.document_permissions(-1)
        with pytest.raises(OfdError):
            document.document_permissions(1)
        permissions["edit"] = False
        permissions["print"] = {"printable": True, "copies": 2}
        permissions["valid_period"] = {
            "start_date": "2024-02-29T12:00:00", "end_date": "2025-01-01T00:00:00"
        }
        document.set_document_permissions(0, permissions)
        assert document.document_permissions(0) == permissions
        invalid = {**permissions, "valid_period": {"start_date": "2024-02-30T00:00:00", "end_date": None}}
        with pytest.raises(OfdError):
            document.set_document_permissions(0, invalid)
        assert document.document_permissions(0) == permissions
        document.save(path)
    with Document.open(path) as document:
        assert document.document_permissions(0) == permissions


def test_view_preferences_round_trip_and_validation(tmp_path):
    path = tmp_path / "view-preferences.ofd"
    with Document.new() as document:
        preferences = document.view_preferences(0)
        assert preferences["page_mode"] == "None"
        assert preferences["page_layout"] == "OneColumn"
        assert preferences["tab_display"] == "FileName"
        assert preferences["zoom"] is None
        with pytest.raises(IndexError):
            document.view_preferences(-1)
        preferences.update(page_mode="UseOutlines", page_layout="TwoColumnR",
                           hide_toolbar=True, zoom_mode="FitWidth")
        document.set_view_preferences(0, preferences)
        assert document.view_preferences(0) == preferences
        invalid = {**preferences, "zoom": 1.25}
        with pytest.raises(OfdError):
            document.set_view_preferences(0, invalid)
        assert document.view_preferences(0) == preferences
        document.save(path)
    with Document.open(path) as document:
        assert document.view_preferences(0) == preferences


def test_document_actions_round_trip_and_validation(tmp_path):
    path = tmp_path / "document-actions.ofd"
    with Document.new() as document:
        document.add_text_page("target", 80, 40)
        page_id = document.pages()[0]["id"]
        document.add_bookmark("chapter", page_id)
        document.add_attachment("note.txt", b"note")
        attachment_id = document.attachments()[0]["id"]
        assert document.document_actions(0) == []
        with pytest.raises(OfdError):
            document.add_document_action(0, {"kind": "page", "event": "DO",
                                             "destination": {"page_id": "missing", "kind": "Fit"}})
        document.add_document_action(0, {"kind": "uri", "event": "DO",
                                         "uri": "https://example.org/?x=1&y=2"})
        document.add_document_action(0, {"kind": "page", "event": "DO",
                                         "destination": {"page_id": page_id, "kind": "Fit"}})
        document.add_document_action(0, {"kind": "bookmark", "event": "CLICK", "name": "chapter"})
        document.add_document_action(0, {"kind": "attachment", "event": "DO",
                                         "id": attachment_id, "new_window": True})
        actions = document.document_actions(0)
        assert [action["kind"] for action in actions] == ["URI", "Goto", "Goto", "GotoA"]
        assert actions[0]["attributes"]["URI"] == "https://example.org/?x=1&y=2"
        assert actions[1]["destination"]["page_id"] == page_id
        assert actions[2]["bookmark"] == "chapter"
        with pytest.raises(IndexError):
            document.remove_document_action(0, -1)
        with pytest.raises(OfdError):
            document.remove_document_action(0, 4)
        document.remove_document_action(0, 1)
        assert len(document.document_actions(0)) == 3
        document.save(path)
    with Document.open(path) as document:
        assert [action["kind"] for action in document.document_actions(0)] == ["URI", "Goto", "GotoA"]


def test_page_actions_round_trip_and_validation(tmp_path):
    path = tmp_path / "page-actions.ofd"
    with Document.new() as document:
        document.add_text_page("page action", 80, 40)
        page_id = document.pages()[0]["id"]
        assert document.page_actions(0) == []
        with pytest.raises(OfdError):
            document.add_page_action(0, {"kind": "uri", "event": "CLICK", "uri": "https://example.org"})
        with pytest.raises(OfdError):
            document.add_page_action(0, {"kind": "page", "event": "PO",
                                         "destination": {"page_id": "missing", "kind": "Fit"}})
        assert document.page_actions(0) == []
        document.add_page_action(0, {"kind": "uri", "event": "PO", "uri": "https://example.org"})
        document.add_page_action(0, {"kind": "page", "event": "PO",
                                     "destination": {"page_id": page_id, "kind": "Fit"}})
        assert [action["kind"] for action in document.page_actions(0)] == ["URI", "Goto"]
        with pytest.raises(IndexError):
            document.remove_page_action(0, -1)
        document.remove_page_action(0, 0)
        document.save(path)
    with Document.open(path) as document:
        actions = document.page_actions(0)
        assert len(actions) == 1
        assert actions[0]["event"] == "PO"
        assert actions[0]["destination"]["page_id"] == page_id


def test_integrity_manifest_reports_missing_and_extra_files(tmp_path):
    path = tmp_path / "integrity.ofd"
    with Document.new() as document:
        assert document.integrity_manifest() is None
        assert document.check_integrity_manifest() is None
        files = "".join(
            f'<File ID="{index}" FileLoc="/{name}"/>'
            for index, name in enumerate(document.entries(), start=1)
        )
        xml = (f'<DocEntries ID="manifest"><SignedValueLoc>/signedvalue.dat</SignedValueLoc>'
               f'<FileList>{files}</FileList></DocEntries>')
        document.set_entry("OFDEntries.xml", xml.encode())
        document.set_entry("signedvalue.dat", b"opaque signature")
        manifest = document.integrity_manifest()
        assert manifest["id"] == "manifest"
        assert manifest["signed_value_path"] == "signedvalue.dat"
        assert document.check_integrity_manifest()["complete"] is True
        document.set_entry("unexpected.bin", b"extra")
        report = document.check_integrity_manifest()
        assert report["extra_files"] == ["unexpected.bin"]
        assert report["complete"] is False
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.check_integrity_manifest() == report


def test_java_sm2_integrity_signature_and_manifest():
    fixture = FIXTURES.parents[3] / "ofdrw-crypto/src/test/resources/hello-integrity.ofd"
    with Document.open(fixture) as document:
        assert document.verify_integrity_manifest_sm2() == {
            "manifest_complete": True,
            "signature_valid": True,
            "all_valid": True,
        }
        assert document.verify_integrity_manifest_sm2("wrong-id")["signature_valid"] is False
        document.set_entry("extra.bin", b"extra")
        report = document.verify_integrity_manifest_sm2()
        assert report["manifest_complete"] is False
        assert report["signature_valid"] is True
        assert report["all_valid"] is False
    with Document.new() as document:
        assert document.verify_integrity_manifest_sm2() is None


def test_creates_sm2_integrity_protection(tmp_path):
    root = Path(__file__).resolve().parents[3]
    fixtures = root / "crates/ofdrw/tests/fixtures"
    key = (fixtures / "user-private-key.bin").read_bytes()
    certificate = (fixtures / "user-cert.der").read_bytes()
    path = tmp_path / "protected.ofd"
    with Document.new() as document:
        document.add_text_page("protected", 100, 60)
        with pytest.raises(OfdError):
            document.protect_integrity_sm2(key, (fixtures / "root-cert.der").read_bytes())
        assert document.integrity_manifest() is None
        document.protect_integrity_sm2(key, certificate)
        assert document.verify_integrity_manifest_sm2()["all_valid"] is True
        with pytest.raises(OfdError):
            document.protect_integrity_sm2(key, certificate)
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.verify_integrity_manifest_sm2()["all_valid"] is True


def test_integrity_verification_with_ordered_sm2_chain():
    fixtures = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures"
    key = (fixtures / "trust-leaf-private-key.bin").read_bytes()
    signer = (fixtures / "trust-leaf-cert.der").read_bytes()
    anchor = (fixtures / "trust-root-cert.der").read_bytes()
    with Document.new() as document:
        document.protect_integrity_sm2(key, signer)
        report = document.verify_integrity_manifest_sm2_with_ordered_chain(
            signer, [], anchor, 1_893_456_000,
        )
        assert report["signature_valid"] is True
        assert report["certificate_chain_valid"] is True
        assert report["all_valid"] is True
        report = document.verify_integrity_manifest_sm2_with_ordered_chain(
            signer, [], anchor, 1_600_000_000,
        )
        assert report["signature_valid"] is True
        assert report["certificate_chain_valid"] is False
        assert report["all_valid"] is False


def test_integrity_certificate_pool_finds_valid_path():
    fixtures = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures"
    key = (fixtures / "chain-leaf-private-key.bin").read_bytes()
    signer = (fixtures / "chain-leaf-cert.der").read_bytes()
    intermediate = (fixtures / "chain-intermediate-cert.der").read_bytes()
    unrelated = (fixtures / "root-cert.der").read_bytes()
    anchor = (fixtures / "chain-root-cert.der").read_bytes()
    with Document.new() as document:
        document.protect_integrity_sm2(key, signer)
        report = document.verify_integrity_manifest_sm2_with_certificate_pool(
            signer, [unrelated, intermediate], anchor, 1_893_456_000,
        )
        assert report["all_valid"] is True
        report = document.verify_integrity_manifest_sm2_with_certificate_pool(
            signer, [unrelated], anchor, 1_893_456_000,
        )
        assert report["signature_valid"] is True
        assert report["certificate_chain_valid"] is False


def test_custom_tags_round_trip_and_validation(tmp_path):
    path = tmp_path / "custom-tags.ofd"
    payload = b'<Invoice Ref="1"/>'
    with Document.new() as document:
        document.add_text_page("tag target", 80, 40)
        document.set_entry("Doc_0/Tags/schema.xsd", b"<schema/>")
        with pytest.raises(OfdError):
            document.add_custom_tag(0, "invoice", b"<broken>")
        with pytest.raises(OfdError):
            document.add_custom_tag(0, "invoice", payload, schema_path="missing.xsd")
        assert document.custom_tags() == []
        first = document.add_custom_tag(0, "invoice", payload, namespace="urn:test",
                                        schema_path="Doc_0/Tags/schema.xsd")
        assert first["type_id"] == "invoice"
        assert first["namespace"] == "urn:test"
        assert first["schema_path"] == "Doc_0/Tags/schema.xsd"
        assert document.custom_tag_data(0) == payload
        second = document.add_custom_tag(0, "audit", b"<Audit/>")
        assert second["path"] != first["path"]
        assert document.custom_tags() == [first, second]
        with pytest.raises(IndexError):
            document.custom_tag_data(-1)
        document.save(path)
    with Document.open(path) as document:
        assert document.custom_tags() == [first, second]
        assert document.custom_tag_data(1) == b"<Audit/>"
        with pytest.raises(IndexError):
            document.update_custom_tag(-1, "bad")
        with pytest.raises(OfdError):
            document.update_custom_tag(2, "bad")
        with pytest.raises(OfdError):
            document.update_custom_tag(0, "changed", schema_path="missing.xsd")
        updated = document.update_custom_tag(0, "updated & invoice", namespace="urn:updated")
        assert updated["path"] == first["path"]
        assert updated["schema_path"] is None
        assert document.custom_tags() == [updated, second]
        assert document.custom_tag_data(0) == payload
        with pytest.raises(IndexError):
            document.replace_custom_tag_data(-1, b"<New/>")
        with pytest.raises(OfdError):
            document.replace_custom_tag_data(0, b"<broken>")
        assert document.custom_tag_data(0) == payload
        replaced = document.replace_custom_tag_data(0, b"<New/>")
        assert replaced["path"] != first["path"]
        assert replaced["type_id"] == updated["type_id"]
        assert document.custom_tag_data(0) == b"<New/>"
        assert document.entry(first["path"]) == payload
        with pytest.raises(IndexError):
            document.remove_custom_tag(-1)
        with pytest.raises(OfdError):
            document.remove_custom_tag(2)
        document.remove_custom_tag(0)
        assert document.custom_tags() == [second]
        assert document.entry(first["path"]) == payload
        assert document.validate_references() == []


def test_extensions_round_trip_and_validation(tmp_path):
    path = tmp_path / "extensions.ofd"
    extension = {
        "document_index": 0, "app_name": "Example & Co", "company": "Acme",
        "app_version": "1.2", "date": "2024-02-29", "ref_id": 1,
        "properties": [{"name": "status", "type_name": "string", "value": "ready & checked"}],
        "extend_data_paths": ["Doc_0/Ext/payload.bin"],
        "data_xml": ['<ofd:Data xmlns:ofd="http://www.ofdspec.org/2016"><Payload/></ofd:Data>'],
    }
    with Document.new() as document:
        document.set_entry("Doc_0/Ext/payload.bin", b"external")
        assert document.extensions() == []
        with pytest.raises(OfdError):
            document.add_extension({**extension, "date": "2025-02-29"})
        with pytest.raises(OfdError):
            document.add_extension({**extension, "extend_data_paths": ["missing.bin"]})
        assert document.extensions() == []
        document.add_extension(extension)
        assert document.extensions() == [extension]
        document.save(path)
    with Document.open(path) as document:
        assert document.extensions() == [extension]
        assert document.entry("Doc_0/Ext/payload.bin") == b"external"
        updated = {**extension, "app_name": "Updated & Co"}
        with pytest.raises(IndexError):
            document.update_extension(-1, updated)
        with pytest.raises(OfdError):
            document.update_extension(1, updated)
        with pytest.raises(OfdError):
            document.update_extension(0, {**updated, "date": "2025-02-29"})
        assert document.extensions() == [extension]
        document.update_extension(0, updated)
        assert document.extensions() == [updated]
        with pytest.raises(IndexError):
            document.remove_extension(-1)
        with pytest.raises(OfdError):
            document.remove_extension(1)
        document.remove_extension(0)
        assert document.extensions() == []
        assert document.entry("Doc_0/Ext/payload.bin") == b"external"
        assert document.validate_references() == []


def test_sets_and_removes_custom_document_metadata(tmp_path):
    path = tmp_path / "custom-metadata.ofd"
    with Document.new() as document:
        document.add_text_page("body", 100, 60)
        with pytest.raises(IndexError):
            document.set_document_custom_data(-1, "Key", "value")
        with pytest.raises(IndexError):
            document.remove_document_custom_data(-1, "Key")
        with pytest.raises(OfdError):
            document.remove_document_custom_data(0, "missing")
        document.set_document_custom_data(0, "客户&类型", "<VIP>")
        document.set_document_custom_data(0, "客户&类型", "更新 & 完成")
        document.set_document_custom_data(0, "Other", "keep")
        assert document.documents()[0]["custom_data"] == [
            {"name": "客户&类型", "value": "更新 & 完成"},
            {"name": "Other", "value": "keep"},
        ]
        document.remove_document_custom_data(0, "客户&类型")
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.documents()[0]["custom_data"] == [{"name": "Other", "value": "keep"}]
        assert reopened.validate_references() == []


def test_edits_keywords_and_removes_standard_metadata(tmp_path):
    path = tmp_path / "keywords.ofd"
    with Document.new() as document:
        document.add_text_page("body", 100, 60)
        with pytest.raises(IndexError):
            document.add_document_keyword(-1, "bad")
        with pytest.raises(IndexError):
            document.remove_document_keyword(0, -1)
        with pytest.raises(IndexError):
            document.remove_document_info_field(-1, "Title")
        with pytest.raises(OfdError):
            document.remove_document_info_field(0, "Title")
        document.set_document_info_field(0, "Title", "Rust & OFD")
        document.add_document_keyword(0, "票据 & 发票")
        document.add_document_keyword(0, "duplicate")
        document.add_document_keyword(0, "duplicate")
        document.remove_document_keyword(0, 1)
        document.remove_document_info_field(0, "Title")
        info = document.documents()[0]
        assert info["title"] is None
        assert info["keywords"] == ["票据 & 发票", "duplicate"]
        document.save(path)
    with Document.open(path) as reopened:
        assert reopened.documents()[0]["keywords"] == ["票据 & 发票", "duplicate"]
        assert reopened.validate_references() == []


def test_reads_invoice_annotations():
    with Document.open(FIXTURES / "发票示例.ofd") as document:
        annotations = document.annotations()
        assert len(annotations) == 2
        assert annotations[0]["type"] == "Watermark"
        assert annotations[0]["appearance_box"] == [90, 8, 30, 20]
        assert annotations[1]["text"] == ["下载次数：1   "]


def test_missing_annotation_index_keeps_page_readable():
    with Document.open(FIXTURES / "keyword.ofd") as document:
        assert document.annotations() == []
        assert "<svg" in document.page_svg(0)
        assert any(
            issue["code"] == "annotation_reference"
            and "Doc_0/Annotations.xml" in issue["message"]
            for issue in document.validate_references()
        )


def test_image_without_resource_id_keeps_page_readable():
    with Document.open(FIXTURES / "path_unstd.ofd") as document:
        svg = document.page_svg(1)
        assert "立案决定书" in svg
        assert "<image " not in svg
        assert any(
            issue["code"] == "missing_media" and "75" in issue["message"]
            for issue in document.validate_references()
        )


def test_missing_image_file_keeps_page_readable():
    path = FIXTURES.parents[3] / "ofdrw-converter" / "src" / "test" / "resources" / "testImageNotFound.ofd"
    with Document.open(path) as document:
        assert any(
            issue["code"] == "media_resource" and "missing entry" in issue["message"]
            for issue in document.validate_references()
        )
        for page_index in range(len(document.pages())):
            assert "<svg" in document.page_svg(page_index)


@pytest.mark.parametrize(
    ("module", "name"),
    [
        ("ofdrw-converter", "V4RideRight.ofd"),
        ("ofdrw-converter", "h.ofd"),
        ("ofdrw-layout", "Page5.ofd"),
        ("ofdrw-sign", "VPage7.ofd"),
    ],
)
def test_missing_public_resource_file_keeps_page_readable(module, name):
    path = FIXTURES.parents[3] / module / "src" / "test" / "resources" / name
    with Document.open(path) as document:
        assert any(
            issue["code"] == "resource_reference"
            and "Doc_0/PublicRes.xml" in issue["message"]
            for issue in document.validate_references()
        )
        for page_index in range(len(document.pages())):
            assert "<svg" in document.page_svg(page_index)


def test_package_root_resource_base_reads_absolute_media_path():
    path = (
        FIXTURES.parents[3]
        / "ofdrw-converter"
        / "src"
        / "test"
        / "resources"
        / "不规范资源路径.ofd"
    )
    with Document.open(path) as document:
        assert document.page_image_data(0, 0).startswith(b"GIF")
        assert "data:image/png;base64," in document.page_svg(0)


def test_image_mask_is_exposed_and_rendered():
    path = Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" / "fixtures" / "image-mask.ofd"
    with Document.open(path) as document:
        assert document.page_images(0)[0]["image_mask"] == "4"
        assert document.validate_references() == []
        assert "data:image/png;base64," in document.page_svg(0)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")


def test_edits_page_image_mask_atomically():
    path = Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" / "fixtures" / "image-mask.ofd"
    with Document.open(path) as document:
        document.set_page_image_mask(0, "5", None)
        assert document.page_images(0)[0]["image_mask"] is None
        document.set_page_image_mask(0, "5", "4")
        assert document.page_images(0)[0]["image_mask"] == "4"
        before = document.entry("Doc_0/Pages/Page_0/Content.xml")
        with pytest.raises(OfdError):
            document.set_page_image_mask(0, "5", "999")
        assert document.entry("Doc_0/Pages/Page_0/Content.xml") == before
        with pytest.raises(IndexError):
            document.set_page_image_mask(-1, "5", "4")


def test_edits_page_image_substitution_atomically():
    path = Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" / "fixtures" / "image-mask.ofd"
    with Document.open(path) as document:
        document.set_page_image_substitution(0, "5", "4")
        assert document.page_images(0)[0]["substitution"] == "4"
        assert document.validate_references() == []
        before = document.entry("Doc_0/Pages/Page_0/Content.xml")
        with pytest.raises(OfdError):
            document.set_page_image_substitution(0, "5", "999")
        assert document.entry("Doc_0/Pages/Page_0/Content.xml") == before
        with pytest.raises(IndexError):
            document.set_page_image_substitution(-1, "5", "4")
        document.set_page_image_substitution(0, "5", None)
        assert document.page_images(0)[0]["substitution"] is None


def test_exports_with_image_substitution():
    path = Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" / "fixtures" / "image-mask.ofd"
    with Document.open(path) as document:
        document.set_page_image_substitution(0, "5", "4")
        assert document.page_svg(0) != document.page_svg(0, prefer_image_substitution=True)
        assert document.page_png(0, 254) != document.page_png(
            0, 254, prefer_image_substitution=True)
        pdf = document.to_pdf(prefer_image_substitution=True)
        assert pdf.startswith(b"%PDF-")
        assert pdf != document.to_pdf()
        with pytest.raises(TypeError):
            document.page_svg(0, prefer_image_substitution=1)
        with pytest.raises(TypeError):
            document.page_png(0, prefer_image_substitution="yes")
        with pytest.raises(TypeError):
            document.to_pdf(prefer_image_substitution=1)


def test_image_border_writes_and_renders_atomically():
    path = Path(__file__).resolve().parents[3] / "crates" / "ofdrw" / "tests" / "fixtures" / "image-mask.ofd"
    with Document.open(path) as document:
        document.set_page_image_border(0, "5", {
            "line_width": 0.4, "color": [0, 255, 0], "dash_pattern": [1, 2, 3],
        })
        border = document.page_images(0)[0]["border"]
        assert border["line_width"] == 0.4
        assert border["dash_pattern"] == [1, 2, 3]
        assert border["color_value"] == "0 255 0"
        assert "BorderColor" in border["color_xml"]
        assert 'stroke="#00ff00"' in document.page_svg(0)
        assert document.page_png(0, 254).startswith(b"\x89PNG")
        before = document.entry("Doc_0/Pages/Page_0/Content.xml")
        with pytest.raises(OfdError):
            document.set_page_image_border(0, "5", {"dash_pattern": [1, 0]})
        assert document.entry("Doc_0/Pages/Page_0/Content.xml") == before
        with pytest.raises(IndexError):
            document.set_page_image_border(-1, "5", {})
        document.set_page_image_border(0, "5", None)
        assert document.page_images(0)[0]["border"] is None
        assert 'stroke="#00ff00"' not in document.page_svg(0)


def test_gouraud_fill_writes_and_rejects_invalid_mesh_atomically():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        object_id = document.add_path_object(
            0, "M 0 0 L 20 0 L 20 20 L 0 20 C", (0, 0, 20, 20), stroke=False,
        )
        style = {"points": [
            {"x": 0, "y": 0, "color": [255, 0, 0]},
            {"x": 20, "y": 0, "color": [0, 255, 0]},
            {"x": 0, "y": 20, "color": [0, 0, 255]},
            {"x": 20, "y": 20, "color": [255, 255, 0]},
        ], "vertices_per_row": 2}
        document.set_page_object_gouraud_fill(0, object_id, style)
        assert "ofd-gouraud-" in document.page_svg(0)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        document.set_page_object_gouraud_stroke(0, object_id, style)
        assert 'stroke="url(#ofd-gouraud-' in document.page_svg(0)
        page_path = "Doc_0/Pages/Page_0/Content.xml"
        before = document.entry(page_path)
        with pytest.raises(OfdError):
            document.set_page_object_gouraud_fill(
                0, object_id, {**style, "vertices_per_row": 3})
        assert document.entry(page_path) == before
        with pytest.raises(OfdError):
            document.set_page_object_gouraud_stroke(
                0, object_id, {**style, "vertices_per_row": 3})
        assert document.entry(page_path) == before
        with pytest.raises(IndexError):
            document.set_page_object_gouraud_fill(-1, object_id, style)
        with pytest.raises(TypeError):
            document.set_page_object_gouraud_fill(0, object_id, None)


def test_axial_and_radial_gradient_fill_writes_atomically():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        object_id = document.add_path_object(
            0, "M 0 0 L 20 0 L 20 20 L 0 20 C", (0, 0, 20, 20), stroke=False,
        )
        style = {
            "kind": "axial", "start_point": [0, 0], "end_point": [20, 0],
            "segments": [{"color": [255, 0, 0]}, {"color": [0, 0, 255]}],
            "map_type": "repeat", "map_unit": 5,
        }
        document.set_page_object_gradient_fill(0, object_id, style)
        svg = document.page_svg(0)
        assert "<linearGradient" in svg and 'spreadMethod="repeat"' in svg
        page_path = "Doc_0/Pages/Page_0/Content.xml"
        before = document.entry(page_path)
        with pytest.raises(OfdError):
            document.set_page_object_gradient_fill(
                0, object_id, {**style, "map_unit": 0})
        assert document.entry(page_path) == before
        radial = {**style, "kind": "radial", "map_type": "direct",
                  "map_unit": None, "start_point": [10, 10],
                  "end_point": [10, 10], "end_radius": 10}
        document.set_page_object_gradient_fill(0, object_id, radial)
        assert "<radialGradient" in document.page_svg(0)
        assert document.page_png(0, 25.4).startswith(b"\x89PNG")
        document.set_page_object_gradient_stroke(0, object_id, style)
        assert 'stroke="url(#ofd-shading-' in document.page_svg(0)
        before = document.entry(page_path)
        with pytest.raises(OfdError):
            document.set_page_object_gradient_stroke(
                0, object_id, {**style, "map_unit": 0})
        assert document.entry(page_path) == before
        with pytest.raises(IndexError):
            document.set_page_object_gradient_fill(-1, object_id, style)
        with pytest.raises(TypeError):
            document.set_page_object_gradient_fill(0, object_id, None)


def test_pattern_fill_and_stroke_keep_document_ids_consistent():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        object_id = document.add_path_object(
            0, "M 0 0 L 20 0 L 20 20 L 0 20 C", (0, 0, 20, 20), stroke=False,
        )
        style = {"width": 4, "height": 4, "x_step": 6, "y_step": 6,
                 "reflect": "row_and_column", "cells": [{
                     "kind": "path", "commands": "M 0 0 L 2 0 L 2 4 L 0 4 C",
                     "boundary": [0, 0, 2, 4], "fill": True, "stroke": False,
                     "fill_color": [255, 0, 0],
                 }]}
        document.set_page_object_pattern_fill(0, object_id, style)
        document.set_page_object_pattern_stroke(0, object_id, style)
        assert len(document.page_paths(0)) == 1
        assert document.page_layers(0)[0]["object_count"] == 1
        paints = document.page_object_paints(0)
        assert len(paints) == 1
        assert paints[0]["fill"]["kind"] == "Pattern"
        assert paints[0]["stroke"]["kind"] == "Pattern"
        assert "CellContent" in paints[0]["fill"]["xml"]
        svg = document.page_svg(0)
        assert 'fill="url(#ofd-pattern-' in svg
        assert 'stroke="url(#ofd-pattern-' in svg
        assert document.validate_references() == []
        page = "Doc_0/Pages/Page_0/Content.xml"
        before = document.entry(page)
        with pytest.raises(OfdError):
            document.set_page_object_pattern_fill(0, object_id, {**style, "width": 0})
        assert document.entry(page) == before
        with pytest.raises(IndexError):
            document.set_page_object_pattern_stroke(-1, object_id, style)
        with pytest.raises(TypeError):
            document.set_page_object_pattern_fill(0, object_id, None)


def test_pattern_cells_use_existing_font_image_and_composite_resources():
    with Document.open(FIXTURES / "发票示例.ofd") as source:
        png = source.page_image_data(0, 0)
    with Document.new() as document:
        document.add_text_page("source", 30, 30)
        image_id = document.add_image_object(0, png, "png", (0, 0, 3, 3))
        media_id = document.page_images(0)[0]["resource_id"]
        document.remove_page_object(0, image_id)
        composite_id = document.add_composite_path_object(
            0, "M 0 0 L 3 0 L 3 3 L 0 3 C", (0, 0, 3, 3),
            fill=True, stroke=False, fill_color=(0, 0, 255),
        )
        composite_resource_id = document.page_composite_objects(0)[0]["resource_id"]
        document.remove_page_object(0, composite_id)
        target = document.add_path_object(
            0, "M 0 0 L 30 0 L 30 30 L 0 30 C", (0, 0, 30, 30), stroke=False,
        )
        style = {"width": 10, "height": 10, "cells": [
            {"kind": "text", "text": "X", "boundary": [0, 0, 3, 3], "size": 2},
            {"kind": "image", "resource_id": media_id, "boundary": [3, 0, 3, 3]},
            {"kind": "composite", "resource_id": composite_resource_id,
             "boundary": [6, 0, 3, 3]},
        ]}
        document.set_page_object_pattern_fill(0, target, style)
        assert document.page_text(0) == ["source"]
        assert document.page_text_with_templates(0) == ["source"]
        assert document.page_text_in_region(0, 0, 0, 30, 30) == ["source"]
        assert document.keyword_positions(["X"]) == []
        assert len(document.page_text_objects(0)) == 1
        assert document.page_images(0) == []
        assert document.page_composite_objects(0) == []
        assert document.page_composite_images(0) == []
        svg = document.page_svg(0)
        assert "<pattern" in svg and "<image" in svg and "<text" in svg
        assert document.validate_references() == []
        before = document.entry("Doc_0/Pages/Page_0/Content.xml")
        style["cells"][1]["resource_id"] = "missing"
        with pytest.raises(OfdError):
            document.set_page_object_pattern_fill(0, target, style)
        assert document.entry("Doc_0/Pages/Page_0/Content.xml") == before
        document.set_page_object_pattern_fill(
            0, target, {"width": 10, "height": 10, "thumbnail": media_id}
        )
        content = document.entry("Doc_0/Pages/Page_0/Content.xml").decode()
        assert f'Thumbnail="{media_id}"' in content
        assert "<image" in document.page_svg(0)
        assert document.validate_references() == []
        before = document.entry("Doc_0/Pages/Page_0/Content.xml")
        with pytest.raises(OfdError):
            document.set_page_object_pattern_fill(
                0, target, {"width": 10, "height": 10, "thumbnail": "missing"}
            )
        assert document.entry("Doc_0/Pages/Page_0/Content.xml") == before


def test_replaces_page_path_data_atomically():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        object_id = document.add_path_object(
            0, "M 0 0 L 20 0", (0, 0, 20, 20), stroke=True,
            stroke_color=(255, 0, 0),
        )
        document.set_page_path_data(0, object_id, "M 0 0 Q 10 20 20 0")
        assert document.page_paths(0)[0]["abbreviated_data"] == "M 0 0 Q 10 20 20 0"
        assert 'd="M 0 0 Q 10 20 20 0"' in document.page_svg(0)
        assert 'stroke="#ff0000"' in document.page_svg(0)
        path = document.pages()[0]["path"]
        before = document.entry(path)
        with pytest.raises(OfdError):
            document.set_page_path_data(0, object_id, "M 0 0 X 1 1")
        with pytest.raises(ValueError):
            document.set_page_path_data(0, object_id, "M 0 0 L 1 1\x00X 2 2")
        assert document.entry(path) == before
        with pytest.raises(OfdError):
            document.set_page_path_data(0, "missing", "M 0 0 L 1 1")
        assert document.entry(path) == before


def test_sets_page_path_stroke_style_atomically():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        path_id = document.add_path_object(0, "M 0 10 L 20 10", (0, 0, 20, 20))
        document.set_page_path_stroke_style(0, path_id, {
            "line_width": 2, "cap": "round", "join": "bevel",
            "miter_limit": 5, "dash_pattern": [3, 2], "dash_offset": 1,
        })
        style = document.page_object_stroke_styles(0)[0]["style"]
        assert style["line_width"] == 2
        assert style["cap"] == "round"
        assert style["join"] == "bevel"
        assert style["dash_pattern"] == [3, 2]
        assert 'stroke-width="2"' in document.page_svg(0)
        document.set_page_path_stroke_style(0, path_id, {
            "line_width": 3, "dash_sequence": [1, 2, 3, 4], "dash_offset": 0.5,
        })
        style = document.page_object_stroke_styles(0)[0]["style"]
        assert style["line_width"] == 3
        assert style["dash_sequence"] == [1, 2, 3, 4]
        assert style["dash_pattern"] is None
        before = document.entry(document.pages()[0]["path"])
        with pytest.raises(OfdError):
            document.set_page_path_stroke_style(0, path_id, {
                "line_width": 4, "dash_pattern": [1, 2], "dash_sequence": [3],
            })
        assert document.entry(document.pages()[0]["path"]) == before
        with pytest.raises(OfdError):
            document.set_page_path_stroke_style(0, path_id, {
                "line_width": 1, "dash_pattern": [0, 2],
            })
        assert document.entry(document.pages()[0]["path"]) == before
        document.set_page_path_stroke_style(0, path_id, {"line_width": 0.4})
        assert document.page_object_stroke_styles(0)[0]["style"]["dash_pattern"] is None


def test_sets_full_path_dash_sequence_across_binding():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        path_id = document.add_path_object(0, "M 0 10 L 20 10", (0, 0, 20, 20), stroke=True)
        document.set_page_path_dash_sequence(0, path_id, [1, 2, 3, 4], 0.5)
        style = document.page_object_stroke_styles(0)[0]["style"]
        assert style["dash_sequence"] == [1, 2, 3, 4]
        assert style["dash_offset"] == 0.5
        assert 'stroke-dasharray="1 2 3 4"' in document.page_svg(0)
        path = document.pages()[0]["path"]
        before = document.entry(path)
        with pytest.raises(OfdError):
            document.set_page_path_dash_sequence(0, path_id, [1, 0, 3])
        assert document.entry(path) == before
        document.set_page_path_dash_sequence(0, path_id, None)
        assert document.page_object_stroke_styles(0)[0]["style"]["dash_sequence"] is None


def test_creates_path_and_draw_param_with_full_dash_sequences():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        path_id = document.add_path_object(
            0, "M 0 10 L 20 10", (0, 0, 20, 20),
            dash_sequence=[1, 2, 3],
        )
        assert document.page_object_stroke_styles(0)[0]["style"]["dash_sequence"] == [1, 2, 3]
        path = document.pages()[0]["path"]
        before = document.entry(path)
        with pytest.raises(OfdError):
            document.add_path_object(
                0, "M 0 10 L 20 10", (0, 0, 20, 20),
                dash_pattern=(1, 2), dash_sequence=[3],
            )
        assert document.entry(path) == before
        draw_id = document.add_document_draw_param(0, {
            "dash_sequence": [4, 1, 2, 1],
        })
        document.set_page_path_dash_sequence(0, path_id, None)
        document.set_page_object_draw_param(0, path_id, draw_id)
        assert document.page_object_stroke_styles(0)[0]["style"]["dash_sequence"] == [4, 1, 2, 1]
        assert 'stroke-dasharray="4 1 2 1"' in document.page_svg(0)


def test_reads_full_path_dash_sequence_across_binding():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        document.add_path_object(0, "M 0 10 L 20 10", (0, 0, 20, 20))
        path = document.pages()[0]["path"]
        source = document.entry(path)
        document.set_entry(path, source.replace(
            b"<ofd:PathObject ",
            b'<ofd:PathObject DashPattern="1 2 3 4" DashOffset="0.5" ',
            1,
        ))
        style = document.page_object_stroke_styles(0)[0]["style"]
        assert style["dash_pattern"] is None
        assert style["dash_sequence"] == [1, 2, 3, 4]
        assert 'stroke-dasharray="1 2 3 4"' in document.page_svg(0)


def test_writes_reusable_draw_params_and_assigns_them_to_objects():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        object_id = document.add_path_object(
            0, "M 0 0 L 20 0", (0, 0, 20, 20), stroke=True,
        )
        base = document.add_document_draw_param(0, {
            "stroke": {"color": [255, 0, 0], "alpha": 200},
            "line_width": 1.5,
            "cap": "Round", "join": "Bevel", "dash_pattern": [3, 2],
            "dash_offset": 1,
        })
        derived = document.add_document_draw_param(0, {
            "relative": str(base), "fill": {"color": [0, 0, 255]},
        })
        document.set_page_object_draw_param(0, object_id, derived)
        paint = document.page_object_paints(0)[0]
        assert paint["effective_stroke_draw_param"] == str(base)
        assert paint["effective_stroke"]["alpha"] == 200
        assert paint["effective_fill_draw_param"] == str(derived)
        stroke = document.page_object_stroke_styles(0)[0]
        assert stroke["stroke_enabled"] is True
        assert stroke["style"]["cap"] == "round"
        assert stroke["style"]["join"] == "bevel"
        assert stroke["style"]["dash_pattern"] == [3, 2]
        assert stroke["style"]["dash_offset"] == 1
        assert "stroke=\"#ff0000\"" in document.page_svg(0)
        before = document.entry("Doc_0/PublicRes.xml")
        with pytest.raises(OfdError):
            document.add_document_draw_param(0, {"relative": "999999"})
        assert document.entry("Doc_0/PublicRes.xml") == before
        with pytest.raises(OfdError):
            document.set_page_object_draw_param(0, object_id, "999999")
        document.set_page_object_draw_param(0, object_id, None)
        assert document.page_object_paints(0)[0]["effective_stroke"] is None
        assert document.page_object_stroke_styles(0)[0]["style"]["dash_pattern"] is None
        with pytest.raises(IndexError):
            document.add_document_draw_param(-1, {})


def test_page_draw_param_is_local_and_inherits_public_style():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        document.append_blank_page(0, 20, 20)
        first = document.add_path_object(
            0, "M 0 0 L 10 0 L 10 10 C", (0, 0, 10, 10), fill=True, stroke=False,
        )
        second = document.add_path_object(
            1, "M 0 0 L 10 0 L 10 10 C", (0, 0, 10, 10), fill=True, stroke=False,
        )
        public = document.add_document_draw_param(0, {
            "fill": {"color": [255, 0, 0]},
        })
        local = document.add_page_draw_param(0, {
            "relative": str(public), "fill": {"color": [0, 255, 0]},
        })
        assert [item["id"] for item in document.document_draw_params(0)] == [str(public)]
        page_params = document.page_draw_params(0)
        assert {item["id"] for item in page_params} == {str(public), str(local)}
        local_info = next(item for item in page_params if item["id"] == str(local))
        assert local_info["relative"] == str(public)
        assert "<ofd:DrawParam" in local_info["xml"]
        assert local_info["resource_path"].endswith(f"RustPageDrawParams_{local}.xml")
        assert [item["id"] for item in document.page_draw_params(1)] == [str(public)]
        with pytest.raises(IndexError):
            document.page_draw_params(-1)
        with pytest.raises(IndexError):
            document.document_draw_params(-1)
        document.set_page_object_draw_param(0, first, local)
        document.set_page_object_draw_param(1, second, public)
        document.set_document_draw_param(0, public, {"fill": {"color": [0, 0, 255]}})
        document.set_page_draw_param(0, local, {
            "relative": str(public), "fill": {"color": [0, 255, 0]},
        })
        assert 'fill="#0000ff"' in document.page_svg(1)
        assert 'fill="#00ff00"' in document.page_svg(0)
        before = document.to_bytes()
        with pytest.raises(OfdError):
            document.set_page_draw_param(0, local, {"relative": str(local)})
        with pytest.raises(OfdError):
            document.set_page_draw_param(1, local, {})
        with pytest.raises(OfdError):
            document.set_document_draw_param(0, local, {})
        with pytest.raises(OfdError):
            document.set_page_object_draw_param(1, second, local)
        with pytest.raises(OfdError):
            document.add_page_draw_param(1, {"relative": str(local)})
        with pytest.raises(IndexError):
            document.add_page_draw_param(-1, {})
        assert document.to_bytes() == before
        assert document.validate_references() == []
        with pytest.raises(OfdError):
            document.remove_page_draw_param(0, local)
        with pytest.raises(OfdError):
            document.remove_document_draw_param(0, public)
        assert document.to_bytes() == before
        document.set_page_object_draw_param(0, first, None)
        document.set_page_object_draw_param(1, second, None)
        document.remove_page_draw_param(0, local)
        document.remove_document_draw_param(0, public)
        assert document.page_draw_params(0) == []


def test_draw_param_gradient_and_gouraud_paints():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        object_id = document.add_path_object(
            0, "M 0 0 L 20 0 L 20 20 L 0 20 C", (0, 0, 20, 20),
            fill=True, stroke=False,
        )
        gradient = {
            "kind": "axial", "start_point": [0, 0], "end_point": [20, 0],
            "segments": [{"color": [255, 0, 0]}, {"color": [0, 0, 255]}],
        }
        draw_param = document.add_document_draw_param(0, {
            "fill_gradient": gradient,
        })
        document.set_page_object_draw_param(0, object_id, draw_param)
        assert document.page_object_paints(0)[0]["effective_fill"]["kind"] == "Axial"
        assert "linearGradient" in document.page_svg(0)

        mesh = {"points": [
            {"x": 0, "y": 0, "color": [255, 0, 0]},
            {"x": 20, "y": 0, "color": [0, 255, 0]},
            {"x": 0, "y": 20, "color": [0, 0, 255]},
        ]}
        mesh_id = document.add_document_draw_param(0, {"fill_gouraud": mesh})
        document.set_page_object_draw_param(0, object_id, mesh_id)
        assert document.page_object_paints(0)[0]["effective_fill"]["kind"] == "Gouraud"
        assert document.page_png(0, 25.4)
        before = document.entry("Doc_0/PublicRes.xml")
        with pytest.raises(OfdError):
            document.add_document_draw_param(0, {
                "fill": {"color": [1, 2, 3]}, "fill_gradient": gradient,
            })
        assert document.entry("Doc_0/PublicRes.xml") == before


def test_draw_param_pattern_paint_reuses_cell_content():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        ids = [document.add_path_object(
            0, "M 0 0 L 8 0 L 8 8 L 0 8 C", (x, 0, 8, 8),
            fill=True, stroke=False,
        ) for x in (0, 10)]
        pattern = {"width": 4, "height": 4, "cells": [{
            "kind": "path", "commands": "M 0 0 L 4 0 L 4 4 L 0 4 C",
            "boundary": [0, 0, 4, 4], "fill": True, "stroke": False,
            "fill_color": [255, 0, 0],
        }]}
        draw_param = document.add_document_draw_param(0, {"fill_pattern": pattern})
        for object_id in ids:
            document.set_page_object_draw_param(0, object_id, draw_param)
        paints = document.page_object_paints(0)
        assert len(paints) == 2
        assert all(item["effective_fill"]["kind"] == "Pattern" for item in paints)
        assert all("CellContent" in item["effective_fill"]["xml"] for item in paints)
        assert document.page_png(0, 25.4)
        before = document.entry("Doc_0/PublicRes.xml")
        with pytest.raises(OfdError):
            document.add_document_draw_param(0, {"fill_pattern": {
                "width": 4, "height": 4, "cells": [{
                    "kind": "image", "resource_id": "999999",
                    "boundary": [0, 0, 4, 4],
                }],
            }})
        assert document.entry("Doc_0/PublicRes.xml") == before


def test_draw_param_thumbnail_pattern_uses_document_image_resource():
    with Document.open(FIXTURES / "发票示例.ofd") as source:
        png = source.page_image_data(0, 0)
    with Document.new() as document:
        document.add_blank_page(20, 20)
        media_id = document.add_multimedia_resource(0, "Image", "PNG", png)
        path_id = document.add_path_object(
            0, "M 0 0 L 20 0 L 20 20 L 0 20 C", (0, 0, 20, 20),
            fill=True, stroke=False,
        )
        draw_param = document.add_document_draw_param(0, {"fill_pattern": {
            "width": 4, "height": 4, "thumbnail": str(media_id),
        }})
        document.set_page_object_draw_param(0, path_id, draw_param)
        assert f'Thumbnail="{media_id}"' in document.entry("Doc_0/PublicRes.xml").decode()
        assert "<image" in document.page_svg(0)
        assert document.validate_references() == []
        before = document.entry("Doc_0/PublicRes.xml")
        with pytest.raises(OfdError):
            document.add_document_draw_param(0, {"fill_pattern": {
                "width": 4, "height": 4, "thumbnail": "missing",
            }})
        assert document.entry("Doc_0/PublicRes.xml") == before


def test_page_object_paints_resolve_draw_param_precedence():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        object_id = document.add_path_object(
            0, "M 0 0 L 20 0", (0, 0, 20, 20), stroke=True,
        )
        resource_path = "Doc_0/PublicRes.xml"
        resources = document.entry(resource_path).decode()
        params = (
            '<ofd:DrawParams>'
            '<ofd:DrawParam ID="100"><ofd:StrokeColor Value="255 0 0"/></ofd:DrawParam>'
            '<ofd:DrawParam ID="101" Relative="100"><ofd:FillColor Value="0 0 255"/></ofd:DrawParam>'
            '</ofd:DrawParams>'
        )
        document.set_entry(resource_path, resources.replace(
            "</ofd:Res>", params + "</ofd:Res>").encode())
        page_path = "Doc_0/Pages/Page_0/Content.xml"
        page = document.entry(page_path).decode().replace(
            f'<ofd:PathObject ID="{object_id}"',
            f'<ofd:PathObject ID="{object_id}" DrawParam="101"',
        )
        document.set_entry(page_path, page.encode())
        paint = document.page_object_paints(0)[0]
        assert paint["stroke"] is None
        assert paint["effective_stroke"]["value"] == "255 0 0"
        assert paint["effective_stroke_draw_param"] == "100"
        assert paint["effective_fill"]["value"] == "0 0 255"
        assert paint["effective_fill_draw_param"] == "101"

        document.set_page_object_solid_stroke(0, object_id, {
            "color": [0, 255, 0],
        })
        paint = document.page_object_paints(0)[0]
        assert paint["stroke"]["value"] == "0 255 0"
        assert paint["effective_stroke"] == paint["stroke"]
        assert paint["effective_stroke_draw_param"] is None


def test_solid_paint_replaces_gradient_and_sets_stroke_atomically():
    with Document.new() as document:
        document.add_blank_page(20, 20)
        object_id = document.add_path_object(
            0, "M 0 0 L 20 0 L 20 20 L 0 20 C", (0, 0, 20, 20), stroke=False,
        )
        document.set_page_object_gradient_fill(0, object_id, {
            "kind": "axial", "start_point": [0, 0], "end_point": [20, 0],
            "segments": [{"color": [255, 0, 0]}, {"color": [0, 0, 255]}],
        })
        document.set_page_object_solid_fill(0, object_id, {
            "color": [10, 20, 30], "alpha": 128,
        })
        document.set_page_object_solid_stroke(0, object_id, {
            "color": [0, 255, 0],
        })
        paints = document.page_object_paints(0)
        assert len(paints) == 1
        assert paints[0]["id"] == str(object_id)
        assert paints[0]["fill_enabled"] and paints[0]["stroke_enabled"]
        assert paints[0]["fill"]["kind"] == "Solid"
        assert paints[0]["fill"]["alpha"] == 128
        assert paints[0]["fill"]["value"] == "10 20 30"
        assert paints[0]["stroke"]["kind"] == "Solid"
        assert "StrokeColor" in paints[0]["stroke"]["xml"]
        with pytest.raises(IndexError):
            document.page_object_paints(-1)
        page = "Doc_0/Pages/Page_0/Content.xml"
        xml = document.entry(page).decode()
        assert "AxialShd" not in xml
        assert 'FillColor Value="10 20 30" Alpha="128"' in xml
        assert 'StrokeColor Value="0 255 0" Alpha="255"' in xml
        svg = document.page_svg(0)
        assert 'fill="#0a141e"' in svg and 'stroke="#00ff00"' in svg
        before = document.entry(page)
        with pytest.raises(OfdError):
            document.set_page_object_solid_fill(0, object_id, {"color": [256, 0, 0]})
        assert document.entry(page) == before
        with pytest.raises(IndexError):
            document.set_page_object_solid_stroke(-1, object_id, {"color": [0, 0, 0]})
        with pytest.raises(TypeError):
            document.set_page_object_solid_fill(0, object_id, None)


def test_signed_page_rejects_object_attribute_changes():
    path = FIXTURES.parents[3] / "ofdrw-sign/src/test/resources/signedFile.ofd"
    with Document.open(path) as document:
        assert document.signatures()
        page_path = "Doc_0/Pages/Page_0/Content.xml"
        before = document.entry(page_path)
        with pytest.raises(OfdError, match="signed"):
            document.set_page_object_visible(0, "62", False)
        with pytest.raises(OfdError, match="signed"):
            document.set_page_object_alpha(0, "62", 128)
        with pytest.raises(OfdError, match="signed"):
            document.set_page_object_boundary(0, "62", (1, 1, 10, 10))
        with pytest.raises(OfdError, match="signed"):
            document.set_page_object_ctm(0, "62", (1, 0, 0, 1, 2, 3))
        with pytest.raises(OfdError, match="signed"):
            document.add_path_object(0, "M 0 0 L 5 5", (1, 1, 5, 5))
        with pytest.raises(OfdError, match="signed"):
            document.add_page_layer(0, "Body")
        with pytest.raises(OfdError, match="signed"):
            document.move_page_object_to_layer(0, "62", "60")
        with pytest.raises(OfdError, match="signed"):
            document.move_page_object(0, "62", "64")
        with pytest.raises(OfdError, match="signed"):
            document.remove_page_layer(0, "60")
        with pytest.raises(OfdError, match="signed"):
            document.remove_page_object(0, "62")
        with pytest.raises(OfdError, match="signed"):
            document.set_page_area(0, (0, 0, 210, 297))
        with pytest.raises(OfdError, match="signed"):
            document.append_blank_page(0, 10, 10)
        with pytest.raises(OfdError, match="signed"):
            document.add_attachment_to_document(0, "x.txt", b"x")
        with pytest.raises(OfdError, match="signed"):
            document.remove_attachment(0)
        assert document.entry(page_path) == before


def test_reads_embedded_font():
    with Document.open(FIXTURES / "keyword.ofd") as document:
        font = document.fonts()[0]
        assert font["font_name"] == "宋体"
        assert font["embedded_path"] == "Doc_0/Res/Font7.ttf"
        assert len(document.font_data(0)) == 244_104
        assert document.page_fonts(0)[0]["embedded_path"] == font["embedded_path"]
        assert document.page_font_data(0, 0) == document.font_data(0)
        with pytest.raises(IndexError):
            document.page_fonts(-1)
        with pytest.raises(IndexError):
            document.page_font_data(0, -1)
        with pytest.raises(OfdError, match="out of bounds"):
            document.page_font_data(1, 0)
        with pytest.raises(IndexError):
            document.font_data(-1)
        with pytest.raises(OfdError, match="out of bounds"):
            document.font_data(1)


def test_embedded_font_reference_uses_unique_archive_case():
    with Document.open(FIXTURES / "keyword.ofd") as document:
        original_font = document.fonts()[0]
        resource_path = original_font["resource_path"]
        resource = document.entry(resource_path)
        updated = resource.replace(b"Font7.ttf", b"font7.TTF")
        assert updated != resource
        document.set_entry(resource_path, updated)
        font = document.fonts()[0]
        assert font["embedded_path"] == "Doc_0/Res/Font7.ttf"
        assert len(document.font_data(0)) == 244_104


def test_page_local_font_metadata_and_bytes():
    font = (
        Path(__file__).resolve().parents[3]
        / "crates"
        / "ofdrw"
        / "tests"
        / "fixtures"
        / "dejavu-sans.ttf"
    ).read_bytes()
    with Document.new() as document:
        document.add_text_page("Hello", 30, 20)
        path = document.pages()[0]["path"]
        page_dir = path.rsplit("/", 1)[0]
        document.set_entry(page_dir + "/Res/local.ttf", font)
        document.set_entry(
            page_dir + "/PageRes.xml",
            b'<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="Res"><ofd:Fonts><ofd:Font ID="1" FontName="DejaVu Sans"><ofd:FontFile>local.ttf</ofd:FontFile></ofd:Font></ofd:Fonts></ofd:Res>',
        )
        document.set_entry(
            path,
            document.entry(path).replace(
                b"<ofd:Content>",
                b"<ofd:PageRes>PageRes.xml</ofd:PageRes><ofd:Content>",
            ),
        )
        page_font = document.page_fonts(0)[0]
        assert page_font["font_name"] == "DejaVu Sans"
        assert page_font["resource_path"] == page_dir + "/PageRes.xml"
        assert document.page_font_data(0, 0) == font
        assert document.fonts()[0]["font_name"] != "DejaVu Sans"
        replacement = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
                       "tests" / "fixtures" / "dejavu-serif.ttf").read_bytes()
        document.replace_page_font_data(0, 0, replacement)
        assert document.page_font_data(0, 0) == replacement
        before_data = document.to_bytes()
        with pytest.raises(OfdError, match="font format"):
            document.replace_page_font_data(0, 0, b"invalid")
        with pytest.raises(OfdError, match="truncated font"):
            document.replace_page_font_data(0, 0, b"\x00\x01\x00\x00")
        assert document.to_bytes() == before_data
        document.set_page_font_attribute(0, 0, "FamilyName", "Page & Local")
        assert document.page_fonts(0)[0]["family_name"] == "Page & Local"
        assert document.fonts()[0]["family_name"] != "Page & Local"
        before = document.to_bytes()
        with pytest.raises(OfdError, match="Charset"):
            document.set_page_font_attribute(0, 0, "Charset", "invalid")
        assert document.to_bytes() == before
        document.add_text_object(0, "fi", (1, 1, 10, 5), 3, font_id=1, shape=True)
        assert "fi" in document.page_text(0)


def test_embeds_font_only_in_selected_page():
    font = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
            "tests" / "fixtures" / "dejavu-sans.ttf").read_bytes()
    with Document.new() as document:
        document.add_blank_page(40, 30)
        document.add_blank_page(40, 30)
        font_id = document.add_embedded_font_to_page(0, "Page & Font", font)
        assert document.page_fonts(0)[0]["id"] == str(font_id)
        assert document.page_font_data(0, 0) == font
        assert all(item["id"] != str(font_id) for item in document.page_fonts(1))
        before = document.to_bytes()
        with pytest.raises(OfdError, match="font format"):
            document.add_embedded_font_to_page(0, "bad", b"invalid")
        assert document.to_bytes() == before
        with Document.from_bytes(before) as reopened:
            assert reopened.page_font_data(0, 0) == font
            assert reopened.validate_references() == []


def test_writes_embedded_font(tmp_path):
    with Document.open(FIXTURES / "keyword.ofd") as source:
        font_bytes = source.font_data(0)
    path = tmp_path / "font.ofd"
    with Document.new() as document:
        with pytest.raises(OfdError, match="font format"):
            document.add_embedded_font("bad", b"bad")
        font_id = document.add_embedded_font("宋体", font_bytes)
        with pytest.raises(ValueError):
            document.add_text_page_with_font("bad", -1)
        document.add_text_page_with_font("字体测试", font_id)
        document.save(path)
    with Document.open(path) as document:
        assert document.page_text_objects(0)[0]["font_id"] == str(font_id)
        assert document.font_data(1) == font_bytes


def test_standalone_svg_embeds_font_bytes():
    font = (
        Path(__file__).resolve().parents[3]
        / "crates" / "ofdrw" / "tests" / "fixtures" / "dejavu-serif.ttf"
    ).read_bytes()
    with Document.new() as document:
        font_id = document.add_embedded_font("serif, fallback", font)
        document.add_text_page_with_font("standalone SVG", font_id, 60, 30)
        svg = document.page_svg(0)
        assert "@font-face" in svg
        assert "data:font/ttf;base64," in svg
        assert 'font-family="ofd-embedded-font-0"' in svg
        assert 'font-family="serif, fallback"' not in svg


def test_edits_font_substitution_attributes_across_binding():
    font_bytes = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
                  "tests" / "fixtures" / "dejavu-serif.ttf").read_bytes()
    with Document.new() as document:
        font_id = document.add_embedded_font("DejaVu Serif", font_bytes)
        index = next(i for i, font in enumerate(document.fonts()) if font["id"] == str(font_id))
        document.set_font_attribute(index, "FamilyName", "A & B")
        document.set_font_attribute(index, "Charset", "SHIFT_JIS")
        document.set_font_attribute(index, "Bold", "true")
        assert document.fonts()[index]["family_name"] == "A & B"
        assert document.fonts()[index]["attributes"]["Charset"] == "shift-jis"
        before = document.to_bytes()
        with pytest.raises(OfdError, match="Charset"):
            document.set_font_attribute(index, "Charset", "invalid")
        assert document.to_bytes() == before
        document.set_font_attribute(index, "Bold", None)
        replacement = (Path(__file__).resolve().parents[3] / "crates" / "ofdrw" /
                       "tests" / "fixtures" / "dejavu-sans.ttf").read_bytes()
        document.replace_font_data(index, replacement)
        with pytest.raises(OfdError, match="font format"):
            document.replace_font_data(index, b"invalid")
        with pytest.raises(OfdError, match="truncated font"):
            document.replace_font_data(index, b"\x00\x01\x00\x00")
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.fonts()[index]["family_name"] == "A & B"
            assert "Bold" not in reopened.fonts()[index]["attributes"]
            assert reopened.font_data(index) == replacement


def test_embeds_font_in_existing_document(tmp_path):
    with Document.open(FIXTURES / "keyword.ofd") as source:
        font_bytes = source.font_data(0)
    path = tmp_path / "existing-font.ofd"
    with Document.open(FIXTURES / "helloworld.ofd") as document:
        with pytest.raises(IndexError):
            document.add_embedded_font_to_document(-1, "font", font_bytes)
        with pytest.raises(OfdError, match="font format"):
            document.add_embedded_font_to_document(0, "font", b"bad")
        font_id = document.add_embedded_font_to_document(0, "Rust 字体", font_bytes)
        assert document.fonts()[-1]["id"] == str(font_id)
        assert document.font_data(1) == font_bytes
        with pytest.raises(IndexError):
            document.append_text_page_with_font(-1, "bad", font_id)
        with pytest.raises(ValueError):
            document.append_text_page_with_font(0, "bad", -1)
        with pytest.raises(OfdError, match="not defined"):
            document.append_text_page_with_font(0, "bad", 999)
        document.append_text_page_with_font(0, "selected font", font_id, 80, 60)
        assert document.page_text_objects(1)[0]["font_id"] == str(font_id)
        document.save(path)
    with Document.open(path) as document:
        assert document.fonts()[-1]["id"] == str(font_id)
        assert document.font_data(1) == font_bytes
        assert document.page_text(1) == ["selected font"]
        assert document.validate_references() == []


@pytest.mark.parametrize("name", [
    'separation-paths',
    'separation-cmyk',
    'separation-sampled',
    'separation-stitching',
    'separation-calculator',
    'separation-image',
    'separation-image16-mask',
    'separation-jpeg',
    'separation-indexed',
    'separation-all',
    'separation-none',
    'separation-none-clipping',
    'separation-none-independent',
    'separation-none-text-clip',
    'separation-shading',
    'separation-mesh',
    'separation-patch',
    'separation-plane',
])

def test_imports_pdf_separation_colors_across_binding(name):
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / (name + ".pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        if name == "separation-none":
            assert document.page_text(0) == ["none"]
            assert document.page_images(0) == []
        png = document.page_png(0, 72)
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.page_png(0, 72) == png
            assert reopened.validate_references() == []


@pytest.mark.parametrize("name", [
    'devicen-jpx-two',
    'devicen-jpx-five',
    'devicen-jpx-thirty-two',
    'devicen-jpx-five-mask',
    'devicen-jpx-five-sixteen-mask',
    'devicen-jpx-five-soft-mask',
    'devicen-jpx-five-no-bpc',
    'devicen-jpx-four',
    'devicen-jpx-three-alpha',
    'devicen-jpx-five-container',
    'devicen-paths',
    'devicen-sampled',
    'devicen-cubic',
    'devicen-cmyk',
    'devicen-three-input',
    'devicen-image',
    'devicen-image-five',
    'devicen-image-thirty-two',
    'devicen-image16-mask',
    'devicen-indexed',
    'devicen-none',
    'devicen-mixed-none',
    'devicen-none-clipping',
    'devicen-nchannel',
    'devicen-shading',
    'devicen-mesh',
    'devicen-patch',
    'devicen-plane',
])
def test_imports_pdf_device_n_colors_across_binding(name):
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / (name + ".pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        if name == "devicen-none":
            assert document.page_text(0) == ["none"]
            assert document.page_images(0) == []
        png = document.page_png(0, 72)
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.page_png(0, 72) == png
            assert reopened.validate_references() == []


@pytest.mark.parametrize("kind", ['xobject', 'form', 'inline', 'inline-embedded-ei', 'inline-trailing-cr', 'inline-flate', 'form-inline', 'masked'])
def test_pdf_image_intent_inheritance_across_binding(kind):
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / ("image-intent-" + kind + ".pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert len(document.page_images(0)) == 5
        colors = [document.page_image_data(0, index) for index in range(5)]
        assert colors[0] == colors[2]
        assert colors[0] != colors[1]
        assert colors[0] != colors[3]
        if kind != "masked":
            assert colors[1] == colors[4]
        png = document.page_png(0, 72)
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.page_png(0, 72) == png
            assert reopened.validate_references() == []


@pytest.mark.parametrize("kind", ["colored", "uncolored-rgb", "nested", "mask", "cell-text", "state-reuse"])
def test_pdf_tiling_patterns_across_binding(kind):
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / ("tiling-" + kind + ".pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert document.page_count == 1
        assert "<pattern" in document.page_svg(0)
        assert document.page_composite_units(0)
        if kind == "state-reuse":
            assert len(document.page_composite_units(0)) == 1
        png = document.page_png(0, 72)
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.page_png(0, 72) == png
            assert reopened.validate_references() == []


@pytest.mark.parametrize("kind", ["axial", "affine-radial", "bbox-background", "combined-alpha", "nested", "tensor", "state-reuse", "indirect-params"])
def test_pdf_shading_patterns_across_binding(kind):
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / ("shading-pattern-" + kind + ".pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert document.page_count == 1
        assert "<pattern" in document.page_svg(0)
        assert document.page_composite_units(0)
        if kind == "state-reuse":
            assert len(document.page_composite_units(0)) == 1
        png = document.page_png(0, 72)
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.page_png(0, 72) == png
            assert reopened.validate_references() == []


@pytest.mark.parametrize("kind", ["axial-shear", "radial-shear", "axial-alpha", "radial-clip", "axial-form", "radial-multiply", "radial-rotate"])
def test_pdf_direct_affine_shadings_across_binding(kind):
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / ("affine-shading-" + kind + ".pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert document.page_count == 1
        assert "<pattern" in document.page_svg(0)
        assert document.page_composite_units(0)
        png = document.page_png(0, 72)
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.page_png(0, 72) == png
            assert reopened.validate_references() == []

@pytest.mark.parametrize("kind", ["grow", "equal", "shrink-both", "affine", "pattern-background", "nested-pattern", "calculator-step"])
def test_pdf_general_radial_shadings_across_binding(kind):
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / ("general-radial-" + kind + ".pdf")
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert document.page_count == 1
        assert "<image" in document.page_svg(0)
        png = document.page_png(0, 72)
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.page_png(0, 72) == png
            assert reopened.validate_references() == []
