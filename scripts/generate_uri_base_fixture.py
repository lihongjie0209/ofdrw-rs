"""Author URI links with an indirect catalog base; no network access is needed."""
from generate_link_fixtures import write_pdf


def text(value, encoding="utf-16-be"):
    bom = b"\xfe\xff" if encoding == "utf-16-be" else b"\xff\xfe"
    return b"<" + (bom + value.encode(encoding)).hex().encode() + b">"


targets = [text("../资料/页.html?字=值#段"), b"(?view=full)", b"(#section)",
           b"(//other.example/a)", b"(mailto:user@example.org)", b"()"]
objects = [
    b"<< /Type /Catalog /Pages 2 0 R /URI 11 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 144 144] /Contents 4 0 R /Annots [5 0 R 6 0 R 7 0 R 8 0 R 9 0 R 10 0 R] >>",
    b"<< /Length 0 >>\nstream\n\nendstream",
]
for index, target in enumerate(targets):
    bottom = 10 + index * 20
    objects.append(f"<< /Type /Annot /Subtype /Link /Rect [10 {bottom} 100 {bottom + 10}] /F 4 /Border [0 0 0] /A << /S /URI /URI ".encode() + target + b" >> >>")
objects.extend([b"<< /Base 12 0 R >>", text("https://example.org/docs/manual.pdf?lang=en", "utf-16-le")])
write_pdf("uri-base-links.pdf", objects)
