import java.awt.BasicStroke;
import java.awt.Color;
import java.awt.Graphics2D;
import java.awt.RenderingHints;
import java.awt.Shape;
import java.awt.geom.AffineTransform;
import java.awt.geom.Area;
import java.awt.geom.Ellipse2D;
import java.awt.geom.Path2D;
import java.awt.geom.Rectangle2D;
import java.awt.image.BufferedImage;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayDeque;
import javax.imageio.ImageIO;
import javax.xml.parsers.DocumentBuilderFactory;
import org.w3c.dom.Element;
import org.w3c.dom.Node;
import org.ofdrw.layout.OFDDoc;
import org.ofdrw.layout.VirtualPage;
import org.ofdrw.layout.element.Position;
import org.ofdrw.layout.element.canvas.Canvas;
import org.ofdrw.layout.element.canvas.DrawContext;
import org.ofdrw.reader.OFDReader;
import org.ofdrw.converter.ImageMaker;

/** Independent Java2D geometry oracle; also retain unmodified OFDRW output. */
public class CanvasAffineClipReference {
    static double[] values(Element command, String attribute) {
        String[] tokens = command.getAttribute(attribute).split(" ");
        double[] result = new double[tokens.length];
        for (int i = 0; i < tokens.length; i++) result[i] = Double.parseDouble(tokens[i]);
        return result;
    }
    static AffineTransform matrix(Element command) {
        return new AffineTransform(values(command,"matrix"));
    }
    static Area filledAndStroked(Shape path) {
        Area result = new Area(path);
        result.add(new Area(new BasicStroke(0.353f).createStrokedShape(path)));
        return result;
    }
    static class State {
        AffineTransform matrix = new AffineTransform();
        Path2D.Double path = new Path2D.Double();
        Area clip;
        State copy() {
            State copy = new State();
            copy.matrix = new AffineTransform(matrix);
            copy.path = (Path2D.Double)path.clone();
            copy.clip = clip == null ? null : new Area(clip);
            return copy;
        }
    }
    static void writeJava(DrawContext c, Element scene) {
        for (Node n = scene.getFirstChild(); n != null; n = n.getNextSibling()) {
            if (!(n instanceof Element)) continue;
            Element command = (Element)n;
            switch (command.getTagName()) {
                case "set_transform": {
                    double[] m = values(command,"matrix");
                    c.setTransform(m[0],m[1],m[2],m[3],m[4],m[5]); break;
                }
                case "begin_path": c.beginPath(); break;
                case "rect": {
                    double[] r = values(command,"rect"); c.rect(r[0],r[1],r[2],r[3]); break;
                }
                case "move_to": {
                    double[] p = values(command,"point"); c.moveTo(p[0],p[1]); break;
                }
                case "bezier_curve_to": {
                    double[] a = values(command,"control1"), b = values(command,"control2"), e = values(command,"end");
                    c.bezierCurveTo(a[0],a[1],b[0],b[1],e[0],e[1]); break;
                }
                case "quadratic_curve_to": {
                    double[] a = values(command,"control"), e = values(command,"end");
                    c.quadraticCurveTo(a[0],a[1],e[0],e[1]); break;
                }
                case "circle_arc": {
                    double[] p = values(command,"center");
                    c.arc(p[0],p[1],Double.parseDouble(command.getAttribute("radius")),0,360,true); break;
                }
                case "close_path": c.closePath(); break;
                case "clip": c.clip(); break;
                case "save": c.save(); break;
                case "restore": c.restore(); break;
                case "fill_rect": {
                    double[] r = values(command,"rect"), color = values(command,"color");
                    // Set parameters explicitly: pinned Java save/restore loses drawParam.
                    c.setLineWidth(0.353).setFillColor((int)color[0],(int)color[1],(int)color[2])
                        .setStrokeColor((int)color[0],(int)color[1],(int)color[2]);
                    c.fillRect(r[0],r[1],r[2],r[3]); break;
                }
                default: throw new IllegalArgumentException(command.getTagName());
            }
        }
    }
    static BufferedImage drawGeometry(Element scene, int ppm) {
        BufferedImage image = new BufferedImage(40*ppm,30*ppm,BufferedImage.TYPE_INT_RGB);
        Graphics2D g = image.createGraphics();
        g.setColor(Color.WHITE); g.fillRect(0,0,image.getWidth(),image.getHeight());
        g.setRenderingHint(RenderingHints.KEY_ANTIALIASING,RenderingHints.VALUE_ANTIALIAS_ON);
        g.setRenderingHint(RenderingHints.KEY_STROKE_CONTROL,RenderingHints.VALUE_STROKE_PURE);
        g.scale(ppm,ppm); g.translate(4,3);
        State state = new State();
        ArrayDeque<State> saved = new ArrayDeque<>();
        for (Node n = scene.getFirstChild(); n != null; n = n.getNextSibling()) {
            if (!(n instanceof Element)) continue;
            Element command = (Element)n;
            switch (command.getTagName()) {
                case "set_transform": state.matrix = matrix(command); break;
                case "begin_path": state.path = new Path2D.Double(); break;
                case "rect": {
                    double[] r = values(command,"rect");
                    state.path.append(new Rectangle2D.Double(r[0],r[1],r[2],r[3]),false); break;
                }
                case "move_to": {
                    double[] p = values(command,"point"); state.path.moveTo(p[0],p[1]); break;
                }
                case "bezier_curve_to": {
                    double[] a = values(command,"control1"), b = values(command,"control2"), e = values(command,"end");
                    state.path.curveTo(a[0],a[1],b[0],b[1],e[0],e[1]); break;
                }
                case "quadratic_curve_to": {
                    double[] a = values(command,"control"), e = values(command,"end");
                    state.path.quadTo(a[0],a[1],e[0],e[1]); break;
                }
                case "circle_arc": {
                    double[] p = values(command,"center"); double r = Double.parseDouble(command.getAttribute("radius"));
                    state.path.append(new Ellipse2D.Double(p[0]-r,p[1]-r,2*r,2*r),false); break;
                }
                case "close_path": state.path.closePath(); break;
                // OFDRW replaces the clip rather than intersecting it with the old clip.
                case "clip": state.clip = new Area(state.matrix.createTransformedShape(filledAndStroked(state.path))); break;
                case "save": saved.push(state.copy()); break;
                case "restore": state = saved.pop(); break;
                case "fill_rect": {
                    double[] r = values(command,"rect"), color = values(command,"color");
                    Area shape = new Area(state.matrix.createTransformedShape(
                        filledAndStroked(new Rectangle2D.Double(r[0],r[1],r[2],r[3]))));
                    if (state.clip != null) shape.intersect(state.clip);
                    shape.intersect(new Area(new Rectangle2D.Double(0,0,28,22)));
                    g.setColor(new Color((int)color[0],(int)color[1],(int)color[2]));
                    g.fill(shape); break;
                }
                default: throw new IllegalArgumentException(command.getTagName());
            }
        }
        g.dispose(); return image;
    }
    public static void main(String[] args) throws Exception {
        if (args[0].equals("--no-clips")) {
            Path root = Paths.get(args[1]);
            try (java.nio.file.DirectoryStream<Path> files = java.nio.file.Files.newDirectoryStream(root,"*-no-clips.ofd")) {
                for (Path ofd : files) {
                    try (OFDReader reader = new OFDReader(ofd)) {
                        String name = ofd.getFileName().toString().replace("-no-clips.ofd","-imagemaker-no-clips-4.png");
                        ImageIO.write(new ImageMaker(reader,4).makePage(0),"png",root.resolve(name).toFile());
                    }
                }
            }
            return;
        }
        Path root = Paths.get(args[1]);
        Element scenes = DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(args[0]).getDocumentElement();
        for (Node n = scenes.getFirstChild(); n != null; n = n.getNextSibling()) {
            if (!(n instanceof Element)) continue;
            Element scene = (Element)n;
            String name = scene.getAttribute("name");
            Path ofd = root.resolve(name+".ofd");
            try (OFDDoc doc = new OFDDoc(ofd)) {
                VirtualPage page = new VirtualPage(40d,30d);
                Canvas canvas = new Canvas(28d,22d);
                canvas.setPosition(Position.Absolute).setX(4d).setY(3d).setBorder(0d);
                canvas.setDrawer(c -> writeJava(c,scene));
                page.add(canvas); doc.addVPage(page);
            }
            for (int ppm : new int[]{4,12}) {
                ImageIO.write(drawGeometry(scene,ppm),"png",root.resolve(name+"-geometry-"+ppm+".png").toFile());
            }
            try (OFDReader reader = new OFDReader(ofd)) {
                ImageIO.write(new ImageMaker(reader,4).makePage(0),"png",root.resolve(name+"-imagemaker-4.png").toFile());
            }
        }
    }
}
