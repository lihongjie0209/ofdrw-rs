"""Author referenced shading parameters and equivalent direct-value references.

Ghostscript 10.02.1 cannot render all of these references; Poppler ignores the
aliased BBox. Independent rasters therefore use committed direct-value PDFs.
Core tests also require byte-identical SVG/PNG for direct and referenced data.
"""
from pathlib import Path
import argparse
import subprocess
from generate_nested_stitching_fixture import pdf_bytes, stream


def fixture(pattern=True, indirect=True):
    resources = b'<< /Pattern << /P 5 0 R >> >>' if pattern else b'<< /Shading << /S 6 0 R >> >>'
    content = b'/Pattern cs /P scn 4 4 88 56 re f' if pattern else b'/S sh'
    definition = b'<< /Type /Pattern /PatternType 20 0 R /Shading 6 0 R /Matrix 14 0 R /ExtGState 21 0 R >>'
    shading = (b'<< /ShadingType 12 0 R /ColorSpace /DeviceRGB /BBox 17 0 R /Coords 9 0 R '
               b'/Extend 10 0 R /Domain 11 0 R /Background 13 0 R /Function 7 0 R >>')
    if not indirect:
        definition = (b'<< /Type /Pattern /PatternType 2 /Shading 6 0 R /Matrix [1 0 0 1 0 0] '
                      b'/ExtGState << /RI /RelativeColorimetric /ca 1 >> >>')
        shading = (b'<< /ShadingType 2 /ColorSpace /DeviceRGB /BBox [20 10 76 54] /Coords [32 32 64 32] '
                   b'/Extend [false false] /Domain [0 1] /Background [.2 .8 .3] /Function 7 0 R >>')
    return pdf_bytes([
        b'<< /Type /Catalog /Pages 2 0 R >>',
        b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 64] /Resources '+resources+b' /Contents 4 0 R >>',
        stream(b'', content),
        definition, shading,
        b'<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >>',
        b'[20 10 76 54]', b'[16 0 R 16 0 R 64 32]', b'[15 0 R 15 0 R]',
        b'[0 1]', b'2', b'[.2 .8 .3]', b'[18 0 R 19 0 R 19 0 R 18 0 R 19 0 R 19 0 R]',
        b'false', b'32', b'8 0 R', b'1', b'0', b'2',
        b'<< /RI 22 0 R /ca 23 0 R >>', b'/RelativeColorimetric', b'1',
    ])


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--references', action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]/'crates/ofdrw/tests/fixtures'
    for pattern, name in [(True, 'indirect-params'), (False, 'indirect-direct-sh')]:
        target = root/f'shading-pattern-{name}.pdf'
        target.write_bytes(fixture(pattern))
        if args.references:
            reference = target.with_name(target.stem+'-reference.pdf')
            reference.write_bytes(fixture(pattern, indirect=False))
            subprocess.run(['gs','-q','-dSAFER','-dBATCH','-dNOPAUSE','-sDEVICE=png16m','-r144',
                            '-dGraphicsAlphaBits=1',f'-sOutputFile={target.with_suffix(".png")}',str(reference)],check=True)
