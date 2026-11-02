"""Generate image canvas references with the pinned, read-only Java checkout."""
from pathlib import Path
import json
import base64
import os
import struct
import subprocess
import tempfile
import zlib

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT.parent
PIN = "84e9eb260ba6fd351ecccf08a01a40305d4af635"
SOURCE = r'''
import java.nio.file.*;
import org.ofdrw.layout.*;
import org.ofdrw.layout.element.*;
import org.ofdrw.layout.element.canvas.*;
public class CanvasImageReference {
    static void write(Path root, String name, Drawer drawer) throws Exception {
        try (OFDDoc doc = new OFDDoc(root.resolve(name+".ofd"))) {
            VirtualPage page = new VirtualPage(40d,30d);
            Canvas canvas = new Canvas(24d,20d);
            canvas.setPosition(Position.Absolute).setX(4d).setY(3d).setBorder(0d);
            canvas.setDrawer(drawer);
            page.add(canvas); doc.addVPage(page);
        }
    }
    public static void main(String[] args) throws Exception {
        Path root = Paths.get(args[0]);
        Path image = root.resolve("source.png");
        write(root,"sized",c -> {
            c.translate(2,1).scale(2,3).setGlobalAlpha(0.5)
             .drawImage(image,1,2,6,4);
        });
        write(root,"native-size",c -> {
            c.PPM=2;
            c.drawImage(image,10,2);
        });
        write(root,"mirrored-x",c -> c.drawImage(image,14,2,-6,4));
        write(root,"mirrored-y",c -> c.drawImage(image,2,10,6,-4));
        write(root,"zero-width",c -> c.drawImage(image,2,2,0,4));
        write(root,"cropped",c -> {
            c.PPM=2;
            c.drawImage(image,-0.9,0.9,2.9,2,2,2,10,8);
        });
    }
}
'''


def png():
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    rows = b''.join(b'\0' + bytes(v for x in range(6) for v in (255 if x < 3 else 0, 255 if y < 2 else 0, 80, 255)) for y in range(4))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 6, 4, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b'')


def requests(data):
    image = {"data_base64": base64.b64encode(data).decode("ascii"), "format": "png"}
    cases = []
    for name, rect in [("mirrored-x", [14,2,-6,4]), ("mirrored-y", [2,10,6,-4]), ("zero-width", [2,2,0,4])]:
        cases.append({"name": name, "commands": [{"type":"draw_image", **image, "rect":rect}]})
    cases.extend([
        {"name":"sized", "commands":[{"type":"translate","offset":[2,1]}, {"type":"scale","factors":[2,3]}, {"type":"set_global_alpha","alpha":0.5}, {"type":"draw_image", **image, "rect":[1,2,6,4]}]},
        {"name":"native-size", "commands":[{"type":"draw_image_at", **image, "point":[10,2], "pixels_per_mm":2}]},
        {"name":"cropped", "commands":[{"type":"draw_image_region", **image, "source":[-0.9,0.9,2.9,2], "rect":[2,2,10,8], "pixels_per_mm":2}]},
    ])
    return {"boundary":[4,3,24,20], "cases":cases}


def main():
    revision = subprocess.check_output(['git', '-C', str(JAVA), 'rev-parse', 'HEAD'], text=True).strip()
    if revision != PIN:
        raise SystemExit('Check out the pinned Java reference first')
    if not (JAVA/'ofdrw-layout/target/classes').is_dir():
        raise SystemExit('Build the sibling Java layout module first')
    classpath = os.pathsep.join(map(str, [*sorted(JAVA.glob('*/target/classes')), *sorted((Path.home()/'.m2/repository').rglob('*.jar'))]))
    destination = ROOT/'crates/ofdrw/tests/fixtures/canvas-images-java'
    destination.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='ofdrw-canvas-images-') as temp:
        root = Path(temp)
        (root/'CanvasImageReference.java').write_text(SOURCE)
        (root/'source.png').write_bytes(png())
        for executable, arguments in [
            ('javac', ['-proc:none', '-cp', classpath, str(root/'CanvasImageReference.java')]),
            ('java', ['-cp', os.pathsep.join([str(root), classpath]), 'CanvasImageReference', str(root)]),
        ]:
            argument_file = root/f'{executable}.args'
            argument_file.write_text('\n'.join(json.dumps(arg) for arg in arguments)+'\n')
            subprocess.run([executable, f'@{argument_file}'], check=True)
        for name in ['source.png', 'sized.ofd', 'native-size.ofd', 'cropped.ofd', 'mirrored-x.ofd', 'mirrored-y.ofd', 'zero-width.ofd']:
            (destination/name).write_bytes((root/name).read_bytes())
    (destination/'requests.json').write_text(json.dumps(requests(png()), indent=2)+'\n')
    (destination/'reference.json').write_text(json.dumps({'java_revision': PIN, 'pixels_per_mm': 2, 'source_units': 'millimeters truncated to integer pixels', 'cases': ['sized', 'native-size', 'cropped', 'mirrored-x', 'mirrored-y', 'zero-width']}, indent=2)+'\n')


if __name__ == '__main__':
    main()
