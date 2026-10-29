"""Author DeviceN composite-color PDFs and independent Ghostscript references."""
import argparse
from pathlib import Path
import subprocess
import tempfile

from generate_nested_stitching_fixture import pdf_bytes, stream
from generate_mesh_shading_fixtures import record
from generate_patch_shading_fixtures import quad

CASES = ['paths', 'sampled', 'cubic', 'cmyk', 'three-input', 'image', 'image-five',
         'image-thirty-two', 'image16-mask', 'indexed', 'none', 'mixed-none', 'none-clipping',
         'nchannel', 'shading', 'mesh', 'patch', 'plane']


def fixture(name, oracle=False):
    suffix = name.removeprefix('devicen-')
    count = 32 if suffix == 'image-thirty-two' else (5 if suffix == 'image-five' else (3 if suffix == 'three-input' else 2))
    names = [b'/Orange', b'/Green']+[f'/Extra{i}'.encode() for i in range(count-2)]
    if suffix in ['none', 'none-clipping']:
        names = [b'/None'] * count
    if suffix == 'mixed-none':
        names[0] = b'/None'
    domain = b' '.join([b'0 1'] * count)
    transform = stream(b'/FunctionType 4 /Domain ['+domain+b'] /Range [0 1 0 1 0 1]',
                       b'{ 1 exch sub exch 1 exch sub exch .25 }')
    alternate = b'/DeviceRGB'
    if suffix in ['image-five','image-thirty-two']:
        transform = stream(b'/FunctionType 4 /Domain ['+domain+b'] /Range [0 1 0 1 0 1]',
                           b'{ '+b'pop '*(count-2)+b'1 exch sub exch 1 exch sub exch .25 }')
    if suffix == 'three-input':
        transform = stream(b'/FunctionType 4 /Domain ['+domain+b'] /Range [0 1 0 1 0 1]',
                           b'{ 1 exch sub 3 1 roll 1 exch sub 3 1 roll 1 exch sub 3 1 roll }')
    if suffix == 'cmyk':
        alternate = b'/DeviceCMYK'
        transform = stream(b'/FunctionType 4 /Domain ['+domain+b'] /Range [0 1 0 1 0 1 0 1]', b'{ 0 0 }')
    if suffix in ['sampled', 'cubic']:
        size = 3 if suffix == 'cubic' else 2
        samples = bytes(channel for y in range(size) for x in range(size)
                        for channel in [round(255*(1-(x/(size-1))**2)), round(255*(1-y/(size-1))), 64])
        transform = stream(f'/FunctionType 0 /Domain [0 1 0 1] /Range [0 1 0 1 0 1] /Size [{size} {size}] /BitsPerSample 8 /Order {3 if size == 3 else 1}'.encode(),samples)
    objects = [b'<< /Type /Catalog /Pages 2 0 R >>', b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',b'',b'',
               b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>']
    def add(value):
        objects.append(value)
        return f'{len(objects)} 0 R'.encode()
    function = add(transform)
    attributes = b''
    if suffix == 'nchannel':
        attributes = (b' << /Subtype /NChannel /Colorants << '
                      b'/Orange [/Separation /Orange /DeviceRGB << /FunctionType 2 /Domain [0 1] /C0 [1 1 1] /C1 [1 0 0] /N 1 >>] '
                      b'/Green [/Separation /Green /DeviceRGB << /FunctionType 2 /Domain [0 1] /C0 [1 1 1] /C1 [0 1 0] /N 1 >>] >> >>')
    space = add(b'[/DeviceN ['+b' '.join(names)+b'] '+alternate+b' '+function+attributes+b']')
    indexed = add(b'[/Indexed '+space+b' 3 <00ff55aaaaffff00>]')
    samples = bytes(value for y in range(16) for x in range(16)
                    for value in ([x*17,y*17]+[0]*(count-2)))
    image_dict = b'/Type /XObject /Subtype /Image /Width 16 /Height 16 /ColorSpace '+space+b' /BitsPerComponent 8'
    if suffix == 'image16-mask':
        samples = b''.join(value.to_bytes(2,'big') for y in range(16) for x in range(16) for value in [x*4369,y*4369])
        image_dict = image_dict.replace(b'/BitsPerComponent 8',b'/BitsPerComponent 16')
        if oracle:
            alpha = bytes(0 if x*4369 <= 30000 and y*4369 <= 30000 else 255 for y in range(16) for x in range(16))
            mask = add(stream(b'/Type /XObject /Subtype /Image /Width 16 /Height 16 /ColorSpace /DeviceGray /BitsPerComponent 8',alpha))
            image_dict += b' /SMask '+mask
        else:
            image_dict += b' /Mask [0 30000 0 30000]'
    image = add(stream(image_dict,samples))
    indexed_image = add(stream(b'/Type /XObject /Subtype /Image /Width 80 /Height 80 /ColorSpace '+indexed+b' /BitsPerComponent 2',bytes([0]*5+[0x55]*5+[0xaa]*5+[0xff]*5)*80))
    shading_function = add(b'<< /FunctionType 2 /Domain [0 1] /C0 [0 1] /C1 [1 0] /N 1 >>')
    axial = add(b'<< /ShadingType 2 /ColorSpace '+space+b' /Coords [8 0 88 0] /Extend [true true] /Function '+shading_function+b' >>')
    mesh = add(stream(b'/ShadingType 4 /ColorSpace '+space+b' /BitsPerCoordinate 8 /BitsPerComponent 8 /BitsPerFlag 2 /Decode [8 88 8 88 0 1 0 1]',b''.join(record(0,x,y,values,8,8,2) for x,y,values in [(0,0,(0,0)),(1,0,(1,0)),(0,1,(0,1))])))
    fields = [(0,2)]+[(round(v*255),8) for point in quad((0,0),(0,1),(1,1),(1,0))[:12] for v in point]+[(v,8) for color in [(0,0),(0,255),(255,255),(255,0)] for v in color]
    bits = ''.join(f'{value:0{width}b}' for value,width in fields)
    bits += '1' * (-len(bits)%8)
    patch = add(stream(b'/ShadingType 6 /ColorSpace '+space+b' /BitsPerCoordinate 8 /BitsPerComponent 8 /BitsPerFlag 2 /Decode [8 88 8 88 0 1 0 1]',int(bits,2).to_bytes(len(bits)//8,'big')))
    plane_function = add(stream(b'/FunctionType 4 /Domain [0 1 0 1] /Range [0 1 0 1]',b'{ }'))
    plane = add(b'<< /ShadingType 1 /ColorSpace '+space+b' /Matrix [80 0 0 80 8 8] /Function '+plane_function+b' >>')
    background = b'.8 g 0 0 96 96 re f '
    content = background+b'/CS cs '
    if suffix in ['image','image-five','image-thirty-two','image16-mask']:
        content += b'q 80 0 0 80 8 8 cm /Im Do Q'
    elif suffix == 'indexed':
        content += b'q 80 0 0 80 8 8 cm /Ix Do Q'
    elif suffix in ['shading','mesh','patch','plane']:
        content += {'shading':b'/Ax sh','mesh':b'/Ms sh','patch':b'/Pt sh','plane':b'/Pl sh'}[suffix]
    elif suffix == 'none':
        content += b'.5 .5 scn 8 8 80 80 re f /CS CS .5 .5 SCN 2 w 8 8 80 80 re S BT /F1 24 Tf 12 40 Td (none) Tj ET q 80 0 0 80 8 8 cm /Im Do Q /Ax sh /Ms sh /Pt sh /Pl sh'
        if oracle:
            content = background
    elif suffix == 'none-clipping':
        content += b'8 8 40 80 re W f 0 1 0 rg 0 0 96 96 re f'
    else:
        extra = b' .5' if count == 3 else b''
        for i in range(5):
            content += f'{i/4} {1-i/4}'.encode()+extra+b' scn '+f'{8+16*i} 8 16 80 re f '.encode()
        content += b'.5 .5'+extra+b' scn BT /F1 18 Tf 10 40 Td (inks) Tj ET'
        content += b'/CS CS .25 .75'+extra+b' SCN 2 w 10 10 76 76 re S'
    objects[2] = (b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 96] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> /ColorSpace << /CS '+space+b' >> /XObject << /Im '+image+b' /Ix '+indexed_image+b' >> /Shading << /Ax '+axial+b' /Ms '+mesh+b' /Pt '+patch+b' /Pl '+plane+b' >> >> >>')
    objects[3] = stream(b'',content)
    return pdf_bytes(objects)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--references',action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]/'crates/ofdrw/tests/fixtures'
    for suffix in CASES:
        name = 'devicen-'+suffix
        target = root/(name+'.pdf')
        target.write_bytes(fixture(name))
        if args.references:
            with tempfile.TemporaryDirectory() as temporary:
                source = target
                if suffix in ['none','image16-mask']:
                    source = Path(temporary)/'oracle.pdf'
                    source.write_bytes(fixture(name,oracle=True))
                subprocess.run(['gs','-q','-dSAFER','-dBATCH','-dNOPAUSE',*(['-dUseFastColor'] if suffix=='cmyk' else []),'-sDEVICE=png16m','-r72','-dTextAlphaBits=4','-dGraphicsAlphaBits=1',f'-sOutputFile={root/(name+".png")}',str(source)],check=True)
