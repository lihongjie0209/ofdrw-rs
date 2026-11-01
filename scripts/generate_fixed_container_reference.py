"""Generate absolute fixed-page box coordinates using the pinned Java OFDRW build.

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
import org.ofdrw.layout.*;
import org.ofdrw.layout.element.*;
import org.ofdrw.layout.engine.render.DivRender;
import org.ofdrw.core.basicStructure.pageObj.layer.block.*;
public class FixedContainerReference {
    static void emit(String name, Div div) {
        VirtualPage page = new VirtualPage(new PageLayout(20d,20d));
        page.add(div);
        Rectangle outer = div.box();
        System.out.printf(java.util.Locale.ROOT,"A\t%s\t%.9f %.9f %.9f %.9f%n",
            name,div.getX(),div.getY(),outer.getWidth(),outer.getHeight());
        CT_PageBlock layer = new CT_PageBlock();
        if (!div.isPlaceholder()) DivRender.render(layer,div,new AtomicInteger());
        for (var block : layer.getPageBlocks()) {
            PathObject p = (PathObject) block;
            if (Boolean.TRUE.equals(p.getFill()))
                System.out.printf("B\t%s\t%s%n",name,p.getBoundary());
        }
    }
    public static void main(String[] args) {
        emit("outside",new Div(12d,22d,40d,3d).setMargin(1d).setPadding(0.5d).setBorder(0.25d).setBackgroundColor(240,230,220));
        emit("negative",new Div(-4d,-3d,8d,2d).setBackgroundColor(240,230,220));
        emit("tall",new Div(3d,4d,5d,50d).setIntegrity(true).setBackgroundColor(240,230,220));
        emit("placeholder",new Div(5d,6d,4d,9d).setPlaceholder(true).setBackgroundColor(240,230,220));
        emit("fractional",new Div(1.25d,2.5d,6.5d,3.25d).setMargin(0.25d,0.5d,0.75d,1d).setBorder(0.25d,0.5d,0.75d,1d).setPadding(0.25d,0.5d,0.75d,1d).setBackgroundColor(240,230,220));
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
    with tempfile.TemporaryDirectory(prefix="ofdrw-fixed-reference-") as directory:
        source = Path(directory) / "FixedContainerReference.java"
        source.write_text(SOURCE)
        def argfile(name, arguments):
            path = Path(directory) / name
            path.write_text("\n".join('"' + v.replace("\\", "\\\\").replace('"', '\\"') + '"' for v in arguments))
            return "@" + str(path)
        subprocess.run(["javac", argfile("compile.args", ["-proc:none", "-cp", classpath, str(source)])], check=True)
        result = subprocess.run(["java", argfile("run.args", ["-cp", directory + os.pathsep + classpath, "FixedContainerReference"])], check=True, capture_output=True, text=True)
    path = ROOT / "crates/ofdrw/tests/fixtures/fixed-containers-java.tsv"
    path.write_text(f"# Java OFDRW revision {revision}; generated independently of Rust.\n"
                    "# A case allocation; B case background boundary\n" + result.stdout)
    inputs = {
        "outside": {"position": [12,22], "layout": {"size": [40,3], "margin": [1]*4, "padding": [0.5]*4, "border": [0.25]*4}},
        "negative": {"position": [-4,-3], "layout": {"size": [8,2]}},
        "tall": {"position": [3,4], "layout": {"size": [5,50], "integrity": True}},
        "placeholder": {"position": [5,6], "layout": {"size": [4,9], "placeholder": True}},
        "fractional": {"position": [1.25,2.5], "layout": {"size": [6.5,3.25],
            "margin": [0.25,0.5,0.75,1], "border": [0.25,0.5,0.75,1], "padding": [0.25,0.5,0.75,1]}},
    }
    cases = {}
    for row in result.stdout.splitlines():
        kind, name, values = row.split("\t")
        source = inputs[name]
        case = cases.setdefault(name, {"name": name, "input": {"position": source["position"],
            "container": {"layout": {**source["layout"], "background_color": [240,230,220]}, "children": []}},
            "backgrounds": []})
        values = [float(v) for v in values.split()]
        if kind == "A": case["allocation"] = values
        else: case["backgrounds"].append(values)
    json_path = path.with_suffix(".json")
    json_path.write_text(json.dumps({"java_revision": revision, "cases": list(cases.values())}, indent=2) + "\n")
    print(path.relative_to(ROOT))
    print(json_path.relative_to(ROOT))

if __name__ == "__main__":
    main()
