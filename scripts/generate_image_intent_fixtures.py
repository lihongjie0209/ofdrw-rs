"""Author image-intent PDFs and independent Ghostscript reference rasters."""
from pathlib import Path
import argparse
import subprocess
import zlib

from generate_intent_profile import profile
from generate_nested_stitching_fixture import pdf_bytes, stream

CASES = ['xobject','form','inline','inline-embedded-ei','inline-trailing-cr','inline-flate','form-inline','masked']


def fixture(kind):
    resources = (b'<< /ColorSpace << /CS [/ICCBased 5 0 R] >> /XObject << /Im 6 0 R /Own 7 0 R /Fm 8 0 R >> '
                 b'/ExtGState << /G << /RI /Saturation >> >> >>')
    image = b'/Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace [/ICCBased 5 0 R]'
    inline_samples = b' EI ' if kind == 'inline-embedded-ei' else bytes([77,51,26,13 if kind == 'inline-trailing-cr' else 51])
    inline_filter = b' /F /Fl' if kind == 'inline-flate' else b''
    inline_data = zlib.compress(inline_samples) if kind == 'inline-flate' else inline_samples
    inline = b'BI /W 1 /H 1 /BPC 8 /CS /CS'+inline_filter+b' ID '+inline_data+(b'\nEI' if kind == 'inline-trailing-cr' else b' EI')
    page_resources = resources.replace(b'/CS [/ICCBased 5 0 R]',b'/CS /DeviceRGB') if kind == 'form-inline' else resources
    draws = []
    for index in range(5):
        if index == 4:
            paint = b'/Own Do'
        elif kind.startswith('form'):
            paint = b'/Fm Do'
        elif kind.startswith('inline'):
            paint = inline
        else:
            paint = b'/Im Do'
        draws.append(f'q 16 0 0 24 {4+index*18} 12 cm '.encode()+paint+b' Q ')
    content = b'/Perceptual ri '+draws[0]+b'q /RelativeColorimetric ri '+draws[1]+b'Q '+draws[2]+b'/G gs '+draws[3]+draws[4]
    return pdf_bytes([
        b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 48] /Contents 4 0 R /Resources '+page_resources+b' >>',
        stream(b'',content),stream(b'/N 4',profile()),
        stream(image+(b' /SMask 9 0 R' if kind == 'masked' else b''),bytes([77,51,26,51])),
        stream(image+b' /Intent /RelativeColorimetric'+(b' /Mask 10 0 R' if kind == 'masked' else b''),bytes([77,51,26,51])),
        stream(b'/Type /XObject /Subtype /Form /BBox [0 0 1 1] /Resources '+resources,inline if kind == 'form-inline' else b'/Im Do'),
        stream(b'/Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray',bytes([128])),
        stream(b'/Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 1 /ImageMask true',bytes([0]))
    ])


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--references',action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]/'crates/ofdrw/tests/fixtures'
    for kind in CASES:
        name = 'image-intent-'+kind
        target = root/(name+'.pdf')
        target.write_bytes(fixture(kind))
        if args.references:
            subprocess.run(['gs','-q','-dSAFER','-dBATCH','-dNOPAUSE','-dBlackPtComp=0','-sDEVICE=png16m','-r72','-dGraphicsAlphaBits=1',f'-sOutputFile={root/(name+".png")}',str(target)],check=True)
