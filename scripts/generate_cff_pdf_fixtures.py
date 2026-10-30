"""Author small CFF fonts and PDF fixtures; requires fonttools only for generation.

Run: uv run --with fonttools python scripts/generate_cff_pdf_fixtures.py --references
All glyph outlines are authored for this repository under its Apache-2.0 license.
"""
import argparse
from copy import deepcopy
import re
from io import BytesIO
from pathlib import Path
import subprocess
import tempfile
import zlib

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.t2CharStringPen import T2CharStringPen
from fontTools.cffLib import FDArrayIndex, FontDict, FDSelect
from generate_pdf_glyph_fixtures import objects
from generate_nested_stitching_fixture import pdf_bytes, stream


def font(cid=False, font_matrix=None, fd_matrix=None, dingbats=False, long_name=False, fd_matrices=None, fd_select=None, fd_format=0):
    names = [".notdef", "cid00007" if cid else "A", "cid00042" if cid else "fi"]
    if dingbats:
        names = [".notdef", "a1", "a2"]
    if long_name:
        names[1] = "x" * 1025
    builder = FontBuilder(1000, isTTF=False)
    builder.setupGlyphOrder(names)
    builder.setupCharacterMap({65: names[1], 0xfb01: names[2]})
    charstrings = {}
    for index, name in enumerate(names):
        pen = T2CharStringPen([500, 722, 750][index], None)
        if index == 1:
            pen.moveTo((0, 0)); pen.lineTo((300, 700)); pen.lineTo((600, 0)); pen.closePath()
        elif index == 2:
            pen.moveTo((-50, 0)); pen.lineTo((-50, 500))
            pen.curveTo((-50, 750), (300, 750), (300, 550))
            pen.lineTo((180, 550)); pen.curveTo((180, 650), (80, 650), (80, 500))
            pen.lineTo((80, 0)); pen.closePath()
            pen.moveTo((350, 0)); pen.lineTo((350, 500)); pen.lineTo((480, 500)); pen.lineTo((480, 0)); pen.closePath()
        else:
            pen.moveTo((0, 0)); pen.lineTo((0, 700)); pen.lineTo((500, 700)); pen.lineTo((500, 0)); pen.closePath()
        charstrings[name] = pen.getCharString()
    info = {"FullName": "OFD Test CFF", "FamilyName": "OFD Test CFF", "Weight": "Regular"}
    if font_matrix is not None:
        info["FontMatrix"] = font_matrix
    builder.setupCFF("ABCDEF+ZapfDingbats" if dingbats else "OFDTestCFF", info, charstrings, {})
    if dingbats or long_name:
        encoding = [".notdef"] * 256
        encoding[65], encoding[174] = names[1], names[2]
        builder.font["CFF "].cff.topDictIndex[0].Encoding = encoding
    builder.setupHorizontalMetrics({name: (width, 0) for name, width in zip(names, [500, 722, 750])})
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupNameTable({"familyName": "OFD Test CFF", "styleName": "Regular", "uniqueFontIdentifier": "OFD Test CFF", "fullName": "OFD Test CFF", "psName": "OFDTestCFF"})
    builder.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
    builder.setupPost()
    builder.setupMaxp()
    if cid:
        top = builder.font["CFF "].cff.topDictIndex[0]
        top.ROS = ("Adobe", "Identity", 0)
        top.CIDCount = 43
        array = FDArrayIndex()
        array.strings = None
        array.GlobalSubrs = top.GlobalSubrs
        for index, matrix in enumerate(fd_matrices if fd_matrices is not None else [fd_matrix]):
            fd = FontDict()
            fd.Private = top.Private if index == 0 else deepcopy(top.Private)
            if matrix is not None:
                fd.FontMatrix = matrix
            array.append(fd)
        top.FDArray = array
        select = FDSelect(format=fd_format)
        select.gidArray = fd_select if fd_select is not None else [0] * len(names)
        top.FDSelect = select
        top.CharStrings.fdArray = array
        top.CharStrings.fdSelect = select
        for index, name in enumerate(names):
            selected = select.gidArray[index]
            top.CharStrings[name].fdSelectIndex = selected
            top.CharStrings[name].private = array[selected].Private
        del top.Private
    builder.font.recalcTimestamp = False
    builder.font["head"].created = builder.font["head"].modified = 2082844800
    raw = builder.font["CFF "].compile(builder.font)
    output = BytesIO()
    builder.font.save(output)
    return output.getvalue(), raw


def pdf(font_bytes, raw=False, simple=False, cid=False, vertical=False, internal=False, actual_text=None):
    result = objects(font_bytes, simple=simple)
    if simple:
        result[4] = result[4].replace(b"/Subtype /TrueType", b"/Subtype /Type1")
    else:
        result[6] = result[6].replace(b"/Subtype /CIDFontType2", b"/Subtype /CIDFontType0")
        if cid:
            result[3] = result[3].replace(b"0001", b"002a").replace(b"0002", b"0007")
            data = result[5].split(b"stream\n", 1)[1].rsplit(b"endstream", 1)[0]
            result[5] = stream(b"", data.replace(b"0001", b"002a").replace(b"0002", b"0007"))
            result[6] = result[6].replace(b"/W [1 [750 722]]", b"/W [42 [750] 7 [722]]")
        else:
            # A non-CID CFF program uses CID directly as GID (fi=2, A=1).
            result[3] = result[3].replace(b"0001", b"ffff").replace(b"0002", b"0001").replace(b"ffff", b"0002")
            data = result[5].split(b"stream\n", 1)[1].rsplit(b"endstream", 1)[0]
            result[5] = stream(b"", data.replace(b"0001", b"ffff").replace(b"0002", b"0001").replace(b"ffff", b"0002"))
            result[6] = result[6].replace(b"/W [1 [750 722]]", b"/W [2 [750] 1 [722]]")
    if internal:
        result[4] = result[4].replace(b" /ToUnicode 6 0 R", b"").replace(b"/Encoding << /BaseEncoding /WinAnsiEncoding /Differences [65 /fi /A] >> ", b"")
        widths = [0] * 110
        widths[0] = 722
        widths[109] = 750
        result[4] = result[4].replace(b"/FirstChar 65 /LastChar 66 /Widths [750 722]", b"/FirstChar 65 /LastChar 174 /Widths [" + " ".join(map(str, widths)).encode() + b"]")
        result[3] = result[3].replace(b"<41>", b"<ae>").replace(b"<42>", b"<41>")
    if not simple and not cid:
        result[6] = result[6].replace(b" /CIDToGIDMap 10 0 R", b"")
    result = [item.replace(b"DejaVuSerif", b"OFDTestCFF") for item in result]
    if vertical:
        result[2] = result[2].replace(b"144 96", b"144 144")
        result[3] = result[3].replace(b"10 60 Td", b"60 130 Td")
        # Rebuild stream length after changing contents.
        content = result[3].split(b"stream\n", 1)[1].rsplit(b"endstream", 1)[0].rstrip(b"\n")
        result[3] = stream(b"", content)
        result[4] = result[4].replace(b"Identity-H", b"Identity-V")
    if actual_text is not None:
        content = result[3].split(b"stream\n", 1)[1].rsplit(b"endstream", 1)[0].rstrip(b"\n")
        replacement = (b"\xfe\xff" + actual_text.encode("utf-16-be")).hex().upper().encode()
        result[3] = stream(b"", b"/Span << /ActualText <" + replacement + b"> >> BDC\n" + content + b"\nEMC")
    result[7] = result[7].replace(b"FontFile2", b"FontFile3")
    subtype = "OpenType" if not raw else ("Type1C" if simple or not cid else "CIDFontType0C")
    result[8] = stream(f"/Subtype /{subtype} /Filter /FlateDecode".encode(), zlib.compress(font_bytes, 9))
    return pdf_bytes(result)


def write_fixture(fixtures, name, source, references):
    (fixtures / f"{name}.pdf").write_bytes(source)
    if references:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source.pdf"
            path.write_bytes(source)
            subprocess.run(["gs", "-q", "-dSAFER", "-dBATCH", "-dNOPAUSE", "-sDEVICE=png16m", "-r72", "-dTextAlphaBits=4", f"-sOutputFile={fixtures / name}.png", str(path)], check=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--references", action="store_true")
    args = parser.parse_args()
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    sid_otf, sid_cff = font()
    cid_otf, cid_cff = font(cid=True)
    _, dingbats_cff = font(dingbats=True)
    (fixtures / "test-dingbats.cff").write_bytes(dingbats_cff)
    _, long_cff = font(long_name=True)
    (fixtures / "test-long-glyph-name.cff").write_bytes(long_cff)
    for name, data in [("test-name-cff.otf", sid_otf), ("test-name.cff", sid_cff), ("test-cid-cff.otf", cid_otf), ("test-cid.cff", cid_cff)]:
        (fixtures / name).write_bytes(data)
    for name, options in [
        ("test-cff-skew.cff", {"font_matrix": [.001, .0002, 0, .001, 0, 0]}),
        ("test-cff-2048.cff", {"font_matrix": [1/2048, 0, 0, 1/2048, 0, 0]}),
        ("test-cff-fd-matrix.cff", {"cid": True, "fd_matrix": [2, 0, 0, 1, 0, 0]}),
        ("test-cff-fd-identity.cff", {"cid": True, "fd_matrix": [1, 0, 0, 1, 0, 0]}),
    ]:
        _, raw = font(**options)
        (fixtures / name).write_bytes(raw)
    for name, data, raw, simple, cid, vertical in [
        ("type1-opentype-cff", sid_otf, False, True, False, False),
        ("type1-raw-cff", sid_cff, True, True, False, False),
        ("type1-internal-cff", sid_cff, True, True, False, False),
        ("type1-implicit-base-cff", sid_cff, True, True, False, False),
        ("type1-dingbats-base-cff", dingbats_cff, True, True, False, False),
        ("type0-opentype-cid-cff", cid_otf, False, False, True, False),
        ("type0-raw-cid-cff", cid_cff, True, False, True, False),
        ("type0-name-cff", sid_cff, True, False, False, False),
        ("type0-vertical-cid-cff", cid_cff, True, False, True, True),
    ]:
        source = pdf(data, raw, simple, cid, vertical, internal=name in ["type1-internal-cff", "type1-implicit-base-cff", "type1-dingbats-base-cff"])
        if name in ["type1-implicit-base-cff", "type1-dingbats-base-cff"]:
            # CID-free CFF StandardEncoding supplies fi at 174. Only code 65 is overridden.
            parts = re.findall(rb"\d+ 0 obj\n(.*?)\nendobj", source, re.S)
            parts[4] = parts[4][:-2] + b" /Encoding << /Differences [65 /fi] >> >>"
            parts[4] = parts[4].replace(b"/Widths [722 ", b"/Widths [750 ")
            if name == "type1-dingbats-base-cff":
                parts[4] = parts[4].replace(b"/fi", b"/a2").replace(b"OFDTestCFF", b"ABCDEF+ZapfDingbats")
                parts[7] = parts[7].replace(b"OFDTestCFF", b"ABCDEF+ZapfDingbats").replace(b"/Flags 32", b"/Flags 4")
            source = pdf_bytes(parts)
        write_fixture(fixtures, name, source, args.references)

    top_matrix = [.001, .0001, 0, .001, 0, 0]
    local = [.8, .1, .3, 1.2, 50, -20]
    for name, options, raw, simple, vertical, paint in [
        ("type1-affine-cff", {"font_matrix": [.001, .0003, .0004, .0008, .12, -.06]}, True, True, False, "fill"),
        ("type1-reflected-cff", {"font_matrix": [-.001, 0, .0002, .001, .6, 0]}, True, True, False, "fill"),
        ("type1-rotated-cff", {"font_matrix": [0, .001, -.001, 0, .8, 0]}, True, True, False, "fill"),
        ("type1-opentype-affine-cff", {"font_matrix": [.001, .0003, .0004, .0008, .12, -.06]}, False, True, False, "fill"),
        ("type0-fd-matrices-cff", {"cid": True, "font_matrix": top_matrix, "fd_matrices": [None, local], "fd_select": [0, 0, 1]}, True, False, False, "fill"),
        ("type0-fd-translated-top-cff", {"cid": True, "font_matrix": [.001, .0001, 0, .001, .12, .05], "fd_matrices": [None, local], "fd_select": [0, 0, 1]}, True, False, False, "fill"),
        ("type0-fd-ranges-cff", {"cid": True, "font_matrix": top_matrix, "fd_matrices": [None, local], "fd_select": [0, 0, 1], "fd_format": 3}, True, False, False, "fill"),
        ("type0-fd-only-cff", {"cid": True, "fd_matrices": [None, [.0008, .0001, .0003, .0012, .05, -.02]], "fd_select": [0, 0, 1]}, True, False, False, "fill"),
        ("type0-vertical-fd-cff", {"cid": True, "font_matrix": top_matrix, "fd_matrices": [None, local], "fd_select": [0, 0, 1]}, True, False, True, "fill"),
        ("type1-stroked-affine-cff", {"font_matrix": [.001, .0003, .0004, .0008, -.1, -.06]}, True, True, False, "stroke"),
        ("type0-clipped-fd-cff", {"cid": True, "font_matrix": top_matrix, "fd_matrices": [None, local], "fd_select": [0, 0, 1]}, True, False, False, "clip"),
    ]:
        otf, program = font(**options)
        data = program if raw else otf
        source = pdf(data, raw, simple, options.get("cid", False), vertical)
        if paint != "fill":
            parts = re.findall(rb"\d+ 0 obj\n(.*?)\nendobj", source, re.S)
            content = parts[3].split(b"stream\n", 1)[1].rsplit(b"endstream", 1)[0].rstrip(b"\n")
            if paint == "stroke":
                content = b"1 0 0 RG 3 w " + content.replace(b"BT", b"BT 2 Tr", 1)
            else:
                content = content.replace(b"BT", b"BT 7 Tr", 1) + b" 1 0 0 rg 0 0 144 96 re f"
            parts[3] = stream(b"", content)
            source = pdf_bytes(parts)
        write_fixture(fixtures, name, source, args.references)

    source = objects((fixtures / "dejavu-serif.ttf").read_bytes())
    content = source[3].split(b"stream\n", 1)[1].rsplit(b"endstream", 1)[0].rstrip(b"\n")
    source[3] = stream(b"", content.replace(b"BT", b"BT 7 Tr", 1) + b" 1 0 0 rg 0 0 144 96 re f")
    write_fixture(fixtures, "type0-clipped-ligature", pdf_bytes(source), args.references)
