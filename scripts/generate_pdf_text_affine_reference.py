"""Generate embedded-font text-stroke PDFs and independent raster references."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import zlib

from generate_pdf_glyph_fixtures import objects
from generate_nested_stitching_fixture import pdf_bytes, stream

ROOT = Path(__file__).resolve().parents[1]
SHOW = b'[<41> 100 <42>] TJ <41> Tj'
CASES = {
    'uniform-hairline': (b'1 0 0 1 0 0 cm', b'20 60 Td', b'', 1, 0),
    'cff-shear': (b'1 0.2 0.5 1 10 10 cm', b'10 60 Td', b'', 1, 1.5),
    'dashed': (b'1 0.2 0.5 1 10 10 cm', b'10 60 Td', b'[3 2] 1 d', 1, 1.5),
    'fill-stroke-clip': (b'1 0.2 0.4 1 10 10 cm', b'10 60 Td', b'', 6, 1.5),
    'scale': (b'2 0 0 0.7 10 15 cm', b'10 60 Td', b'', 1, 1.5),
    'shear': (b'1 0.2 0.5 1 10 10 cm', b'10 60 Td', b'', 1, 1.5),
    'reflection': (b'-1 0.1 0.2 1 180 10 cm', b'10 60 Td', b'', 1, 1.5),
    'text-matrix': (b'1 0 0 1 0 0 cm', b'1.2 0.3 0.2 0.8 20 60 Tm', b'', 1, 1.5),
    'horizontal-scale': (b'1 0.2 0.5 1 10 10 cm', b'10 60 Td', b'75 Tz', 1, 1.5),
    'fill-stroke': (b'1.5 0.1 0.3 0.8 10 10 cm', b'10 60 Td', b'', 2, 1.5),
    'clip': (b'1 0.2 0.4 1 10 10 cm', b'10 60 Td', b'', 5, 1.5),
    'hairline': (b'2 0.15 0.5 0.8 10 10 cm', b'10 60 Td', b'', 1, 0),
}


def main():
    font = (ROOT / 'crates/ofdrw/tests/fixtures/dejavu-serif.ttf').read_bytes()
    destination = ROOT / 'crates/ofdrw/tests/fixtures/pdf-text-affine'
    destination.mkdir(exist_ok=True)
    cases = []
    with tempfile.TemporaryDirectory(prefix='ofdrw-text-affine-') as temp:
        for name, (ctm, matrix, options, mode, width) in CASES.items():
            data = objects(font, simple=True)
            show = SHOW
            text = 'fiAfi'
            if name == 'cff-shear':
                cff = (ROOT / 'crates/ofdrw/tests/fixtures/test-name-cff.otf').read_bytes()
                data[4] = b'<< /Type /Font /Subtype /Type1 /BaseFont /CanvasCff /Encoding /WinAnsiEncoding /FontDescriptor 8 0 R /FirstChar 65 /LastChar 65 /Widths [722] >>'
                data[7] = data[7].replace(b'/FontFile2', b'/FontFile3').replace(b'DejaVuSerif', b'CanvasCff')
                data[8] = stream(f'/Subtype /OpenType /Length1 {len(cff)} /Filter /FlateDecode'.encode(), zlib.compress(cff))
                show, text = b'(AAA) Tj', 'AAA'
            data[2] = data[2].replace(b'144 96', b'200 160')
            content = (b'q ' + ctm + f' {width} w 0.8 0.1 0.2 rg 0.1 0.2 0.8 RG BT /F1 24 Tf {mode} Tr '.encode()
                       + options + b' ' + matrix + b' ' + show + b' ET ')
            if mode >= 4:
                content += b'0.1 0.7 0.2 rg 0 0 150 100 re f '
            content += b'Q'
            data[3] = stream(b'', content)
            source = destination / f'{name}.pdf'
            source.write_bytes(pdf_bytes(data))
            references = {}
            for dpi in [72, 144, 288, 576]:
                prefix = Path(temp) / f'{name}-{dpi}'
                target = prefix.with_suffix('.png')
                aa = 1 if name.endswith('hairline') else 4
                command = ['gs', '-q', '-dSAFER', '-dBATCH', '-dNOPAUSE', '-sDEVICE=png16m',
                           '-dGridFitTT=0', '-dAlignToPixels=0', f'-dGraphicsAlphaBits={aa}',
                           f'-dTextAlphaBits={aa}', f'-r{dpi}', f'-sOutputFile={target}', str(source)]
                # Ghostscript 10.00 ignores the dash array for this stroked TTF text;
                # Poppler preserves the graphics-state dash pattern.
                if name == 'dashed':
                    command = ['pdftoppm', '-r', str(dpi), '-png', '-singlefile',
                               str(source), str(prefix)]
                subprocess.run(command, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
                (destination / target.name).write_bytes(target.read_bytes())
                references[str(dpi)] = target.name
            cases.append({'name': name, 'file': source.name, 'text': text,
                          'renderer': 'Poppler' if name == 'dashed' else 'Ghostscript',
                          'global_matrix': [float(value) for value in ctm.split()[:-1]],
                          'line_width_pt': width, 'references': references})
    metadata = {'font_sha256': hashlib.sha256(font).hexdigest(), 'cases': cases,
                'raster_policy': 'Unhinted vector geometry; hairline reference uses target device pixels without supersampling.',
                'poppler_version': subprocess.run(['pdftoppm', '-v'], text=True, capture_output=True, check=True).stderr.splitlines()[0],
                'ghostscript_version': subprocess.check_output(['gs', '--version'], text=True).strip(),
                'cff_sha256': hashlib.sha256((ROOT / 'crates/ofdrw/tests/fixtures/test-name-cff.otf').read_bytes()).hexdigest()}
    (destination / 'reference.json').write_text(json.dumps(metadata, indent=2) + '\n')


if __name__ == '__main__':
    main()
