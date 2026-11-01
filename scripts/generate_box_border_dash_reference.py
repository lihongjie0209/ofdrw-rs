"""Generate dashed border paths using the pinned Java OFDRW build.

Build the sibling reference first; this generator never calls Rust.
"""
from pathlib import Path
import os
import json
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT.parent
PIN = "84e9eb260ba6fd351ecccf08a01a40305d4af635"
SOURCE = r"""
import java.util.concurrent.atomic.AtomicInteger;
import org.ofdrw.layout.element.*;
import org.ofdrw.layout.engine.render.DivRender;
import org.ofdrw.core.basicStructure.pageObj.layer.block.*;
public class BoxBorderDashReference {
    static void emit(String name, Double[] dash, boolean unequal) {
        Div div = new Div(10d,5d).setXY(2d,2d).setBorder(1d).setBorderDash(dash);
        if (unequal) div.setBorder(1d,2d,3d,4d);
        CT_PageBlock layer = new CT_PageBlock();
        DivRender.render(layer, div, new AtomicInteger());
        for (var block : layer.getPageBlocks()) {
            PathObject p = (PathObject) block;
            System.out.printf("%s\t%s\t%s\t%s\t%s\t%s%n", name, p.getBoundary(),
                p.getLineWidth(), p.getDashPattern(), p.getDashOffset(), p.getAbbreviatedData());
        }
    }
    public static void main(String[] args) {
        emit("one",new Double[]{3d},false);
        emit("pairs",new Double[]{3d,2d},false);
        emit("offset",new Double[]{1d,3d,2d},false);
        emit("long",new Double[]{1d,3d,2d,4d,1d},false);
        emit("unequal",new Double[]{1d,3d,2d},true);
        emit("negative_offset",new Double[]{-1d,3d,2d},false);
    }
}
"""

def main():
    revision = subprocess.run(["git", "-C", str(JAVA), "rev-parse", "HEAD"], check=True, capture_output=True, text=True).stdout.strip()
    if revision != PIN:
        raise SystemExit("Check out and build the pinned Java OFDRW reference first")
    if not (JAVA / "ofdrw-layout/target/classes").is_dir():
        raise SystemExit("Build the sibling Java ofdrw-layout module first")
    classpath = os.pathsep.join(str(p) for p in [*sorted(JAVA.glob("*/target/classes")), *sorted((Path.home() / ".m2/repository").rglob("*.jar"))])
    with tempfile.TemporaryDirectory(prefix="ofdrw-border-dash-reference-") as directory:
        source = Path(directory) / "BoxBorderDashReference.java"
        source.write_text(SOURCE)
        def argfile(name, arguments):
            path = Path(directory) / name
            path.write_text("\n".join('"' + v.replace("\\", "\\\\").replace('"', '\\"') + '"' for v in arguments))
            return "@" + str(path)
        subprocess.run(["javac", argfile("compile.args", ["-proc:none", "-cp", classpath, str(source)])], check=True)
        result = subprocess.run(["java", argfile("run.args", ["-cp", directory + os.pathsep + classpath, "BoxBorderDashReference"])], check=True, capture_output=True, text=True)
    path = ROOT / "crates/ofdrw/tests/fixtures/box-border-dashes-java.tsv"
    path.write_text(f"# Java OFDRW revision {revision}; generated independently of Rust.\n"
                    "# case boundary line_width dash_pattern dash_offset abbreviated_data\n" + result.stdout)
    arguments = {"one": [3], "pairs": [3, 2], "offset": [1, 3, 2],
                 "long": [1, 3, 2, 4, 1], "unequal": [1, 3, 2],
                 "negative_offset": [-1, 3, 2]}
    cases = {}
    for row in result.stdout.splitlines():
        name, boundary, width, pattern, offset, commands = row.split("\t")
        case = cases.setdefault(name, {"name": name, "dash": arguments[name],
            "border": [1, 2, 3, 4] if name == "unequal" else [1] * 4, "paths": []})
        case["paths"].append({"boundary": [float(v) for v in boundary.split()],
            "line_width": float(width), "pattern": [float(v) for v in pattern.split()],
            "offset": float(offset), "commands": commands})
    json_path = path.with_suffix(".json")
    json_path.write_text(json.dumps({"java_revision": revision, "cases": list(cases.values())}, indent=2) + "\n")
    print(path.relative_to(ROOT))
    print(json_path.relative_to(ROOT))

if __name__ == "__main__":
    main()
