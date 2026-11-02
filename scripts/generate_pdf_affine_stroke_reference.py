"""Generate affine PDF stroke fixtures and independent renderer references.

Ghostscript GraphicsAlphaBits=4 is unsuitable for transparent fill+stroke
(https://www.ghostscript.com/faq/index.html). Its supersampled zero-width
lines also differ from a one-device-pixel hairline. Use Poppler for those
cases, keeping the same strict raster and ink-coverage comparisons.
"""
from pathlib import Path
import json
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
CASES = {
    'solid-curve': (
        3,
        b'q 1.6 0.2 0.5 0.7 20 20 cm 3 w 1 J 1 j 0.2 0.5 0.8 RG 10 10 m 20 90 65 '
        b'20 85 70 c S Q'
    ),
    'scale-dash': (
        3,
        b'q 2 0 0 0.7 10 15 cm 3 w [6 3] 1 d 0.8 0.1 0.2 RG 10 10 m 70 10 l 70 90 '
        b'l S Q'
    ),
    'shear-dash': (
        3,
        b'q 1 0.2 0.55 1 15 15 cm 3 w 2 J 2 j [5 3 2] 1 d 0.1 0.3 0.8 RG 5 10 m 90 '
        b'10 l 60 60 l S Q'
    ),
    'reflection': (
        4,
        b'q -1 0.2 0.25 1.3 175 10 cm 4 w [7 4] 1 d 0.1 0.6 0.2 RG 20 20 m 110 20 '
        b'l 110 80 l 20 80 l h S Q'
    ),
    'changing-ctm': (
        2,
        b'q 2 0.2 0 1 15 10 cm 10 20 m 40 20 l 0.5 0.3 -0.2 1 0 0 cm 90 70 l 60 90 '
        b'l 2 w [4 3] 1 d 0.6 0.1 0.7 RG S Q'
    ),
    'restored-ctm': (
        2,
        b'q 1.5 0.2 0.3 0.8 20 10 cm q 0.8 0.3 0.2 1 0 0 cm 10 20 m 60 20 l Q 90 '
        b'70 l 60 90 l 2 w [4 3] 1 d 0.6 0.1 0.7 RG S Q'
    ),
    'fill-stroke-alpha': (
        3,
        b'q /A gs 1 0 0 rg 0.1 0.2 0.9 RG 3 w 1 J 1 j [5 2] 1 d 1.5 0.15 0.3 0.7 '
        b'20 20 cm 10 10 60 60 re B Q'
    ),
    'pattern-clip': (
        5,
        b'q 20 20 130 90 re W n /Pattern CS /P SCN 5 w [6 3] 0 d 1 0.25 0.45 1.2 '
        b'15 10 cm 0 20 m 100 20 l 70 80 30 100 0 50 c S Q'
    ),
    'hairline': (
        0,
        b'q 2 0.15 0.5 0.8 15 10 cm 0 w [7 3] 1 d 1 J 0 j 0.8 0.2 0.1 RG 10 10 m '
        b'70 10 l 70 60 l S Q'
    ),
}


def stream(data, extra=b''):
    return (b'<< /Length ' + str(len(data)).encode() + b' ' + extra
            + b' >>\nstream\n' + data + b'\nendstream')


def pdf(content):
    pattern = stream(
        b'0 0.2 1 rg 0 0 6 12 re f 1 0.6 0 rg 6 0 6 12 re f',
        b'/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 '
        b'/BBox [0 0 12 12] /XStep 12 /YStep 12 /Resources << >>',
    )
    objects = [
        b'<< /Type /Catalog /Pages 2 0 R >>',
        b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 160] '
        b'/Resources << /Pattern << /P 5 0 R >> '
        b'/ExtGState << /A 6 0 R >> >> /Contents 4 0 R >>',
        stream(content), pattern, b'<< /Type /ExtGState /CA 0.5 /ca 0.65 >>',
    ]
    output = bytearray(b'%PDF-1.7\n')
    offsets = [0]
    for index, value in enumerate(objects, 1):
        offsets.append(len(output))
        output += f'{index} 0 obj\n'.encode() + value + b'\nendobj\n'
    xref = len(output)
    output += f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode()
    for offset in offsets[1:]:
        output += f'{offset:010d} 00000 n \n'.encode()
    output += (f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\n'
               f'startxref\n{xref}\n%%EOF\n').encode()
    return bytes(output)


def main():
    destination = ROOT / 'crates/ofdrw/tests/fixtures/pdf-affine-strokes'
    destination.mkdir(exist_ok=True)
    cases = []
    with tempfile.TemporaryDirectory(prefix='ofdrw-pdf-affine-') as temp:
        for name, (width, content) in CASES.items():
            source = destination / f'{name}.pdf'
            source.write_bytes(pdf(content))
            references = {}
            renderer = 'Poppler' if name in {'hairline', 'fill-stroke-alpha'} else 'Ghostscript'
            for dpi in [72, 144]:
                prefix = Path(temp) / f'{name}-{dpi}'
                target = prefix.with_suffix('.png')
                if renderer == 'Poppler':
                    command = ['pdftoppm', '-r', str(dpi), '-png', '-singlefile',
                               str(source), str(prefix)]
                else:
                    command = ['gs', '-q', '-dSAFER', '-dBATCH', '-dNOPAUSE',
                               '-sDEVICE=png16m', '-dGraphicsAlphaBits=4',
                               f'-r{dpi}', f'-sOutputFile={target}', str(source)]
                subprocess.run(command, check=True, stdout=subprocess.DEVNULL,
                               stderr=subprocess.PIPE)
                output = destination / target.name
                output.write_bytes(target.read_bytes())
                references[str(dpi)] = output.name
            cases.append({'name': name, 'file': source.name, 'line_width_pt': width,
                          'renderer': renderer, 'references': references})
    metadata = {
        'ghostscript_version': subprocess.check_output(['gs', '--version'], text=True).strip(),
        'poppler_version': subprocess.run(['pdftoppm', '-v'], capture_output=True,
                                         text=True, check=True).stderr.splitlines()[0],
        'cases': cases,
    }
    (destination / 'reference.json').write_text(json.dumps(metadata, indent=2) + '\n')


if __name__ == '__main__':
    main()
