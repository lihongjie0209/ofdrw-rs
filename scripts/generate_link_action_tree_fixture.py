"""Author a mixed URI/GoTo action tree with a shared action and click region."""
from generate_link_fixtures import write_pdf

write_pdf("link-action-tree.pdf", [
    b"<< /Type /Catalog /Pages 2 0 R /URI << /Base (https://example.org/docs/) >> >>",
    b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 144 144] /Contents 5 0 R /Annots [6 0 R] >>",
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 144 144] /Contents 5 0 R >>",
    b"<< /Length 0 >>\nstream\n\nendstream",
    b"<< /Type /Annot /Subtype /Link /Rect [10 20 80 40] /F 4 /Border [0 0 0] /QuadPoints [12 22 70 22 78 38 18 38] /A 7 0 R >>",
    b"<< /S /URI /URI (start) /Next [8 0 R 10 0 R] >>",
    b"<< /S /GoTo /D [4 0 R /Fit] /Next [9 0 R 11 0 R] >>",
    b"<< /S /URI /URI (shared) >>",
    b"<< /S /URI /URI (tail) /Next 9 0 R >>",
    b"<< /S /URI /URI (../last) >>",
])
