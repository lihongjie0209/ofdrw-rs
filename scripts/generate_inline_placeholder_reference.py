"""Generate inline placeholder line metrics with the pinned Java OFDRW build.

Reads Java line widths, maximum span heights and line heights without calling Rust.
The paragraph's extra 0.0001 container rounding allowance is not a line metric.
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
import org.ofdrw.layout.element.*;
public class InlinePlaceholderReference {
    static PlaceholderSpan hold(double width,double height) { return new PlaceholderSpan(width,height); }
    static void emit(String name,double width,double indent,PlaceholderSpan... spans) {
        Paragraph p = new Paragraph(); p.setWidth(width); p.setLineSpace(1d);
        if (indent > 0) p.setFirstLineIndentWidth(indent);
        for (PlaceholderSpan span: spans) p.add(span);
        p.doPrepare(width);
        for (TxtLineBlock line: p.getLines())
            System.out.printf(java.util.Locale.ROOT,"%s\t%.9f\t%.9f\t%.9f%n",name,line.getWidth(),line.getMaxSpanHeight(),line.getHeight());
    }
    public static void main(String[] args) {
        emit("mixed-heights",20,0,hold(3,2),hold(2,5),hold(4,3));
        emit("wrap",6,0,hold(3,2),hold(4,3),hold(2,2));
        emit("zero-tall",20,0,hold(0,7),hold(2,2));
        emit("replace-leading-indent",20,4,hold(3,2),hold(2,3));
        emit("character-count",20,0,new PlaceholderSpan(3,new Span("X").setFontSize(2d).setLetterSpacing(0.5d)),hold(1,4));
        emit("tight-wrap",4,0,hold(3,2),hold(2,2),hold(2,4));
        emit("zero-only",20,0,hold(0,2));
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
    with tempfile.TemporaryDirectory(prefix="ofdrw-inline-reference-") as directory:
        source = Path(directory) / "InlinePlaceholderReference.java"
        source.write_text(SOURCE)
        def argfile(name, arguments):
            path = Path(directory) / name
            path.write_text("\n".join('"' + v.replace("\\", "\\\\").replace('"', '\\"') + '"' for v in arguments))
            return "@" + str(path)
        subprocess.run(["javac", argfile("compile.args", ["-proc:none", "-cp", classpath, str(source)])], check=True)
        result = subprocess.run(["java", argfile("run.args", ["-cp", directory + os.pathsep + classpath, "InlinePlaceholderReference"])], check=True, capture_output=True, text=True)

    definitions = {
        "mixed-heights": (20,0,[(3,2),(2,5),(4,3)]),
        "wrap": (6,0,[(3,2),(4,3),(2,2)]),
        "zero-tall": (20,0,[(0,7),(2,2)]),
        "replace-leading-indent": (20,4,[(3,2),(2,3)]),
        "character-count": (20,0,[(7.5,2),(1,4)]),
        "tight-wrap": (4,0,[(3,2),(2,2),(2,4)]),
        "zero-only": (20,0,[(0,2)]),
    }
    cases = {}
    for row in result.stdout.splitlines():
        name, width, baseline, height = row.split("\t")
        available, indent, spans = definitions[name]
        case = cases.setdefault(name, {"name":name,"content_width":available,
            "style":{"font_size":2,"line_height":3,"first_line_indent":indent},
            "spans":[{"text":"","placeholder_width":w,"font_size":h} for w,h in spans],
            "lines":[]})
        y = sum(line["height"] for line in case["lines"])
        case["lines"].append({"text":"","width":float(width),"height":float(height),
            "baseline":y+float(baseline),"x":0,"y":y})
    path = ROOT / "crates/ofdrw/tests/fixtures/inline-placeholders-java.json"
    path.write_text(json.dumps({"java_revision":revision,"cases":list(cases.values())},indent=2)+"\n")
    print(path.relative_to(ROOT))

if __name__ == "__main__":
    main()
