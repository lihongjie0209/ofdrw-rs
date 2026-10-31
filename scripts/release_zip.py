"""ZIP entries with stable metadata for release source and native archives."""

from pathlib import Path
import shutil
import zipfile


def write_entry(archive: zipfile.ZipFile, name: str, source: Path | bytes) -> None:
    """Preserve file bytes without recording checkout times or host permissions."""
    info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
    info.create_system = 3
    info.external_attr = 0o100644 << 16
    info.compress_type = zipfile.ZIP_DEFLATED
    if isinstance(source, bytes):
        archive.writestr(info, source)
    else:
        info.file_size = source.stat().st_size
        with source.open("rb") as reader, archive.open(info, "w") as writer:
            shutil.copyfileobj(reader, writer)
