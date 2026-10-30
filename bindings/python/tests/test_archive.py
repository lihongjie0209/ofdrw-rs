from pathlib import Path
import re

from ofdrw_rs import Document


ROOT = Path(__file__).resolve().parents[4]


def test_prepare_archive_removes_view_preferences_through_python_binding():
    with Document.new() as source:
        source.add_blank_page(40, 30)
        path = "Doc_0/Document.xml"
        source.set_entry(path, source.entry(path).replace(
            b"</ofd:Document>",
            b"<ofd:VPreferences><ofd:PageMode>UseOutlines</ofd:PageMode>"
            b"</ofd:VPreferences></ofd:Document>",
        ))
        before = source.to_bytes()
        assert any(issue["rule"] == "V_PREFERENCES" for issue in source.archive_violations())
        with source.prepare_archive() as prepared:
            assert source.to_bytes() == before
            assert b"VPreferences" not in prepared.entry(path)
            assert all(issue["rule"] != "V_PREFERENCES"
                       for issue in prepared.archive_violations())


def test_prepare_archive_uniquifies_duplicate_page_layer_names():
    with Document.new() as source:
        source.add_blank_page(40, 30)
        while len(source.page_layers(0)) < 4:
            source.add_page_layer(0, "Body")
        path = source.pages()[0]["path"]
        names = iter((b'A', b'A_2', b'A', b'A_2'))
        page = re.sub(rb'<ofd:Layer\b',
                      lambda match: match.group() + b' Name="' + next(names) + b'"',
                      source.entry(path), count=4)
        source.set_entry(path, page)
        before = source.to_bytes()
        assert [issue["rule"] for issue in source.archive_violations()].count("LAYER_NAME") == 2
        with source.prepare_archive() as prepared:
            assert source.to_bytes() == before
            assert all(issue["rule"] != "LAYER_NAME" for issue in prepared.archive_violations())
            assert re.findall(rb'<ofd:Layer\b[^>]*\bName="([^"]+)"', prepared.entry(path)) == [
                b'A', b'A_2', b'A_3', b'A_2_2',
            ]


def test_prepare_archive_rewrites_split_max_unit_id_through_python_binding():
    with Document.new() as source:
        source.add_blank_page(40, 30)
        path = "Doc_0/Document.xml"
        root = source.entry(path)
        original = re.search(rb"<ofd:MaxUnitID>(\d+)</ofd:MaxUnitID>", root)
        assert original and int(original.group(1)) > 1
        source.set_entry(path, root.replace(original.group(0),
            b"<ofd:MaxUnitID>0<!-- split -->1</ofd:MaxUnitID>"))
        before = source.to_bytes()
        with source.prepare_archive() as prepared:
            assert source.to_bytes() == before
            assert original.group(0) in prepared.entry(path)
            assert b"split" not in prepared.entry(path)
            with Document.from_bytes(prepared.to_bytes()) as reopened:
                assert reopened.validate_references() == []


def test_archive_violations_on_java_sample_and_new_document():
    with Document.open(ROOT / "ofdrw-archive/src/test/resources/test-ofda.ofd") as archived:
        issues = archived.archive_violations()
        assert [(issue["rule"], issue["severity"], issue["path"]) for issue in issues] == [
            ("FONT_SUBSET", "ERROR", "Doc_0/PublicRes.xml")
        ]

    with Document.new() as document:
        document.add_text_page("archive", 100, 100)
        issues = document.archive_violations()
        assert [(issue["rule"], issue["severity"]) for issue in issues] == [
            ("DOC_TYPE", "ERROR"),
            ("FONT_SUBSET", "ERROR"),
        ]
        source = document.entry("OFD.xml")
        document.set_entry("OFD.xml", source.replace(b'DocType="OFD"',
                                                     b'DocType="OFD-A"'))
        assert [issue["rule"] for issue in document.archive_violations()] == ["FONT_SUBSET"]
        document.append_document_body()
        issues = document.archive_violations()
        assert [issue["rule"] for issue in issues] == [
            "FONT_SUBSET", "FONT_SUBSET", "SINGLE_DOC"
        ]
        assert issues[2]["actual"] == "2"


def test_prepare_archive_returns_independent_document_with_residual_violations():
    with Document.new() as source:
        source.add_text_page("archive", 100, 100)
        source.add_attachment("report.pdf", b"%PDF-example")
        old_payload = source.attachments()[0]["path"]
        page = "Doc_0/Pages/Page_0/Content.xml"
        source.set_entry(
            page,
            source.entry(page).replace(
                b"</ofd:Page>",
                b'<ofd:Actions><ofd:Action><ofd:URI URI="relative"/></ofd:Action></ofd:Actions></ofd:Page>',
            ),
        )
        with source.prepare_archive() as prepared:
            assert [issue["rule"] for issue in prepared.archive_violations()] == [
                "FONT_SUBSET"
            ]
            assert b'DocType="OFD-A"' in prepared.entry("OFD.xml")
            assert b"<ofd:URI" not in prepared.entry(page)
            assert prepared.attachments()[0]["format"] == "XML"
            assert b"<SHA256>" in prepared.attachment_data(0)
            assert old_payload not in prepared.entries()
        assert b'DocType="OFD"' in source.entry("OFD.xml")
        assert b"<ofd:URI" in source.entry(page)
        assert source.attachment_data(0) == b"%PDF-example"


def test_prepare_archive_moves_single_glyph_horizontal_ctm_across_python_binding():
    page = "Doc_0/Pages/Page_0/Content.xml"
    with Document.new() as source:
        source.add_text_page("A", 100, 100)
        source.set_entry(
            page,
            source.entry(page).replace(b' Size="5"', b' Size="5" CTM="2 0 0 1 3 4"'),
        )
        original_png = source.page_png(0, 72)
        with source.prepare_archive() as prepared:
            assert b'HScale="2"' in prepared.entry(page)
            assert b'CTM="1 0 0 1 3 4"' in prepared.entry(page)
            assert prepared.page_png(0, 72) == original_png
            assert all(issue["rule"] != "TEXT_HSCALE"
                       for issue in prepared.archive_violations())
        assert b'HScale=' not in source.entry(page)


def test_prepare_archive_retains_shaped_ligature_across_python_binding():
    font = (ROOT / "ofdrw-rs/crates/ofdrw/tests/fixtures/dejavu-serif.ttf").read_bytes()
    page = "Doc_0/Pages/Page_0/Content.xml"
    with Document.new() as source:
        font_id = source.add_embedded_font("DejaVu Serif", font)
        source.add_blank_page(50, 30)
        source.add_text_object(0, "AfiB", (5, 5, 30, 10), 6, font_id, shape=True)
        source.set_entry(
            page,
            source.entry(page).replace(b' Size="6"', b' Size="6" CTM="2 0 0 1 3 4"'),
        )
        original_png = source.page_png(0, 144)
        with source.prepare_archive() as prepared:
            assert b'HScale="2"' in prepared.entry(page)
            assert b'CGTransform' in prepared.entry(page)
            assert prepared.page_text(0) == ["AfiB"]
            assert prepared.page_png(0, 144) == original_png


def test_prepare_archive_flattens_plain_deep_page_blocks_across_python_binding():
    path = "Doc_0/Pages/Page_0/Content.xml"
    with Document.new() as source:
        source.add_text_page("nested", 100, 100)
        page = source.entry(path)
        layer = page.index(b"<ofd:Layer")
        opening_end = page.index(b">", layer) + 1
        page = (page[:opening_end] + b"<ofd:PageBlock>" * 4 +
                page[opening_end:])
        page = page.replace(
            b"</ofd:Layer>",
            b"</ofd:PageBlock>" * 4 + b"</ofd:Layer>",
            1,
        )
        source.set_entry(path, page)
        before = source.page_svg(0)
        assert any(issue["rule"] == "PAGEBLOCK_DEPTH"
                   for issue in source.archive_violations())
        with source.prepare_archive() as prepared:
            assert prepared.entry(path).count(b"<ofd:PageBlock>") == 3
            assert prepared.page_svg(0) == before
            assert prepared.page_text(0) == ["nested"]
            assert prepared.validate_references() == []
            assert all(issue["rule"] != "PAGEBLOCK_DEPTH"
                       for issue in prepared.archive_violations())
        assert source.entry(path) == page


def test_prepare_archive_flattens_deep_template_blocks_across_python_binding():
    with Document.new() as source:
        source.add_text_page("template", 100, 100)
        source.promote_page_to_template(0, [])
        path = source.page_sources(0)[0]["path"]
        template = source.entry(path)
        layer = template.index(b"<ofd:Layer")
        opening_end = template.index(b">", layer) + 1
        template = (template[:opening_end] + b"<ofd:PageBlock>" * 4 +
                    template[opening_end:])
        template = template.replace(
            b"</ofd:Layer>",
            b"</ofd:PageBlock>" * 4 + b"</ofd:Layer>",
            1,
        )
        source.set_entry(path, template)
        assert any(issue["rule"] == "PAGEBLOCK_DEPTH" and issue["path"] == path
                   for issue in source.archive_violations())
        with source.prepare_archive() as prepared:
            assert prepared.entry(path).count(b"<ofd:PageBlock>") == 3
            assert all(issue["rule"] != "PAGEBLOCK_DEPTH" or issue["path"] != path
                       for issue in prepared.archive_violations())


def test_prepare_archive_flattens_annotation_appearance_blocks_across_python_binding():
    with Document.new() as source:
        source.add_text_page("annotation", 100, 100)
        source.add_highlight_annotation(0, (1, 2, 30, 4), (255, 255, 0), 128)
        path = source.annotations()[0]["source_path"]
        annotation = source.entry(path)
        opening = annotation.index(b"<ofd:Appearance")
        opening_end = annotation.index(b">", opening) + 1
        annotation = (annotation[:opening_end] + b"<ofd:PageBlock>" +
                      annotation[opening_end:])
        annotation = annotation.replace(
            b"</ofd:Appearance>",
            b"</ofd:PageBlock></ofd:Appearance>",
            1,
        )
        source.set_entry(path, annotation)
        before = source.page_svg(0)
        with source.prepare_archive() as prepared:
            assert b"PageBlock" not in prepared.entry(path)
            assert prepared.page_svg(0) == before
            assert prepared.validate_references() == []
            assert all(issue["rule"] != "ANNOTATION"
                       for issue in prepared.archive_violations())
        assert source.entry(path) == annotation


def test_prepare_archive_creates_public_resources_across_python_binding():
    with Document.new() as source:
        source.add_text_page("page font", 100, 100)
        public_path = "Doc_0/PublicRes.xml"
        public = source.entry(public_path)
        start = public.index(b"<ofd:Fonts>")
        end = public.index(b"</ofd:Fonts>", start) + len(b"</ofd:Fonts>")
        fonts = public[start:end]
        source.set_entry(public_path, public.replace(fonts, b""))
        source.set_entry(
            "Doc_0/PageRes.xml",
            b'<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="Res">'
            + fonts + b"</ofd:Res>",
        )
        page_path = "Doc_0/Pages/Page_0/Content.xml"
        source.set_entry(
            page_path,
            source.entry(page_path).replace(
                b"</ofd:Page>",
                b"<ofd:PageRes>../../PageRes.xml</ofd:PageRes></ofd:Page>",
            ),
        )
        root_path = "Doc_0/Document.xml"
        link = b"<ofd:PublicRes>PublicRes.xml</ofd:PublicRes>"
        root = source.entry(root_path)
        assert link in root
        source.set_entry(root_path, root.replace(link, b""))
        before = source.page_svg(0)
        assert source.page_fonts(0)[0]["resource_path"] == "Doc_0/PageRes.xml"

        with source.prepare_archive() as prepared:
            new_path = "Doc_0/PublicRes_Archive.xml"
            assert b"<ofd:Fonts>" in prepared.entry(new_path)
            assert b"<ofd:Fonts>" not in prepared.entry("Doc_0/PageRes.xml")
            assert b"<ofd:PublicRes>PublicRes_Archive.xml</ofd:PublicRes>" in prepared.entry(root_path)
            assert prepared.page_fonts(0)[0]["resource_path"] == new_path
            assert prepared.page_svg(0) == before
            assert prepared.validate_references() == []
            assert all(issue["rule"] != "RESOURCE_PLACEMENT" for issue in prepared.archive_violations())


def test_prepare_archive_removes_document_and_page_audio_video():
    with Document.new() as source:
        source.add_blank_page(30, 20)
        source.add_multimedia_resource(0, "Audio", "MP3", b"audio")
        source.add_page_multimedia_resource(0, "Video", "MP4", b"video")
        resources = source.page_multimedia_resources(0)
        assert len(resources) == 2
        assert any(item["rule"] == "AUDIO_VIDEO" for item in source.archive_violations())
        before = source.to_bytes()
        with source.prepare_archive() as prepared:
            assert prepared.page_multimedia_resources(0) == []
            assert all(item["rule"] != "AUDIO_VIDEO" for item in prepared.archive_violations())
            for item in resources:
                assert item["path"] not in prepared.entries()
            assert prepared.validate_references() == []
            with Document.from_bytes(prepared.to_bytes()) as reopened:
                assert reopened.page_multimedia_resources(0) == []
                assert reopened.validate_references() == []
        assert source.to_bytes() == before
        assert len(source.page_multimedia_resources(0)) == 2
