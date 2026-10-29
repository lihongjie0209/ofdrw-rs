"""Author lossless multi-component JPX PDFs (glymur/numpy/packaging + OpenJPEG)."""
import argparse
from pathlib import Path
import subprocess
import tempfile

from generate_nested_stitching_fixture import pdf_bytes, stream

CASES = [('two',2,8,None),('five-container',5,8,'container'),('four',4,8,None),('three-alpha',3,8,'embedded'),('five',5,8,None),('thirty-two',32,8,None),
         ('five-mask',5,8,'key'),('five-sixteen-mask',5,16,'key'),('five-soft-mask',5,8,'soft'),('five-no-bpc',5,8,'omit-bpc')]


def container(data, channels, bits, alpha=False):
    def box(kind, body):
        return (len(body)+8).to_bytes(4,'big')+kind+body
    total = channels + alpha
    ihdr = (16).to_bytes(4,'big')*2+total.to_bytes(2,'big')+bytes([bits-1,7,0,0])
    colr = bytes([1,0,0])+(16).to_bytes(4,'big')
    cdef = total.to_bytes(2,'big')
    for channel in range(total):
        cdef += channel.to_bytes(2,'big')+(1 if alpha and channel == channels else 0).to_bytes(2,'big')+(0 if alpha and channel == channels else channel+1).to_bytes(2,'big')
    return box(b'jP  ',b'\x0d\x0a\x87\x0a')+box(b'ftyp',b'jp2 \0\0\0\0jp2 ')+box(b'jp2h',box(b'ihdr',ihdr)+box(b'colr',colr)+box(b'cdef',cdef))+box(b'jp2c',data)


def fixture(channels, bits, data, mask=None, oracle=False):
    names = b' '.join(f'/Ink{i}'.encode() for i in range(channels))
    function = stream(b'/FunctionType 4 /Domain ['+b'0 1 '*channels+b'] /Range [0 1 0 1 0 1]',
                      b'{ '+b'pop '*(channels-2)+b'1 exch sub exch 1 exch sub exch .25 }')
    color_space = b'[/DeviceN ['+names+b'] /DeviceRGB 5 0 R]'
    image_dict = b'/Type /XObject /Subtype /Image /Width 16 /Height 16 /Filter /JPXDecode /ColorSpace '+color_space
    if mask != 'omit-bpc' or oracle:
        image_dict += f' /BitsPerComponent {bits}'.encode()
    extra = []
    if mask == 'embedded':
        image_dict += b' /SMaskInData 1'
    if mask == 'key' and not (oracle and bits == 16):
        maximum = (1<<bits)-1
        image_dict += f' /Mask [0 {maximum//2} 0 {maximum//2}'.encode()+f' 0 {maximum}'.encode()*(channels-2)+b']'
    if mask == 'soft' or (mask == 'key' and oracle and bits == 16):
        image_dict += b' /SMask 7 0 R'
        extra.append(stream(b'/Type /XObject /Subtype /Image /Width 16 /Height 16 /ColorSpace /DeviceGray /BitsPerComponent 8',(bytes([128])*256 if mask == 'soft' else bytes(0 if x <= 7 and y <= 7 else 255 for y in range(16) for x in range(16)))))
    return pdf_bytes([
        b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 96] /Contents 4 0 R /Resources << /XObject << /Im 6 0 R >> >> >>',
        stream(b'',b'.8 g 0 0 96 96 re f q 80 0 0 80 8 8 cm /Im Do Q'),function,stream(image_dict,data),*extra
    ])


if __name__ == '__main__':
    import glymur
    import numpy as np
    parser = argparse.ArgumentParser()
    parser.add_argument('--references',action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]/'crates/ofdrw/tests/fixtures'
    for suffix,channels,bits,mask in CASES:
        with tempfile.TemporaryDirectory() as temporary:
            encoded = Path(temporary)/'samples.j2k'
            values = np.zeros((16,16,channels+(mask == 'embedded')),dtype=np.uint8 if bits==8 else np.uint16)
            ramp = np.arange(16,dtype=values.dtype)*(((1<<bits)-1)//15)
            values[:,:,0] = ramp
            values[:,:,1] = ramp[:,None]
            if mask == 'embedded':
                values[:,:,-1] = 128
            glymur.Jp2k(str(encoded),data=values,numres=1,mct=False)
            data = encoded.read_bytes()
            raw_data = data
            if mask in ['embedded','container']:
                data = container(data,channels,bits,alpha=(mask == 'embedded'))
        name = 'devicen-jpx-'+suffix
        target = root/(name+'.pdf')
        target.write_bytes(fixture(channels,bits,data,mask))
        if args.references:
            with tempfile.TemporaryDirectory() as temporary:
                source = target
                if mask in ['omit-bpc','container'] or (mask == 'key' and bits == 16):
                    # Ghostscript 10.02.1 guesses one-bit DeviceN samples when
                    # BPC is omitted, compares 16-bit color keys after
                    # reducing precision, and uses container color metadata
                    # despite an explicit PDF ColorSpace. Supply independent metadata
                    # or alpha while keeping the original importer input.
                    source = Path(temporary)/'oracle.pdf'
                    source.write_bytes(fixture(channels,bits,raw_data if mask == 'container' else data,None if mask == 'container' else mask,oracle=True))
                subprocess.run(['gs','-q','-dSAFER','-dBATCH','-dNOPAUSE','-sDEVICE=png16m','-r72','-dGraphicsAlphaBits=1',f'-sOutputFile={root/(name+".png")}',str(source)],check=True)
