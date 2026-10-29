"""Author Separation PDFs and independent Ghostscript references (JPEG generation needs Pillow)."""

import argparse
from io import BytesIO
from pathlib import Path
import subprocess
import tempfile
import re
import zlib

from generate_nested_stitching_fixture import pdf_bytes, stream
from generate_mesh_shading_fixtures import record
from generate_patch_shading_fixtures import quad
from generate_pdf_glyph_fixtures import objects as glyph_objects


def tint_function(name):
    rgb = b'<< /FunctionType 2 /Domain [0 1] /C0 [1 1 1] /C1 [1 0 0] /N 2 >>'
    if name == 'separation-cmyk':
        return b'/DeviceCMYK', b'<< /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [0 .8 .5 .1] /N 1 >>'
    if name == 'separation-sampled':
        return b'/DeviceRGB', stream(b'/FunctionType 0 /Domain [0 1] /Range [0 1 0 1 0 1] '
                                    b'/Size [3] /Order 3 /BitsPerSample 8', bytes([255,255,255,0,255,0,0,0,255]))
    if name == 'separation-stitching':
        return b'/DeviceRGB', (b'<< /FunctionType 3 /Domain [0 1] /Bounds [.5] /Encode [0 1 0 1] /Functions ['
                               b'<< /FunctionType 2 /Domain [0 1] /C0 [1 1 1] /C1 [1 0 0] /N 1 >> '
                               b'<< /FunctionType 2 /Domain [0 1] /C0 [0 1 0] /C1 [0 0 1] /N 1 >>] >>')
    if name == 'separation-calculator':
        return b'/DeviceRGB', stream(b'/FunctionType 4 /Domain [0 1] /Range [0 1 0 1 0 1]', b'{ dup 1 exch sub exch .25 }')
    if name == 'separation-all' or 'none' in name:
        return b'/DeviceRGB', b'<< /FunctionType 2 /Domain [0 1] /C0 [0 0 1] /C1 [0 0 1] /N 1 >>'
    return b'/DeviceRGB', rgb


def fixture(name, root, reference_oracle=False):
    alternate, transform = tint_function(name)
    colorant = b'None' if 'none' in name else (b'All' if name == 'separation-all' else b'Red')
    objects = [b'<< /Type /Catalog /Pages 2 0 R >>', b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>', b'', b'',
               b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>']
    def add(value):
        objects.append(value)
        return f'{len(objects)} 0 R'.encode()
    function = add(transform)
    space = add(b'[/Separation /'+colorant+b' '+alternate+b' '+function+b']')
    indexed = add(b'[/Indexed '+space+b' 3 <0055aaff>]')
    samples = bytes(x*17 for _ in range(16) for x in range(16))
    image_dict = b'/Type /XObject /Subtype /Image /Width 16 /Height 16 /ColorSpace '+space+b' /BitsPerComponent 8'
    if name == 'separation-image16-mask':
        samples = b''.join((x*4369).to_bytes(2,'big') for _ in range(16) for x in range(16))
        image_dict = image_dict.replace(b'/BitsPerComponent 8', b'/BitsPerComponent 16')
        if reference_oracle:
            # ISO 32000-1 8.9.6.4 compares the original 16-bit samples.
            # Expand that predicate into an independent explicit alpha mask;
            # Ghostscript 10.02.1 incorrectly masks the entire original image.
            alpha = bytes(0 if x*4369 <= 30000 else 255 for _ in range(16) for x in range(16))
            alpha_ref = add(stream(b'/Type /XObject /Subtype /Image /Width 16 /Height 16 '
                                   b'/ColorSpace /DeviceGray /BitsPerComponent 8', alpha))
            image_dict += b' /SMask '+alpha_ref
        else:
            image_dict += b' /Mask [0 30000]'
    if name == 'separation-jpeg':
        from PIL import Image
        output = BytesIO()
        Image.frombytes('L',(16,16),samples).save(output,format='JPEG',quality=100)
        samples = output.getvalue()
        image_dict += b' /Filter /DCTDecode'
    image = add(stream(image_dict,samples))
    indexed_image = add(stream(b'/Type /XObject /Subtype /Image /Width 80 /Height 80 /BitsPerComponent 2 /ColorSpace '+indexed, bytes([0x00]*5+[0x55]*5+[0xaa]*5+[0xff]*5)*80))
    mask = add(stream(b'/Type /XObject /Subtype /Image /Width 8 /Height 8 /ImageMask true /BitsPerComponent 1',bytes([0])*8))
    normal_image = add(stream(b'/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8', bytes([0,255,0])))
    shading_function = add(b'<< /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >>')
    axial = add(b'<< /ShadingType 2 /ColorSpace '+space+b' /Coords [8 0 88 0] /Extend [true true] /Function '+shading_function+b' >>')
    radial = add(b'<< /ShadingType 3 /ColorSpace '+space+b' /Coords [48 48 0 48 48 40] /Extend [true true] /Function '+shading_function+b' >>')
    mesh_data = b''.join(record(0,x,y,(t,),8,8,2) for x,y,t in [(0,0,0),(1,0,.5),(0,1,1)])
    mesh = add(stream(b'/ShadingType 4 /ColorSpace '+space+b' /BitsPerCoordinate 8 /BitsPerComponent 8 /BitsPerFlag 2 /Decode [8 88 8 88 0 1]',mesh_data))
    fields = [(0,2)]
    fields += [(round(value*255),8) for point in quad((0,0),(0,1),(1,1),(1,0))[:12] for value in point]
    fields += [(t,8) for t in [0,255,51,204]]
    bits = ''.join(f'{value:0{width}b}' for value,width in fields)
    bits += '0' * (-len(bits)%8)
    patch = add(stream(b'/ShadingType 6 /ColorSpace '+space+b' /BitsPerCoordinate 8 /BitsPerComponent 8 /BitsPerFlag 2 /Decode [8 88 8 88 0 1]',int(bits,2).to_bytes(len(bits)//8,'big')))
    plane_function = add(stream(b'/FunctionType 4 /Domain [0 1 0 1] /Range [0 1]',b'{ add .5 mul }'))
    plane = add(b'<< /ShadingType 1 /ColorSpace '+space+b' /Matrix [80 0 0 80 8 8] /Function '+plane_function+b' >>')
    normal_function = add(b'<< /FunctionType 2 /Domain [0 1] /C0 [0 1 1] /C1 [1 1 0] /N 1 >>')
    normal_shading = add(b'<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [48 48 88 48] /Extend [true true] /Function '+normal_function+b' >>')
    background = b'.8 g 0 0 96 96 re f '
    if 'none' in name:
        background = b'1 0 0 rg 0 0 96 96 re f '
    content = background+b'/CS cs '
    if name in ['separation-image','separation-image16-mask','separation-jpeg']:
        content += b'q 80 0 0 80 8 8 cm /Im Do Q'
    elif name == 'separation-indexed':
        content += b'q 80 0 0 80 8 8 cm /Ix Do Q'
    elif name == 'separation-none':
        content += (b'.5 scn 8 8 80 80 re f /CS CS .5 SCN 12 w 8 8 80 80 re S '
                    b'BT /F1 24 Tf 12 40 Td (none) Tj ET '
                    b'q 80 0 0 80 8 8 cm /Im Do /Ix Do /Mk Do Q '
                    b'/Ax sh /Ms sh /Pt sh /Pl sh')
    elif name == 'separation-none-clipping':
        content += b'8 8 40 80 re W f 0 1 0 rg 0 0 96 96 re f'
    elif name == 'separation-none-independent':
        content += b'q 36 0 0 36 8 8 cm /Normal Do Q q 48 48 40 40 re W n /NormalShade sh Q'
    elif name == 'separation-shading':
        content += b'q 0 0 96 48 re W n /Ax sh Q q 0 48 96 48 re W n /Rad sh Q'
    elif name == 'separation-mesh':
        content += b'/Ms sh'
    elif name == 'separation-patch':
        content += b'/Pt sh'
    elif name == 'separation-plane':
        content += b'/Pl sh'
    else:
        content += b'1 scn 8 8 16 80 re f .75 scn 24 8 16 80 re f .5 scn 40 8 16 80 re f .25 scn 56 8 16 80 re f 0 scn 72 8 16 80 re f '
        content += b'/CS CS .6 SCN 2 w 10 10 76 76 re S '
        content += b'.5 scn BT /F1 18 Tf 10 40 Td (spot) Tj ET'
    objects[2] = (b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 96] /Contents 4 0 R /Resources << '
                  b'/Font << /F1 5 0 R >> /ColorSpace << /CS '+space+b' /IX '+indexed+b' >> '
                  b'/XObject << /Im '+image+b' /Ix '+indexed_image+b' /Mk '+mask+b' /Normal '+normal_image+b' >> '
                  b'/Shading << /Ax '+axial+b' /Rad '+radial+b' /Ms '+mesh+b' /Pt '+patch+b' /Pl '+plane+b' /NormalShade '+normal_shading+b' >> >> >>')
    if reference_oracle and name == 'separation-stitching':
        # ISO 32000-1 7.10.4 makes the right interval active at a stitching bound.
        # Ghostscript 10.02.1's Separation path selects the left value at .5;
        # independently expand these known functions into process RGB instead.
        assert name == 'separation-stitching'
        content = content.replace(b'/CS cs',b'/DeviceRGB cs').replace(b'/CS CS',b'/DeviceRGB CS')
        def convert(match):
            tint = float(match[1])
            rgb = (1,1-2*tint,1-2*tint) if tint < .5 else (0,2-2*tint,2*tint-1)
            operator = b'rg' if match[2] == b'scn' else b'RG'
            return (' '.join(format(value,'.12g') for value in rgb)+' ').encode()+operator
        content = re.sub(rb'([0-9.]+) (scn|SCN)',convert,content)
    if reference_oracle and name == 'separation-none':
        # /None must leave every previously painted pixel unchanged.
        content = background
    objects[3] = stream(b'',content)
    if name == 'separation-none-text-clip':
        objects = glyph_objects((root/'dejavu-serif.ttf').read_bytes())
        function = f'{len(objects)+1} 0 R'.encode()
        objects.append(transform)
        objects[2] = objects[2].replace(b'/Resources <<',b'/Resources << /ColorSpace << /CS [/Separation /None /DeviceRGB '+function+b'] >>')
        body = objects[3].split(b'stream\n',1)[1].rsplit(b'endstream',1)[0].rstrip(b'\n')
        objects[3] = stream(b'',body.replace(b'BT',b'/CS cs BT 4 Tr',1)+b' 0 1 0 rg 0 0 144 96 re f')
    return pdf_bytes(objects)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--references',action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]/'crates/ofdrw/tests/fixtures'
    for suffix in ['paths','cmyk','sampled','stitching','calculator','image','image16-mask','jpeg','indexed',
                   'all','none','none-clipping','none-independent','none-text-clip','shading','mesh','patch','plane']:
        name = 'separation-'+suffix
        target = root/(name+'.pdf')
        target.write_bytes(fixture(name,root))
        if args.references:
            fast_color = ['-dUseFastColor'] if suffix in ['cmyk','all'] else []
            with tempfile.TemporaryDirectory() as temporary:
                reference_source = target
                if suffix in ['stitching', 'image16-mask', 'none']:
                    reference_source = Path(temporary)/'reference-oracle.pdf'
                    reference_source.write_bytes(fixture(name,root,reference_oracle=True))
                subprocess.run(['gs','-q','-dSAFER','-dBATCH','-dNOPAUSE',*fast_color,'-sDEVICE=png16m','-r72',
                                '-dTextAlphaBits=4','-dGraphicsAlphaBits=1',f'-sOutputFile={root/(name+".png")}',str(reference_source)],check=True)
