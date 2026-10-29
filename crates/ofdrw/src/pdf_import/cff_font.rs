//! CFF charset selectors and lossless sfnt encapsulation for PDF fonts.

use super::{HashMap, OfdError};

fn invalid() -> OfdError {
    OfdError::Invalid("PDF embedded CFF font is invalid or unsupported".into())
}

struct Reader<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], OfdError> {
        let end = self.position.checked_add(count).ok_or_else(invalid)?;
        let bytes = self.data.get(self.position..end).ok_or_else(invalid)?;
        self.position = end;
        Ok(bytes)
    }

    fn byte(&mut self) -> Result<u8, OfdError> {
        Ok(self.take(1)?[0])
    }

    fn integer(&mut self, width: usize) -> Result<u32, OfdError> {
        Ok(self
            .take(width)?
            .iter()
            .fold(0, |value, byte| (value << 8) | u32::from(*byte)))
    }
}

fn index<'a>(reader: &mut Reader<'a>, limit: usize) -> Result<Vec<&'a [u8]>, OfdError> {
    let count = reader.integer(2)? as usize;
    if count > limit {
        return Err(invalid());
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    let width = usize::from(reader.byte()?);
    if !(1..=4).contains(&width) {
        return Err(invalid());
    }
    let offsets = (0..=count)
        .map(|_| reader.integer(width))
        .collect::<Result<Vec<_>, _>>()?;
    if offsets[0] != 1 || offsets.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(invalid());
    }
    let bytes = reader.take(offsets[count] as usize - 1)?;
    Ok(offsets
        .windows(2)
        .map(|pair| &bytes[pair[0] as usize - 1..pair[1] as usize - 1])
        .collect())
}

fn real(reader: &mut Reader<'_>) -> Result<f64, OfdError> {
    let mut text = String::new();
    loop {
        let byte = reader.byte()?;
        for nibble in [byte >> 4, byte & 15] {
            match nibble {
                0..=9 => text.push(char::from(b'0' + nibble)),
                10 => text.push('.'),
                11 => text.push('E'),
                12 => text.push_str("E-"),
                14 => text.push('-'),
                15 => {
                    return text
                        .parse::<f64>()
                        .ok()
                        .filter(|n| n.is_finite())
                        .ok_or_else(invalid)
                }
                _ => return Err(invalid()),
            }
            if text.len() > 64 {
                return Err(invalid());
            }
        }
    }
}

fn dictionary(data: &[u8]) -> Result<HashMap<u16, Vec<f64>>, OfdError> {
    let mut reader = Reader { data, position: 0 };
    let mut operands = Vec::new();
    let mut result = HashMap::new();
    let mut operations = 0;
    while reader.position < data.len() {
        let byte = reader.byte()?;
        let value = match byte {
            28 => f64::from(reader.integer(2)? as i16),
            29 => f64::from(reader.integer(4)? as i32),
            30 => real(&mut reader)?,
            32..=246 => f64::from(i32::from(byte) - 139),
            247..=250 => f64::from((u32::from(byte) - 247) * 256 + u32::from(reader.byte()?) + 108),
            251..=254 => {
                -f64::from((u32::from(byte) - 251) * 256 + u32::from(reader.byte()?) + 108)
            }
            0..=21 => {
                let operator = if byte == 12 {
                    1200 + u16::from(reader.byte()?)
                } else {
                    u16::from(byte)
                };
                operations += 1;
                if operations > 4096 {
                    return Err(invalid());
                }
                result.insert(operator, std::mem::take(&mut operands));
                continue;
            }
            _ => return Err(invalid()),
        };
        operands.push(value);
        if operands.len() > 48 {
            return Err(invalid());
        }
    }
    if !operands.is_empty() {
        return Err(invalid());
    }
    Ok(result)
}

fn top_dictionary(data: &[u8]) -> Result<HashMap<u16, Vec<f64>>, OfdError> {
    if data.first() != Some(&1) {
        return Err(invalid());
    }
    let header = usize::from(*data.get(2).ok_or_else(invalid)?);
    if header < 4 {
        return Err(invalid());
    }
    let mut reader = Reader {
        data,
        position: header,
    };
    if index(&mut reader, 1)?.len() != 1 {
        return Err(invalid());
    }
    let dictionaries = index(&mut reader, 1)?;
    dictionary(dictionaries.first().ok_or_else(invalid)?)
}

fn offset(values: &[f64]) -> Result<usize, OfdError> {
    match values {
        [value] if *value >= 0.0 && *value <= u32::MAX as f64 && value.fract() == 0.0 => {
            Ok(*value as usize)
        }
        _ => Err(invalid()),
    }
}

pub(crate) struct GlyphMatrices {
    transforms: Vec<[f64; 6]>,
    selection: Vec<u8>,
    glyph_count: u16,
}

fn matrix(values: &[f64]) -> Result<[f64; 6], OfdError> {
    let matrix: [f64; 6] = values.try_into().map_err(|_| invalid())?;
    if matrix
        .iter()
        .any(|value| !value.is_finite() || value.abs() > 1e6)
    {
        return Err(invalid());
    }
    Ok(matrix)
}

fn compose(parent: [f64; 6], child: [f64; 6]) -> Result<[f64; 6], OfdError> {
    let [a, b, c, d, e, f] = parent;
    let [p, q, r, s, t, u] = child;
    matrix(&[
        a * p + c * q,
        b * p + d * q,
        a * r + c * s,
        b * r + d * s,
        a * t + c * u + e,
        b * t + d * u + f,
    ])
}

fn fd_selection(data: &[u8], at: usize, glyphs: u16, fonts: usize) -> Result<Vec<u8>, OfdError> {
    let mut reader = Reader { data, position: at };
    let mut selection = Vec::with_capacity(usize::from(glyphs));
    match reader.byte()? {
        0 => selection.extend_from_slice(reader.take(usize::from(glyphs))?),
        3 => {
            let ranges = reader.integer(2)? as usize;
            if ranges == 0 || ranges > usize::from(glyphs) {
                return Err(invalid());
            }
            let mut first = reader.integer(2)? as usize;
            if first != 0 {
                return Err(invalid());
            }
            for _ in 0..ranges {
                let font = reader.byte()?;
                let next = reader.integer(2)? as usize;
                if next <= first || next > usize::from(glyphs) {
                    return Err(invalid());
                }
                selection.resize(next, font);
                first = next;
            }
            if first != usize::from(glyphs) {
                return Err(invalid());
            }
        }
        _ => return Err(invalid()),
    }
    if selection.iter().any(|font| usize::from(*font) >= fonts) {
        return Err(invalid());
    }
    Ok(selection)
}

impl GlyphMatrices {
    pub(crate) fn parse(data: &[u8]) -> Result<Self, OfdError> {
        let table = ttf_parser::cff::Table::parse(data).ok_or_else(invalid)?;
        let top = top_dictionary(data)?;
        if top
            .get(&1205)
            .is_some_and(|values| values.as_slice() != [0.0])
            || top
                .get(&1206)
                .is_some_and(|values| values.as_slice() != [2.0])
            || top.contains_key(&1220)
        {
            return Err(invalid());
        }
        // PostScript strings (including FSType metadata) are retained, never executed.
        let declared = top.get(&1207).map(|values| matrix(values)).transpose()?;
        let top_matrix = declared.unwrap_or([0.001, 0.0, 0.0, 0.001, 0.0, 0.0]);
        let glyph_count = table.number_of_glyphs();
        let (transforms, selection) = if let Some(values) = top.get(&1236) {
            let mut reader = Reader {
                data,
                position: offset(values)?,
            };
            let fonts = index(&mut reader, 256)?;
            if fonts.is_empty() {
                return Err(invalid());
            }
            let mut transforms = Vec::with_capacity(fonts.len());
            for font in fonts {
                let dictionary = dictionary(font)?;
                let transform = match dictionary.get(&1207) {
                    Some(values) => {
                        let local = matrix(values)?;
                        // An explicit top-level matrix concatenates the FD matrix.
                        // Without it, the FD matrix replaces the implicit default.
                        declared.map_or(Ok(local), |parent| compose(parent, local))?
                    }
                    None => top_matrix,
                };
                transforms.push(transform);
            }
            let select = offset(top.get(&1237).ok_or_else(invalid)?)?;
            let selection = fd_selection(data, select, glyph_count, transforms.len())?;
            (transforms, selection)
        } else {
            if top.contains_key(&1237) {
                return Err(invalid());
            }
            (vec![top_matrix], Vec::new())
        };
        Ok(Self {
            transforms,
            selection,
            glyph_count,
        })
    }

    pub(crate) fn for_glyph(&self, glyph: u16) -> Result<[f64; 6], OfdError> {
        if glyph >= self.glyph_count {
            return Err(invalid());
        }
        let index = self.selection.get(usize::from(glyph)).copied().unwrap_or(0);
        self.transforms
            .get(usize::from(index))
            .copied()
            .ok_or_else(invalid)
    }
}

pub(crate) fn transformed_bounds(bounds: ttf_parser::Rect, matrix: [f64; 6]) -> [f64; 4] {
    let [a, b, c, d, e, f] = matrix;
    let mut result = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for (x, y) in [
        (bounds.x_min, bounds.y_min),
        (bounds.x_min, bounds.y_max),
        (bounds.x_max, bounds.y_min),
        (bounds.x_max, bounds.y_max),
    ] {
        let x = f64::from(x);
        let y = f64::from(y);
        let px = a * x + c * y + e;
        let py = b * x + d * y + f;
        result[0] = result[0].min(px);
        result[1] = result[1].min(py);
        result[2] = result[2].max(px);
        result[3] = result[3].max(py);
    }
    result
}

pub(super) fn glyphs(data: &[u8], cids: &[u16]) -> Result<Vec<u16>, OfdError> {
    let table = ttf_parser::cff::Table::parse(data).ok_or_else(invalid)?;
    if table.glyph_cid(ttf_parser::GlyphId(0)).is_none() {
        return Ok(cids.to_vec());
    }
    let top = top_dictionary(data)?;
    let charset = top
        .get(&15)
        .map(|values| offset(values))
        .transpose()?
        .unwrap_or(0);
    let mut inverse = HashMap::new();
    inverse.insert(0_u16, 0_u16);
    let count = table.number_of_glyphs();
    if charset <= 2 {
        if count > 256 {
            return Err(invalid());
        }
        for gid in 1..count {
            let cid = table
                .glyph_cid(ttf_parser::GlyphId(gid))
                .ok_or_else(invalid)?;
            if inverse.insert(cid, gid).is_some() {
                return Err(invalid());
            }
        }
    } else {
        let mut reader = Reader {
            data,
            position: charset,
        };
        let format = reader.byte()?;
        if format > 2 {
            return Err(invalid());
        }
        let mut gid = 1_u32;
        while gid < u32::from(count) {
            let first = reader.integer(2)?;
            let left = match format {
                0 => 0,
                1 => reader.integer(1)?,
                _ => reader.integer(2)?,
            };
            if first + left > 65535 || gid + left >= u32::from(count) {
                return Err(invalid());
            }
            for cid in first..=first + left {
                if inverse.insert(cid as u16, gid as u16).is_some() {
                    return Err(invalid());
                }
                gid += 1;
            }
        }
    }
    Ok(cids
        .iter()
        .map(|cid| inverse.get(cid).copied().unwrap_or(0))
        .collect())
}

pub(super) fn validate_glyphs(
    table: ttf_parser::cff::Table<'_>,
    glyphs: &[u16],
) -> Result<(), OfdError> {
    struct Outline;
    impl ttf_parser::OutlineBuilder for Outline {
        fn move_to(&mut self, _: f32, _: f32) {}
        fn line_to(&mut self, _: f32, _: f32) {}
        fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {}
        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {}
        fn close(&mut self) {}
    }
    let mut seen = vec![false; usize::from(table.number_of_glyphs())];
    for &glyph in glyphs {
        let seen = seen.get_mut(usize::from(glyph)).ok_or_else(invalid)?;
        if std::mem::replace(seen, true) {
            continue;
        }
        match table.outline(ttf_parser::GlyphId(glyph), &mut Outline) {
            Ok(_) | Err(ttf_parser::CFFError::ZeroBBox) => {}
            Err(_) => {
                return Err(OfdError::Invalid(
                    "PDF CFF glyph program is invalid or unsupported".into(),
                ))
            }
        }
    }
    Ok(())
}

fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0_u32, |sum, chunk| {
        let mut bytes = [0; 4];
        bytes[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(bytes))
    })
}

/// Preserve every CFF byte inside a deterministic, outline-only sfnt wrapper.
pub(crate) fn wrap(data: &[u8]) -> Result<Vec<u8>, OfdError> {
    let cff = ttf_parser::cff::Table::parse(data).ok_or_else(invalid)?;
    let matrix = cff.matrix();
    let reciprocal = 1.0 / f64::from(matrix.sx);
    GlyphMatrices::parse(data)?;
    let units = if reciprocal.is_finite() && (16.0..=16384.0).contains(&reciprocal.round()) {
        reciprocal.round() as u16
    } else {
        1000
    };
    let count = cff.number_of_glyphs();
    let mut head = vec![0; 54];
    head[..4].copy_from_slice(&0x00010000_u32.to_be_bytes());
    head[4..8].copy_from_slice(&0x00010000_u32.to_be_bytes());
    head[12..16].copy_from_slice(&0x5f0f3cf5_u32.to_be_bytes());
    head[18..20].copy_from_slice(&units.to_be_bytes());
    head[40..42].copy_from_slice(&units.to_be_bytes());
    head[42..44].copy_from_slice(&units.to_be_bytes());
    head[46..48].copy_from_slice(&8_u16.to_be_bytes());
    head[48..50].copy_from_slice(&2_i16.to_be_bytes());
    let mut hhea = vec![0; 36];
    hhea[..4].copy_from_slice(&0x00010000_u32.to_be_bytes());
    hhea[4..6].copy_from_slice(&units.to_be_bytes());
    hhea[6..8].copy_from_slice(&((-i32::from(units) / 5) as i16).to_be_bytes());
    hhea[10..12].copy_from_slice(&units.to_be_bytes());
    hhea[18..20].copy_from_slice(&1_u16.to_be_bytes());
    hhea[34..36].copy_from_slice(&count.to_be_bytes());
    let mut hmtx = Vec::with_capacity(usize::from(count) * 4);
    for _ in 0..count {
        hmtx.extend(units.to_be_bytes());
        hmtx.extend([0, 0]);
    }
    let mut maxp = 0x00005000_u32.to_be_bytes().to_vec();
    maxp.extend(count.to_be_bytes());
    let tables = [
        (b"CFF ", data.to_vec()),
        (b"head", head),
        (b"hhea", hhea),
        (b"hmtx", hmtx),
        (b"maxp", maxp),
    ];
    let mut output = b"OTTO\0\x05\0\x40\0\x02\0\x10".to_vec();
    output.resize(12 + tables.len() * 16, 0);
    let mut head_offset = 0;
    for (index, (tag, table)) in tables.into_iter().enumerate() {
        let offset = output.len();
        let directory = &mut output[12 + index * 16..28 + index * 16];
        directory[..4].copy_from_slice(tag);
        directory[4..8].copy_from_slice(&checksum(&table).to_be_bytes());
        directory[8..12].copy_from_slice(&(offset as u32).to_be_bytes());
        directory[12..16].copy_from_slice(&(table.len() as u32).to_be_bytes());
        output.extend(table);
        output.resize(output.len().next_multiple_of(4), 0);
        if tag == b"head" {
            head_offset = offset;
        }
    }
    let adjustment = 0xb1b0afba_u32.wrapping_sub(checksum(&output));
    output[head_offset + 8..head_offset + 12].copy_from_slice(&adjustment.to_be_bytes());
    ttf_parser::Face::parse(&output, 0).map_err(|_| invalid())?;
    Ok(output)
}

pub(super) fn builtin_encoding(
    pdf: &super::PdfDocument,
    font: &lopdf::Dictionary,
) -> Result<Option<super::simple_encoding::SimpleEncoding>, OfdError> {
    use super::{pdf_byte_stream_data, pdf_resolved_value, Object};
    if font.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Type1".as_slice()) {
        return Ok(None);
    }
    let Ok(descriptor) = font.get(b"FontDescriptor") else {
        return Ok(None);
    };
    let descriptor = pdf_resolved_value(Some(pdf), descriptor, "FontDescriptor")?
        .as_dict()
        .map_err(|_| invalid())?;
    let Ok(program) = descriptor.get(b"FontFile3") else {
        return Ok(None);
    };
    let stream = pdf_resolved_value(Some(pdf), program, "FontFile3")?
        .as_stream()
        .map_err(|_| invalid())?;
    let subtype = stream.dict.get(b"Subtype").and_then(Object::as_name).ok();
    if !matches!(subtype, Some(b"Type1C" | b"OpenType")) {
        return Ok(None);
    }
    let bytes = pdf_byte_stream_data(stream, Some(pdf), "embedded CFF encoding")?;
    let face;
    let table = if subtype == Some(b"OpenType".as_slice()) {
        face = ttf_parser::Face::parse(&bytes, 0).map_err(|_| invalid())?;
        face.tables().cff.ok_or_else(invalid)?
    } else {
        ttf_parser::cff::Table::parse(&bytes).ok_or_else(invalid)?
    };
    if table.glyph_cid(ttf_parser::GlyphId(0)).is_some() {
        return Err(invalid());
    }
    let dingbats = font
        .get(b"BaseFont")
        .ok()
        .map(|value| pdf_resolved_value(Some(pdf), value, "BaseFont"))
        .transpose()?
        .and_then(|value| value.as_name().ok())
        .and_then(super::standard_fonts::StandardFont::from_name)
        .is_some_and(super::standard_fonts::StandardFont::is_dingbats);
    Ok(Some(super::simple_encoding::SimpleEncoding::from_cff(
        table, dingbats,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;
    const CID: &[u8] = include_bytes!("../../tests/fixtures/test-cid.cff");
    const SID: &[u8] = include_bytes!("../../tests/fixtures/test-name.cff");

    #[test]
    fn cid_charset_selects_original_glyphs_and_missing_cids_use_notdef() {
        assert_eq!(glyphs(CID, &[42, 7, 43, 0]).unwrap(), [2, 1, 0, 0]);
        assert_eq!(glyphs(SID, &[2, 1, 0]).unwrap(), [2, 1, 0]);
    }

    #[test]
    fn charset_formats_zero_one_and_two_have_the_same_selector_semantics() {
        let at = offset(top_dictionary(CID).unwrap().get(&15).unwrap()).unwrap();
        for bytes in [[0, 0, 7, 0, 8], [1, 0, 7, 1, 0], [2, 0, 7, 0, 1]] {
            let mut source = CID.to_vec();
            source[at..at + 5].copy_from_slice(&bytes);
            assert_eq!(glyphs(&source, &[8, 7, 9, 0]).unwrap(), [2, 1, 0, 0]);
        }
        for bytes in [[0, 0, 7, 0, 7], [1, 255, 255, 1, 0], [2, 0, 7, 0, 3]] {
            let mut source = CID.to_vec();
            source[at..at + 5].copy_from_slice(&bytes);
            assert!(glyphs(&source, &[7]).is_err());
        }
    }

    #[test]
    fn raw_wrapper_preserves_program_and_has_consistent_sfnt_checksums() {
        for source in [CID, SID] {
            let wrapped = wrap(source).unwrap();
            assert_eq!(checksum(&wrapped), 0xb1b0afba);
            let face = ttf_parser::Face::parse(&wrapped, 0).unwrap();
            assert_eq!(face.units_per_em(), 1000);
            assert_eq!(face.number_of_glyphs(), 3);
            assert_eq!(
                face.raw_face()
                    .table(ttf_parser::Tag::from_bytes(b"CFF "))
                    .unwrap(),
                source
            );
            assert_eq!(
                GlyphMatrices::parse(source).unwrap().for_glyph(2).unwrap(),
                [0.001, 0.0, 0.0, 0.001, 0.0, 0.0]
            );
        }
    }

    #[test]
    fn font_matrices_accept_uniform_units_and_affine_transforms() {
        let source = include_bytes!("../../tests/fixtures/test-cff-2048.cff");
        let wrapped = wrap(source).unwrap();
        assert_eq!(
            ttf_parser::Face::parse(&wrapped, 0).unwrap().units_per_em(),
            2048
        );
        assert!(wrap(include_bytes!(
            "../../tests/fixtures/test-cff-fd-identity.cff"
        ))
        .is_ok());
        assert!(wrap(include_bytes!("../../tests/fixtures/test-cff-skew.cff")).is_ok());
        assert!(wrap(include_bytes!(
            "../../tests/fixtures/test-cff-fd-matrix.cff"
        ))
        .is_ok());
    }

    #[test]
    fn fd_select_formats_and_affine_bounds_validate_all_ranges() {
        assert_eq!(fd_selection(&[0, 0, 0, 1], 0, 3, 2).unwrap(), [0, 0, 1]);
        assert_eq!(
            fd_selection(&[3, 0, 2, 0, 0, 0, 0, 2, 1, 0, 3], 0, 3, 2).unwrap(),
            [0, 0, 1]
        );
        for data in [
            vec![0, 0],
            vec![0, 0, 0, 2],
            vec![4],
            vec![3, 0, 0],
            vec![3, 0, 1, 0, 1, 0, 0, 3],
            vec![3, 0, 1, 0, 0, 0, 0, 2],
            vec![3, 0, 1, 0, 0, 0, 0, 4],
        ] {
            assert!(fd_selection(&data, 0, 3, 2).is_err());
        }
        assert_eq!(
            compose([2., 3., 4., 5., 6., 7.], [1., 2., 3., 4., 5., 6.]).unwrap(),
            [10., 13., 22., 29., 40., 52.]
        );
        let bounds = ttf_parser::Rect {
            x_min: -2,
            y_min: 1,
            x_max: 5,
            y_max: 3,
        };
        assert_eq!(
            transformed_bounds(bounds, [0., 2., -3., 0., 10., 20.]),
            [1., 16., 7., 30.]
        );
        assert!(matrix(&[0.; 5]).is_err());
        assert!(matrix(&[f64::NAN; 6]).is_err());
        assert!(matrix(&[1e7; 6]).is_err());
    }

    #[test]
    fn fd_matrices_compose_explicit_top_and_replace_implicit_default() {
        for (source, expected) in [
            (
                include_bytes!("../../tests/fixtures/type0-fd-matrices-cff.pdf").as_slice(),
                [0.0008, 0.00018, 0.0003, 0.00123, 0.05, -0.015],
            ),
            (
                include_bytes!("../../tests/fixtures/type0-fd-only-cff.pdf").as_slice(),
                [0.0008, 0.0001, 0.0003, 0.0012, 0.05, -0.02],
            ),
        ] {
            let pdf = super::super::PdfDocument::load_mem(source).unwrap();
            let bytes = pdf
                .get_object((9, 0))
                .unwrap()
                .as_stream()
                .unwrap()
                .get_plain_content()
                .unwrap();
            let parsed = GlyphMatrices::parse(&bytes).unwrap();
            let actual = parsed.for_glyph(2).unwrap();
            for (actual, expected) in actual.into_iter().zip(expected) {
                assert!((actual - expected).abs() < 1e-12);
            }
            assert!(parsed.for_glyph(3).is_err());
        }
    }

    #[test]
    fn malformed_selected_charstrings_fail_instead_of_disappearing() {
        let mut data = SID.to_vec();
        assert_eq!(data.last(), Some(&14));
        let last = data.len() - 1;
        data[last] = 0;
        let table = ttf_parser::cff::Table::parse(&data).unwrap();
        assert!(validate_glyphs(table, &[1]).is_ok());
        assert!(validate_glyphs(table, &[2]).is_err());
    }

    #[test]
    fn dictionaries_and_indices_bound_numbers_offsets_and_operand_stacks() {
        assert_eq!(
            dictionary(&[30, 0x0a, 0x00, 0x1f, 12, 7]).unwrap()[&1207],
            [0.001]
        );
        assert_eq!(dictionary(&[28, 255, 254, 15]).unwrap()[&15], [-2.0]);
        for data in [
            vec![28],
            vec![29, 0, 0],
            vec![30, 0xdf],
            vec![139; 49],
            vec![139],
            vec![12],
        ] {
            assert!(dictionary(&data).is_err());
        }
        for data in [
            &[][..],
            &[0, 1, 0],
            &[0, 1, 1, 0, 1],
            &[0, 1, 1, 1, 9],
            &[0, 2, 1, 1, 3, 2, 0, 0],
        ] {
            assert!(index(&mut Reader { data, position: 0 }, 1).is_err());
        }
        assert!(offset(&[1.5]).is_err());
        assert!(offset(&[-1.0]).is_err());
        assert!(offset(&[f64::NAN]).is_err());
        let operators = [139, 0].repeat(4097);
        assert!(dictionary(&operators).is_err());
        for end in 0..CID.len() {
            let _ = glyphs(&CID[..end], &[7]);
            let _ = wrap(&CID[..end]);
        }
    }
}
