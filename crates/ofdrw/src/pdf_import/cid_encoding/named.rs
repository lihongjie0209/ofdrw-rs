//! Compressed, commit-pinned Adobe resources, with original license headers.

use std::io::{Cursor, Read};

use super::{invalid, OfdError, MAX_BYTES};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/cmaps/names.rs"
));

pub(super) fn data(name: &[u8]) -> Result<(&'static str, &'static str, Vec<u8>), OfdError> {
    let index = NAMES
        .binary_search_by(|(candidate, _)| candidate.as_bytes().cmp(name))
        .map_err(|_| invalid("unsupported named CMap"))?;
    let (name, ordering) = NAMES[index];
    let mut archive = zip::ZipArchive::new(Cursor::new(
        include_bytes!("../../../assets/cmaps/cmaps.zip").as_slice(),
    ))
    .map_err(|_| invalid("invalid built-in CMap archive"))?;
    let mut data = Vec::new();
    archive
        .by_name(name)
        .map_err(|_| invalid("missing built-in CMap"))?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut data)?;
    if data.len() > MAX_BYTES {
        return Err(super::limit());
    }
    Ok((name, ordering, data))
}
