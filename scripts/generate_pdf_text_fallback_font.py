"""Generate a font for PDF text extraction tests (fonttools required).

The geometric outlines are authored here under the repository's Apache-2.0
license. This is a test font, not a replacement for readable CJK glyphs.
"""
from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen


def main():
    builder = FontBuilder(1000, isTTF=True)
    characters = [0x4E2D, 0x6587, 0x31350]
    names = [".notdef"] + [f"uni{value:X}" for value in characters]
    builder.setupGlyphOrder(names)
    builder.setupCharacterMap(dict(zip(characters, names[1:])))
    glyphs = {}
    for index, name in enumerate(names):
        pen = TTGlyphPen(None)
        # Different contours make accidentally conflated glyphs observable.
        pen.moveTo((50, 0))
        pen.lineTo((50, 700))
        pen.lineTo((450 + index * 100, 700))
        pen.lineTo((450 + index * 100, 0))
        pen.closePath()
        glyphs[name] = pen.glyph()
    builder.setupGlyf(glyphs)
    builder.setupHorizontalMetrics({name: (1000, 50) for name in names})
    builder.setupHorizontalHeader(ascent=800, descent=-200)
    builder.setupNameTable({
        "familyName": "OFD PDF Text Fixture",
        "styleName": "Regular",
        "uniqueFontIdentifier": "OFD PDF Text Fixture Regular",
        "fullName": "OFD PDF Text Fixture Regular",
        "psName": "OFDPDFTextFixture-Regular",
    })
    builder.setupOS2(sTypoAscender=800, sTypoDescender=-200,
                     usWinAscent=800, usWinDescent=200)
    builder.setupPost()
    builder.setupMaxp()
    builder.font["head"].created = 2082844800
    builder.font["head"].modified = 2082844800
    builder.font.recalcTimestamp = False
    target = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures/pdf-text-fallback.ttf"
    builder.save(target)


if __name__ == "__main__":
    main()
