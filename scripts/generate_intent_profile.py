"""Author a synthetic ICC v2 CMYK/Lab profile with distinct rendering intents."""
from pathlib import Path
import struct


def fixed(value):
    return struct.pack('>i',round(value*65536))


def lut(inputs, outputs, color):
    matrix = b''.join(fixed(int(row==col)) for row in range(3) for col in range(3))
    ramp = bytes(range(256))
    return b'mft1'+bytes(4)+bytes([inputs,outputs,2,0])+matrix+ramp*inputs+bytes(color)*(2**inputs)+ramp*outputs


def profile():
    white = b''.join(fixed(n) for n in [.9642,1,.8249])
    description = b'OFD rendering intent regression\0'
    desc = b'desc'+bytes(4)+struct.pack('>I',len(description))+description+bytes(8+2+1+67)
    tags = [(b'desc',desc),(b'cprt',b'text'+bytes(4)+b'Apache-2.0; authored OFDRW Rust test data\0'),
            (b'wtpt',b'XYZ '+bytes(4)+white)]
    for intent,lightness in enumerate([64,128,192]):
        tags.append((f'A2B{intent}'.encode(),lut(4,3,[lightness,128,128])))
        tags.append((f'B2A{intent}'.encode(),lut(3,4,[0,0,0,0])))
    header = bytearray(128)
    header[4:8] = b'OFDR'
    header[8:12] = bytes([2,0x20,0,0])
    header[12:24] = b'prtrCMYKLab '
    header[24:36] = struct.pack('>6H',2026,1,1,0,0,0)
    header[36:40] = b'acsp'
    header[40:44] = b'APPL'
    header[68:80] = white
    header[80:84] = b'OFDR'
    offset = 132+len(tags)*12
    table = bytearray(struct.pack('>I',len(tags)))
    payload = bytearray()
    for name,data in tags:
        table += name+struct.pack('>II',offset,len(data))
        padded = data+bytes(-len(data)%4)
        payload += padded
        offset += len(padded)
    header[:4] = struct.pack('>I',offset)
    return bytes(header+table+payload)


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[1]/'crates/ofdrw/tests/fixtures'
    (root/'intent-test-cmyk.icc').write_bytes(profile())
