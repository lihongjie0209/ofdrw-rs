"""Regenerate the tiny CMYK JPEG fixture with `uv run --with pillow python`."""

from pathlib import Path

from PIL import Image


def main() -> None:
    image = Image.new("CMYK", (2, 1))
    image.putdata([(0, 255, 255, 0), (0, 0, 0, 255)])
    destination = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures/cmyk.jpg"
    image.save(destination, quality=95)


if __name__ == "__main__":
    main()
