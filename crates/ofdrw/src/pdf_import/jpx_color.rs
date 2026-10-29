//! Decoder-only color metadata adapters for explicit DeviceN JPEG2000 images.

use super::{OfdError, PdfColorSpace, MAX_ENTRY_BYTES};
use std::borrow::Cow;

fn invalid() -> OfdError {
    OfdError::Invalid("invalid PDF JPEG2000 container boxes".into())
}

fn take_box<'a>(data: &mut &'a [u8], count: &mut usize) -> Result<([u8; 4], &'a [u8]), OfdError> {
    *count += 1;
    if *count > 4096 {
        return Err(OfdError::Limit(
            "PDF JPEG2000 has too many container boxes".into(),
        ));
    }
    let head = data.get(..8).ok_or_else(invalid)?;
    let length = u32::from_be_bytes(head[..4].try_into().map_err(|_| invalid())?);
    let (length, prefix) = match length {
        0 => (data.len(), 8),
        1 => (
            usize::try_from(u64::from_be_bytes(
                data.get(8..16)
                    .ok_or_else(invalid)?
                    .try_into()
                    .map_err(|_| invalid())?,
            ))
            .map_err(|_| invalid())?,
            16,
        ),
        length => (length as usize, 8),
    };
    let body = data.get(prefix..length).ok_or_else(invalid)?;
    let kind = head[4..8].try_into().map_err(|_| invalid())?;
    *data = data.get(length..).ok_or_else(invalid)?;
    Ok((kind, body))
}

fn push_box(data: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    // Input is capped at 64 MiB, so every normalized box length fits u32.
    data.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
    data.extend_from_slice(kind);
    data.extend_from_slice(body);
}

pub(super) fn codec_data<'a>(
    data: &'a [u8],
    space: Option<&PdfColorSpace>,
) -> Result<Cow<'a, [u8]>, OfdError> {
    let Some(PdfColorSpace::DeviceN { channels, .. }) = space else {
        return Ok(Cow::Borrowed(data));
    };
    if data.len() as u64 > MAX_ENTRY_BYTES {
        return Err(OfdError::Limit("PDF JPEG2000 stream is too large".into()));
    }
    if data.starts_with(b"\xff\x4f") {
        let count = data
            .get(40..42)
            .filter(|_| data.starts_with(b"\xff\x4f\xff\x51"))
            .map(|bytes| usize::from(u16::from_be_bytes([bytes[0], bytes[1]])));
        // Preserve the existing display decoder's handling of an additional
        // raw opacity component. Otherwise every declared component is color.
        if count != Some(*channels) {
            return Ok(Cow::Borrowed(data));
        }
        let mut wrapped = Vec::with_capacity(data.len() + 48);
        wrapped.extend_from_slice(b"\x00\x00\x00\x0cjP  \x0d\x0a\x87\x0a");
        wrapped.extend_from_slice(b"\x00\x00\x00\x14ftypjp2 \x00\x00\x00\x00jp2 ");
        push_box(&mut wrapped, b"jp2h", &[]);
        push_box(&mut wrapped, b"jp2c", data);
        return Ok(Cow::Owned(wrapped));
    }
    let mut remaining = data;
    let mut output = Vec::with_capacity(data.len());
    let mut count = 0;
    let mut changed = false;
    while !remaining.is_empty() {
        let (kind, body) = take_box(&mut remaining, &mut count)?;
        if kind != *b"jp2h" {
            push_box(&mut output, &kind, body);
            continue;
        }
        let mut children = body;
        let mut header = Vec::with_capacity(body.len());
        while !children.is_empty() {
            let (kind, body) = take_box(&mut children, &mut count)?;
            if kind == *b"cdef" {
                let entries = body.get(..2).ok_or_else(invalid)?;
                let entries = usize::from(u16::from_be_bytes([entries[0], entries[1]]));
                if entries == 0 {
                    return Err(invalid());
                }
                let entries = body.get(2..2 + entries * 6).ok_or_else(invalid)?;
                if entries.chunks_exact(6).any(|entry| entry[2..4] != [0, 0]) {
                    // Keep native opacity/profile interpretation for alpha
                    // containers; stripping color metadata must not lose alpha.
                    return Ok(Cow::Borrowed(data));
                }
            }
            if kind == *b"colr" {
                changed = true;
            } else {
                push_box(&mut header, &kind, body);
            }
        }
        push_box(&mut output, b"jp2h", &header);
    }
    Ok(if changed {
        Cow::Owned(output)
    } else {
        Cow::Borrowed(data)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn space() -> PdfColorSpace {
        PdfColorSpace::DeviceN {
            channels: 5,
            tint: None,
        }
    }

    #[test]
    fn malformed_and_excessive_container_boxes_return_errors() {
        for bytes in [
            vec![0; 7],
            vec![0, 0, 0, 1, b'j', b'p', b'2', b'h'],
            vec![0, 0, 0, 7, b'j', b'p', b'2', b'h'],
        ] {
            assert!(codec_data(&bytes, Some(&space())).is_err());
        }
        let mut many = Vec::new();
        for _ in 0..4097 {
            push_box(&mut many, b"free", &[]);
        }
        assert!(matches!(
            codec_data(&many, Some(&space())),
            Err(OfdError::Limit(_))
        ));
        let mut extended = Vec::new();
        extended.extend_from_slice(b"\0\0\0\x01free");
        extended.extend_from_slice(&16_u64.to_be_bytes());
        assert!(codec_data(&extended, Some(&space())).is_ok());
        assert!(matches!(
            codec_data(&[0; 3], None).unwrap(),
            Cow::Borrowed(_)
        ));
    }
}
