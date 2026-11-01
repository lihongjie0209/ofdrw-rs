"""Generate vector canvas OFDs with the pinned Java OFDRW implementation.

The Java checkout is read-only; the harness and intermediate outputs live in a
TemporaryDirectory. Reference OFDs and parsed path attributes are independent of
Rust's canvas executor.
"""
from pathlib import Path
import json
import os
import subprocess
import tempfile
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT.parent
PIN = "84e9eb260ba6fd351ecccf08a01a40305d4af635"
SOURCE = r'''
import java.nio.file.*;
import org.ofdrw.layout.*;
import org.ofdrw.layout.element.*;
import org.ofdrw.layout.element.canvas.*;
public class CanvasReference {
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
        write(root,"path-state",c -> {
            c.beginPath().moveTo(1,2).lineTo(9,2).save();
            c.beginPath().rect(2,4,6,3).strokeRect(9,8,2,2).stroke();
            c.restore().stroke().fillRect(1,12,4,2).stroke();
        });
        write(root,"deferred-transform",c -> {
            c.beginPath().moveTo(1,1).lineTo(4,2).translate(3,4).scale(2,3).stroke();
            c.setTransform(1,0,0,1,0,0).transform(0,1,-1,0,15,1).stroke();
        });
        write(root,"curves",c -> {
            c.moveTo(2,2).quadraticCurveTo(4,0,6,2).bezierCurveTo(6,4,8,4,8,2)
             .arc(3,2,25,true,false,12,8).closePath().stroke();
            c.beginPath().arc(10,10,3,0,360,true).stroke();
        });
        write(root,"fill-alpha",c -> {
            c.setLineWidth(0.7).setFillColor(0,128,255).setStrokeColor(255,0,0)
             .setGlobalAlpha(0.5).rect(2,2,10,8).fill().fillRect(14,2,4,6).stroke();
        });
        write(root,"frozen-clip",c -> {
            c.translate(2,1).rect(0,0,8,8).clip().beginPath()
             .setTransform(1,0,0,1,10,0).fillRect(0,0,14,14);
        });
    }
}
'''


def main():
    revision = subprocess.check_output(["git", "-C", str(JAVA), "rev-parse", "HEAD"], text=True).strip()
    if revision != PIN:
        raise SystemExit("Check out the pinned Java reference first")
    if not (JAVA / "ofdrw-layout/target/classes").is_dir():
        raise SystemExit("Build the sibling Java layout module first")
    classpath = os.pathsep.join(map(str, [*sorted(JAVA.glob("*/target/classes")), *sorted((Path.home()/".m2/repository").rglob("*.jar"))]))
    destination = ROOT/"crates/ofdrw/tests/fixtures/canvas-java"
    destination.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="ofdrw-canvas-java-") as temporary:
        directory = Path(temporary)
        source = directory/"CanvasReference.java"
        source.write_text(SOURCE)
        def argfile(name, arguments):
            path = directory/name
            path.write_text("\n".join('"'+v.replace("\\", "\\\\").replace('"', '\\"')+'"' for v in arguments))
            return "@"+str(path)
        subprocess.run(["javac",argfile("compile.args",["-proc:none","-cp",classpath,str(source)])],check=True)
        subprocess.run(["java",argfile("run.args",["-Djava.awt.headless=true","-cp",str(directory)+os.pathsep+classpath,"CanvasReference",str(directory)])],check=True)
        cases=[]
        for name in ["path-state","deferred-transform","curves","fill-alpha","frozen-clip"]:
            data=(directory/(name+".ofd")).read_bytes()
            (destination/(name+".ofd")).write_bytes(data)
            with zipfile.ZipFile(directory/(name+".ofd")) as archive:
                roots=[ET.fromstring(archive.read(n)) for n in archive.namelist() if n.endswith('.xml')]
                params={node.get('ID'):node for root in roots for node in root.iter() if node.tag.endswith('}DrawParam')}
                objects=[]
                for root in roots:
                    if root.tag.split('}')[-1]!='Page':continue
                    for node in root.iter():
                        if node.tag.split('}')[-1]!='PathObject':continue
                        draw=params.get(node.get('DrawParam'))
                        width=node.get('LineWidth')
                        if width is None and draw is not None:width=draw.get('LineWidth')
                        objects.append({"path":" ".join(next(ch.text for ch in node if ch.tag.endswith('}AbbreviatedData')).split()),
                            "line_width":float(width) if width is not None else 0.353,
                            "boundary":[float(v) for v in node.get('Boundary').split()],
                            "ctm":[float(v) for v in node.get('CTM','1 0 0 1 0 0').split()],
                            "fill":node.get('Fill','false')=='true',"stroke":node.get('Stroke','true')=='true',
                            "alpha":int(node.get('Alpha','255'))})
                case={"name":name,"objects":objects}
                if name=="frozen-clip":
                    # Pinned DrawContext emits inverse object CTM but omits the
                    # TransFlag=true needed for that encoding in OFD 1.2.
                    # Preserve the raw Java output and make the one-attribute
                    # repair explicit; no path/matrix is calculated by Rust.
                    repaired=name+"-transflag.ofd"
                    with zipfile.ZipFile(destination/repaired,"w",zipfile.ZIP_DEFLATED) as fixed:
                        for info in archive.infolist():
                            content=archive.read(info.filename)
                            if info.filename.endswith("Content.xml"):
                                assert content.count(b"<ofd:Clips>")==1
                                content=content.replace(b"<ofd:Clips>",b'<ofd:Clips TransFlag="true">')
                            fixed.writestr(info,content)
                    case["render_reference"]=repaired
                    case["normalization"]="Only set Clips TransFlag=true for Java's existing inverse-CTM clip encoding; preserve original OFD alongside it."
                cases.append(case)
    output=destination/"reference.json"
    output.write_text(json.dumps({"java_revision":revision,"page_size":[40,30],"boundary":[4,3,24,20],"cases":cases},indent=2)+"\n")
    print(output.relative_to(ROOT))

if __name__=="__main__":main()
