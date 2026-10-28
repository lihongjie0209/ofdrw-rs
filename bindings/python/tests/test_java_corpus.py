"""End-to-end reading and SVG checks against the Java project's OFD samples."""

from pathlib import Path
import base64
import struct
import zlib
from xml.etree import ElementTree

import pytest

from ofdrw_rs import Document


ROOT = Path(__file__).resolve().parents[4]
MODULES = (
    "ofdrw-archive",
    "ofdrw-converter",
    "ofdrw-crypto",
    "ofdrw-layout",
    "ofdrw-reader",
    "ofdrw-sign",
    "ofdrw-tool",
)
SAMPLES = tuple(
    path
    for module in MODULES
    for path in sorted((ROOT / module / "src/test/resources").glob("*.ofd"))
)


@pytest.mark.parametrize("path", SAMPLES, ids=lambda path: str(path.relative_to(ROOT)))
def test_java_ofd_sample_pages_extract_and_render(path):
    with Document.open(path) as document:
        pages = document.pages()
        assert pages
        assert isinstance(document.archive_violations(), list)
        for index in range(len(pages)):
            assert isinstance(document.page_text(index), list)
            svg = document.page_svg(index)
            assert ElementTree.fromstring(svg).tag.endswith("svg")


@pytest.mark.parametrize(
    "name",
    ("helloworld.ofd", "helloworld_with_pageblock.ofd"),
)
def test_java_reader_text_survives_page_blocks_and_package_roundtrip(name):
    path = ROOT / "ofdrw-reader" / "src/test/resources" / name
    expected = "你好呀，OFD Reader&Writer！"
    with Document.open(path) as document:
        assert document.page_text(0) == [expected]
        assert "".join(ElementTree.fromstring(document.page_svg(0)).itertext()) == expected
        data = document.to_bytes()
    with Document.from_bytes(data) as reopened:
        assert reopened.page_text(0) == [expected]
        assert "".join(ElementTree.fromstring(reopened.page_svg(0)).itertext()) == expected


@pytest.mark.parametrize(
    "sample",
    (
        "ofdrw-reader/src/test/resources/helloworld_with_pageblock.ofd",
        "ofdrw-converter/src/test/resources/发票示例.ofd",
        "ofdrw-converter/src/test/resources/1.ofd",
    ),
)
def test_java_sample_rasterizes_to_png_through_python_binding(sample):
    with Document.open(ROOT / sample) as document:
        png = document.page_png(0, 25.4)
        assert png.startswith(b"\x89PNG\r\n\x1a\n")
        width, height = struct.unpack(">II", png[16:24])
        assert width > 0 and height > 0


@pytest.mark.parametrize(
    ("sample", "page_index"),
    (
        ("ofdrw-converter/src/test/resources/999.ofd", 4),
        ("ofdrw-converter/src/test/resources/intro-数科.ofd", 11),
        ("ofdrw-layout/src/test/resources/AddWatermarkAnnot.ofd", 5),
    ),
)
def test_java_multi_page_sample_rasterizes_later_pages(sample, page_index):
    with Document.open(ROOT / sample) as document:
        assert len(document.pages()) > page_index
        png = document.page_png(page_index, 25.4)
        assert png.startswith(b"\x89PNG\r\n\x1a\n")
        width, height = struct.unpack(">II", png[16:24])
        assert width > 0 and height > 0


def _first_rgba_pixel(png):
    assert png.startswith(b"\x89PNG\r\n\x1a\n")
    assert png[24:26] == b"\x08\x06"  # 8-bit RGBA
    compressed = []
    offset = 8
    while offset < len(png):
        length = struct.unpack_from(">I", png, offset)[0]
        kind = png[offset + 4:offset + 8]
        if kind == b"IDAT":
            compressed.append(png[offset + 8:offset + 8 + length])
        offset += length + 12
        if kind == b"IEND":
            break
    # All PNG filters predict zero for the first pixel of the first row.
    return tuple(zlib.decompress(b"".join(compressed))[1:5])


def test_nested_ofd_seal_keeps_transparent_background_through_python_binding():
    path = ROOT / "ofdrw-layout/src/test/resources/keyword2.ofd"
    with Document.open(path) as document:
        svg = ElementTree.fromstring(document.page_svg(0))
        seal = next(node for node in svg.iter() if "data-ofd-stamp" in node.attrib)
        href = seal.attrib["href"]
        assert href.startswith("data:image/png;base64,")
        assert _first_rgba_pixel(base64.b64decode(href.split(",", 1)[1]))[3] == 0

    with Document.new() as blank:
        blank.add_blank_page(20, 20)
        assert _first_rgba_pixel(blank.page_png(0, 25.4)) == (255, 255, 255, 255)
