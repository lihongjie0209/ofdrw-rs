//! PDF CMaps: source byte codes and CID selectors stay distinct.

mod named;

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
};

use lopdf::{content::Content, Object};

use super::{pdf_byte_stream_data, pdf_resolved_value, OfdError, PdfDocument};

const MAX_BYTES: usize = 1024 * 1024;
const MAX_ASSIGNMENTS: usize = 100_000;
const MAX_SPACES: usize = 128;

#[derive(Clone, Default)]
pub(super) struct CidEncoding {
    spaces: Vec<CodeSpace>,
    mappings: Arc<HashMap<(u8, u32), u16>>,
    notdefs: Arc<HashMap<(u8, u32), u16>>,
    identity: bool,
    vertical: bool,
    ordering: Option<&'static str>,
    height: usize,
    cost: Budget,
}

fn writing_mode(value: &Object) -> Result<bool, OfdError> {
    match value {
        Object::Integer(0) => Ok(false),
        Object::Integer(1) => Ok(true),
        _ => Err(invalid("WMode must be 0 or 1")),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CodeSpace {
    low: Vec<u8>,
    high: Vec<u8>,
}

impl CodeSpace {
    fn contains(&self, bytes: &[u8]) -> bool {
        self.low.len() == bytes.len()
            && bytes
                .iter()
                .zip(self.low.iter().zip(&self.high))
                .all(|(byte, (low, high))| (low..=high).contains(&byte))
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.low
            .iter()
            .zip(&self.high)
            .zip(other.low.iter().zip(&other.high))
            .all(|((low, high), (other_low, other_high))| low <= other_high && other_low <= high)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DecodedCode {
    pub source: u32,
    pub length: u8,
    pub cid: u16,
}

#[derive(Clone, Copy, Default)]
struct Budget {
    bytes: usize,
    assignments: usize,
}

static NAMED_CACHE: OnceLock<Mutex<VecDeque<(&'static str, CidEncoding)>>> = OnceLock::new();

fn invalid(message: &str) -> OfdError {
    OfdError::Invalid(format!("invalid PDF Encoding CMap: {message}"))
}

fn limit() -> OfdError {
    OfdError::Limit("PDF Encoding CMap exceeds parsing limits".into())
}

impl CidEncoding {
    pub fn ordering(&self) -> Option<&'static str> {
        self.ordering
    }

    fn named(
        pdf: &PdfDocument,
        name: &[u8],
        depth: usize,
        budget: &mut Budget,
    ) -> Result<Self, OfdError> {
        let cache = NAMED_CACHE.get_or_init(Mutex::default);
        let cached = cache.lock().ok().and_then(|maps| {
            maps.iter()
                .find(|(candidate, _)| candidate.as_bytes() == name)
                .map(|(_, map)| map.clone())
        });
        if let Some(map) = cached {
            budget.bytes = budget.bytes.saturating_add(map.cost.bytes);
            budget.assignments = budget.assignments.saturating_add(map.cost.assignments);
            if budget.bytes > MAX_BYTES
                || budget.assignments > MAX_ASSIGNMENTS
                || depth + map.height > 8
            {
                return Err(limit());
            }
            return Ok(map);
        }
        let (name, ordering, data) = named::data(name)?;
        let before = *budget;
        let mut map = Self::parse_inner(
            pdf,
            &Object::Stream(lopdf::Stream::new(lopdf::Dictionary::new(), data)),
            depth,
            budget,
        )?;
        map.ordering = Some(ordering);
        map.cost = Budget {
            bytes: budget.bytes - before.bytes,
            assignments: budget.assignments - before.assignments,
        };
        // Share immutable mappings; inheritance clones only the tables it overrides.
        if let Ok(mut maps) = cache.lock() {
            if maps.len() >= 16 {
                maps.pop_front();
            }
            maps.push_back((name, map.clone()));
        }
        Ok(map)
    }

    pub fn is_vertical(&self) -> bool {
        self.vertical
    }

    pub fn parse(pdf: &PdfDocument, value: &Object) -> Result<Self, OfdError> {
        Self::parse_inner(pdf, value, 0, &mut Budget::default())
    }

    fn parse_inner(
        pdf: &PdfDocument,
        value: &Object,
        depth: usize,
        budget: &mut Budget,
    ) -> Result<Self, OfdError> {
        if depth > 8 {
            return Err(limit());
        }
        let value = pdf_resolved_value(Some(pdf), value, "Type0 Encoding CMap")?;
        if let Object::Name(name) = value {
            if name != b"Identity-H" && name != b"Identity-V" {
                return Self::named(pdf, name, depth, budget);
            }
            return Ok(Self {
                identity: true,
                vertical: name == b"Identity-V",
                spaces: vec![CodeSpace {
                    low: vec![0, 0],
                    high: vec![255, 255],
                }],
                ..Self::default()
            });
        }
        let stream = value
            .as_stream()
            .map_err(|_| invalid("expected a name or stream"))?;
        if stream.content.len() > MAX_BYTES {
            return Err(limit());
        }
        let data = pdf_byte_stream_data(stream, Some(pdf), "Encoding CMap")?;
        budget.bytes = budget.bytes.saturating_add(data.len());
        if budget.bytes > MAX_BYTES {
            return Err(limit());
        }
        let dictionary_mode = stream
            .dict
            .get(b"WMode")
            .ok()
            .map(|mode| writing_mode(pdf_resolved_value(Some(pdf), mode, "CMap WMode")?))
            .transpose()?;
        let mut body_mode = None;
        let base = stream.dict.get(b"UseCMap").ok();
        let mut result = base
            .map(|base| Self::parse_inner(pdf, base, depth + 1, budget))
            .transpose()?
            .unwrap_or_default();
        if base.is_some() {
            result.height += 1;
        }
        let operations = Content::decode(&cmap_source(&data)?)
            .map_err(|_| invalid("malformed CMap syntax"))?
            .operations;
        let mut index = 0;
        let mut in_cmap = false;
        let mut finished = false;
        let mut saw_cmap = false;
        while index < operations.len() {
            let op = &operations[index];
            match op.operator.as_str() {
                "begincmap" if !in_cmap && !saw_cmap && op.operands.is_empty() => {
                    in_cmap = true;
                    saw_cmap = true;
                }
                "endcmap" if in_cmap && op.operands.is_empty() => {
                    in_cmap = false;
                    finished = true;
                }
                "begincodespacerange"
                | "begincidchar"
                | "begincidrange"
                | "beginnotdefchar"
                | "beginnotdefrange"
                    if in_cmap =>
                {
                    let count = match op.operands.as_slice() {
                        [Object::Integer(count)] if *count >= 0 => {
                            usize::try_from(*count).map_err(|_| limit())?
                        }
                        _ => return Err(invalid("section count is invalid")),
                    };
                    if count > MAX_ASSIGNMENTS {
                        return Err(limit());
                    }
                    let end = operations
                        .get(index + 1)
                        .ok_or_else(|| invalid("unterminated section"))?;
                    let expected = op.operator.replacen("begin", "end", 1);
                    if end.operator != expected {
                        return Err(invalid("mismatched section end"));
                    }
                    result.section(&op.operator, count, &end.operands, budget)?;
                    index += 1;
                }
                "usecmap" if in_cmap => {
                    let [Object::Name(name)] = op.operands.as_slice() else {
                        return Err(invalid("usecmap operand is invalid"));
                    };
                    if let Some(base) = base {
                        let base = pdf_resolved_value(Some(pdf), base, "UseCMap")?;
                        let base_name = match base {
                            Object::Name(name) => Some(name.as_slice()),
                            Object::Stream(stream) => stream
                                .dict
                                .get(b"CMapName")
                                .ok()
                                .map(|name| pdf_resolved_value(Some(pdf), name, "CMapName"))
                                .transpose()?
                                .and_then(|value| value.as_name().ok()),
                            _ => None,
                        };
                        if base_name != Some(name.as_slice()) {
                            return Err(invalid("usecmap disagrees with UseCMap"));
                        }
                    } else {
                        // A base must precede all local ranges and mappings.
                        if !result.spaces.is_empty()
                            || !result.mappings.is_empty()
                            || !result.notdefs.is_empty()
                        {
                            return Err(invalid("usecmap follows local definitions"));
                        }
                        result =
                            Self::parse_inner(pdf, &Object::Name(name.clone()), depth + 1, budget)?;
                        result.height += 1;
                    }
                }
                "usefont" if in_cmap => {
                    if !matches!(op.operands.as_slice(), [Object::Integer(0)]) {
                        return Err(invalid("CID descendant font number must be zero"));
                    }
                }
                "def" => match op.operands.as_slice() {
                    [Object::Name(name), value] if name == b"WMode" => {
                        body_mode = Some(writing_mode(value)?);
                    }
                    [Object::Name(name), value]
                        if name == b"CMapType" && value.as_i64().ok() != Some(1) =>
                    {
                        return Err(invalid("Encoding CMapType must be 1"));
                    }
                    _ => {}
                },
                // Interpret the declarative mapping sections, never execute PostScript.
                "CMapName" | "findresource" | "dict" | "begin" | "end" | "dup" | "currentdict"
                | "defineresource" | "pop" | "readonly" => {}
                _ => return Err(invalid("unsupported operator or misplaced section")),
            }
            index += 1;
        }
        if let (Some(dictionary), Some(body)) = (dictionary_mode, body_mode) {
            if dictionary != body {
                return Err(invalid("dictionary and program WMode differ"));
            }
        }
        if let Some(mode) = dictionary_mode.or(body_mode) {
            result.vertical = mode;
        }
        if !finished || in_cmap || result.spaces.is_empty() {
            return Err(invalid("incomplete CMap definition"));
        }
        if depth + result.height > 8 {
            return Err(limit());
        }
        for &(length, source) in result.mappings.keys().chain(result.notdefs.keys()) {
            let bytes = source.to_be_bytes();
            if !result
                .spaces
                .iter()
                .any(|space| space.contains(&bytes[4 - usize::from(length)..]))
            {
                return Err(invalid("mapping lies outside the codespace"));
            }
        }
        Ok(result)
    }

    fn section(
        &mut self,
        kind: &str,
        count: usize,
        values: &[Object],
        budget: &mut Budget,
    ) -> Result<(), OfdError> {
        let range = kind.ends_with("range");
        let space = kind == "begincodespacerange";
        let stride = if range && !space { 3 } else { 2 };
        if values.len() != count * stride {
            return Err(invalid("section count does not match its entries"));
        }
        for entry in values.chunks_exact(stride) {
            let first = code_bytes(&entry[0])?;
            let last = if range { code_bytes(&entry[1])? } else { first };
            if first.len() != last.len() || first > last {
                return Err(invalid("invalid source range"));
            }
            if space {
                if first.iter().zip(last).any(|(low, high)| low > high) {
                    return Err(invalid("invalid codespace bounds"));
                }
                let candidate = CodeSpace {
                    low: first.to_vec(),
                    high: last.to_vec(),
                };
                if self.spaces.contains(&candidate) {
                    continue;
                }
                if self.spaces.iter().any(|space| space.overlaps(&candidate)) {
                    return Err(invalid("ambiguous codespace ranges"));
                }
                if self.spaces.len() == MAX_SPACES {
                    return Err(limit());
                }
                self.spaces.push(candidate);
                continue;
            }
            if self.spaces.is_empty() {
                return Err(invalid("mapping precedes codespace definition"));
            }
            let cid = entry[stride - 1]
                .as_i64()
                .ok()
                .and_then(|value| u16::try_from(value).ok())
                .ok_or_else(|| invalid("CID is outside 0 through 65535"))?;
            let first_code = code_number(first);
            let last_code = code_number(last);
            let assignments = u64::from(last_code) - u64::from(first_code) + 1;
            if assignments > (MAX_ASSIGNMENTS - budget.assignments) as u64 {
                return Err(limit());
            }
            budget.assignments += assignments as usize;
            let notdef = kind.starts_with("beginnotdef");
            if !notdef && u64::from(cid) + assignments - 1 > u64::from(u16::MAX) {
                return Err(invalid("CID range overflows"));
            }
            let target = if notdef {
                Arc::make_mut(&mut self.notdefs)
            } else {
                Arc::make_mut(&mut self.mappings)
            };
            for source in first_code..=last_code {
                let value = if notdef {
                    cid
                } else {
                    cid + (source - first_code) as u16
                };
                target.insert((first.len() as u8, source), value);
            }
        }
        Ok(())
    }

    pub fn decode(&self, bytes: &[u8]) -> Result<Vec<DecodedCode>, OfdError> {
        let mut codes = Vec::new();
        let mut offset = 0;
        while offset < bytes.len() {
            let length = (1..=4)
                .find(|length| {
                    bytes
                        .get(offset..offset + length)
                        .is_some_and(|code| self.spaces.iter().any(|space| space.contains(code)))
                })
                .ok_or_else(|| invalid("text contains a truncated or invalid source code"))?;
            let source = code_number(&bytes[offset..offset + length]);
            let key = (length as u8, source);
            let cid = self
                .mappings
                .get(&key)
                .copied()
                .or_else(|| (self.identity && length == 2).then_some(source as u16))
                .or_else(|| self.notdefs.get(&key).copied())
                .unwrap_or(0);
            codes.push(DecodedCode {
                source,
                length: length as u8,
                cid,
            });
            offset += length;
        }
        Ok(codes)
    }
}

fn code_bytes(value: &Object) -> Result<&[u8], OfdError> {
    let bytes = value
        .as_str()
        .map_err(|_| invalid("source code is not a byte string"))?;
    if bytes.is_empty() || bytes.len() > 4 {
        return Err(invalid("source codes must contain one through four bytes"));
    }
    Ok(bytes)
}

fn code_number(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .fold(0, |value, byte| (value << 8) | u32::from(*byte))
}

fn cmap_source(data: &[u8]) -> Result<Vec<u8>, OfdError> {
    // lopdf's content decoder can stop at a comment followed by an empty line,
    // and does not accept comments between section operands. Keep literal and
    // hexadecimal strings intact while removing only lexical comments.
    let mut output = Vec::with_capacity(data.len());
    let mut containers = Vec::new();
    let mut literal = 0_usize;
    let mut escaped = false;
    let mut hex = false;
    let mut index = 0;
    while index < data.len() {
        let byte = data[index];
        if literal > 0 {
            output.push(byte);
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'(' {
                literal += 1;
            } else if byte == b')' {
                literal -= 1;
            }
        } else if hex {
            output.push(byte);
            if byte == b'>' {
                hex = false;
            }
        } else {
            match byte {
                b'%' => {
                    while index < data.len() && !matches!(data[index], b'\r' | b'\n') {
                        index += 1;
                    }
                    output.push(b' ');
                    continue;
                }
                b'(' => literal = 1,
                b'[' => containers.push(b']'),
                b'<' if data.get(index + 1) == Some(&b'<') => {
                    containers.push(b'>');
                    output.push(byte);
                    index += 1;
                }
                b'<' => hex = true,
                b']' => {
                    if containers.pop() != Some(b']') {
                        return Err(invalid("unbalanced CMap array"));
                    }
                }
                b'>' => {
                    if data.get(index + 1) != Some(&b'>') || containers.pop() != Some(b'>') {
                        return Err(invalid("unbalanced CMap dictionary"));
                    }
                    output.push(byte);
                    index += 1;
                }
                _ => {}
            }
            output.push(byte);
        }
        if literal + containers.len() > 64 {
            return Err(limit());
        }
        index += 1;
    }
    if literal != 0 || hex || !containers.is_empty() {
        return Err(invalid("unterminated CMap operand"));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Stream};

    fn parse(body: &str) -> Result<CidEncoding, OfdError> {
        let data = format!("/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n/CMapType 1 def /WMode 0 def\n{body}\nendcmap CMapName currentdict /CMap defineresource pop end end");
        CidEncoding::parse(
            &PdfDocument::new(),
            &Object::Stream(Stream::new(dictionary! {}, data.into_bytes())),
        )
    }

    #[test]
    fn mixed_length_codes_use_source_bytes_and_separate_cid_mappings() {
        let cmap = parse("4 begincodespacerange <00> <7f> <8140> <81ff> <900000> <9000ff> <a0000000> <a00000ff> endcodespacerange\n4 begincidchar <41> 1 <8140> 20 <900001> 300 <a0000002> 4000 endcidchar").unwrap();
        assert_eq!(
            cmap.decode(&[0x41, 0x81, 0x40, 0x90, 0, 1, 0xa0, 0, 0, 2])
                .unwrap(),
            [
                DecodedCode {
                    source: 0x41,
                    length: 1,
                    cid: 1
                },
                DecodedCode {
                    source: 0x8140,
                    length: 2,
                    cid: 20
                },
                DecodedCode {
                    source: 0x900001,
                    length: 3,
                    cid: 300
                },
                DecodedCode {
                    source: 0xa0000002,
                    length: 4,
                    cid: 4000
                },
            ]
        );
        assert!(cmap.decode(&[0x81, 0x20]).is_err());
        assert!(cmap.decode(&[0x90, 0]).is_err());
        assert!(cmap.decode(&[0xff]).is_err());
    }

    #[test]
    fn cid_ranges_increment_while_notdef_ranges_keep_constant_cids() {
        let cmap = parse("1 begincodespacerange <00> <ff> endcodespacerange\n1 beginnotdefrange <00> <7f> 5 endnotdefrange\n1 begincidrange <40> <42> 100 endcidrange\n1 begincidchar <41> 999 endcidchar\n1 beginnotdefchar <01> 6 endnotdefchar").unwrap();
        assert_eq!(
            cmap.decode(&[0, 1, 0x40, 0x41, 0x42, 0x80])
                .unwrap()
                .iter()
                .map(|code| code.cid)
                .collect::<Vec<_>>(),
            [5, 6, 100, 999, 102, 0]
        );
    }

    #[test]
    fn identity_base_and_indirect_stream_inheritance_allow_local_overrides() {
        let mut pdf = PdfDocument::new();
        let base = pdf.add_object(Stream::new(
            dictionary! {"CMapName" => Object::Name(b"Base".to_vec())},
            b"begincmap /Identity-H usecmap 1 begincidchar <0041> 100 endcidchar endcmap".to_vec(),
        ));
        let child = Object::Stream(Stream::new(
            dictionary! {"UseCMap" => base},
            b"begincmap /Base usecmap 1 begincidchar <0042> 200 endcidchar endcmap".to_vec(),
        ));
        let cmap = CidEncoding::parse(&pdf, &child).unwrap();
        assert_eq!(
            cmap.decode(&[0, 65, 0, 66, 0, 67])
                .unwrap()
                .iter()
                .map(|code| code.cid)
                .collect::<Vec<_>>(),
            [100, 200, 67]
        );
        let child = Object::Stream(Stream::new(
            dictionary! {"UseCMap" => Object::Name(b"Identity-H".to_vec())},
            b"begincmap /Identity-H usecmap endcmap".to_vec(),
        ));
        assert_eq!(
            CidEncoding::parse(&pdf, &child)
                .unwrap()
                .decode(&[0xff, 0xff])
                .unwrap()[0]
                .cid,
            65535
        );
    }

    #[test]
    fn vertical_modes_inherit_override_and_validate_metadata() {
        let mut pdf = PdfDocument::new();
        let identity = Object::Name(b"Identity-V".to_vec());
        assert!(CidEncoding::parse(&pdf, &identity).unwrap().is_vertical());
        let mode = pdf.add_object(1);
        let stream = Stream::new(
            dictionary! {"UseCMap" => identity.clone(), "WMode" => mode},
            b"begincmap endcmap".to_vec(),
        );
        let parsed = CidEncoding::parse(&pdf, &Object::Stream(stream)).unwrap();
        assert!(parsed.is_vertical());
        assert_eq!(parsed.decode(&[0, 65]).unwrap()[0].cid, 65);
        let stream = Stream::new(
            dictionary! {"UseCMap" => identity.clone()},
            b"begincmap /WMode 0 def endcmap".to_vec(),
        );
        assert!(!CidEncoding::parse(&pdf, &Object::Stream(stream))
            .unwrap()
            .is_vertical());
        let stream = Stream::new(
            dictionary! {"UseCMap" => identity, "WMode" => 0},
            b"begincmap /WMode 1 def endcmap".to_vec(),
        );
        assert!(CidEncoding::parse(&pdf, &Object::Stream(stream)).is_err());
    }

    #[test]
    fn compressed_cmap_stream_and_indirect_wmode_import() {
        let mut pdf = PdfDocument::new();
        let mode = pdf.add_object(Object::Integer(0));
        let mut stream = Stream::new(dictionary! {"WMode" => mode}, b"begincmap 1 begincodespacerange <00> <ff> endcodespacerange 1 begincidchar <41> 12 endcidchar endcmap".to_vec());
        stream.compress().unwrap();
        assert_eq!(
            CidEncoding::parse(&pdf, &Object::Stream(stream))
                .unwrap()
                .decode(b"A")
                .unwrap()[0]
                .cid,
            12
        );
    }

    #[test]
    fn invalid_sections_and_metadata_return_errors() {
        for body in [
            "1 begincodespacerange <00> <ff> endcidchar",
            "2 begincodespacerange <00> <ff> endcodespacerange",
            "-1 begincodespacerange endcodespacerange",
            "1 begincodespacerange <> <ff> endcodespacerange",
            "1 begincodespacerange <0000000000> <ffffffff00> endcodespacerange",
            "1 begincodespacerange <ff> <00> endcodespacerange",
            "1 begincodespacerange <00> <ffff> endcodespacerange",
            "1 begincodespacerange <81ff> <8200> endcodespacerange",
            "2 begincodespacerange <00> <ff> <8100> <81ff> endcodespacerange",
            "2 begincodespacerange <00> <7f> <40> <80> endcodespacerange",
            "1 begincidchar <41> 0 endcidchar",
            "1 begincodespacerange <00> <7f> endcodespacerange 1 begincidchar <80> 1 endcidchar",
            "1 begincodespacerange <00> <ff> endcodespacerange 1 begincidchar <41> -1 endcidchar",
            "1 begincodespacerange <00> <ff> endcodespacerange 1 begincidchar <41> 65536 endcidchar",
            "1 begincodespacerange <00> <ff> endcodespacerange 1 begincidchar <41> 1.5 endcidchar",
            "1 begincodespacerange <00> <ff> endcodespacerange 1 begincidrange <41> <42> 65535 endcidrange",
            "1 begincodespacerange <00> <ff> endcodespacerange 1 begincidchar 65 1 endcidchar",
            "1 begincodespacerange <00> <ff> endcodespacerange /WMode 2 def",
            "1 begincodespacerange <00> <ff> endcodespacerange /CMapType 2 def",
            "1 begincodespacerange <00> <ff> endcodespacerange 1 usefont",
            "1 begincodespacerange <00> <ff> endcodespacerange 1 2 add",
            "1 begincodespacerange <00> <ff> endcodespacerange /Identity-H usecmap",
            "begincmap endcmap",
        ] { assert!(parse(body).is_err(), "{body}"); }
        for value in [Object::Null, Object::Name(b"Unknown-CMap".to_vec())] {
            assert!(CidEncoding::parse(&PdfDocument::new(), &value).is_err());
        }
    }

    #[test]
    fn inheritance_cycles_and_resource_exhaustion_are_bounded() {
        let mut pdf = PdfDocument::new();
        let id = pdf.add_object(Stream::new(
            dictionary! {"UseCMap" => Object::Reference((1, 0))},
            b"begincmap endcmap".to_vec(),
        ));
        assert!(matches!(
            CidEncoding::parse(&pdf, &Object::Reference(id)),
            Err(OfdError::Limit(_))
        ));
        assert!(matches!(parse("1 begincodespacerange <00000000> <ffffffff> endcodespacerange 1 beginnotdefrange <00000000> <ffffffff> 0 endnotdefrange"), Err(OfdError::Limit(_))));
        let repeated = "1 begincidrange <0000> <ffff> 0 endcidrange\n".repeat(2);
        assert!(matches!(
            parse(&format!(
                "1 begincodespacerange <0000> <ffff> endcodespacerange {repeated}"
            )),
            Err(OfdError::Limit(_))
        ));
        let huge = Object::Stream(Stream::new(dictionary! {}, vec![b' '; MAX_BYTES + 1]));
        assert!(matches!(
            CidEncoding::parse(&pdf, &huge),
            Err(OfdError::Limit(_))
        ));
    }

    #[test]
    fn every_bundled_adobe_cmap_parses_without_network_access() {
        let pdf = PdfDocument::new();
        for (name, ordering) in named::NAMES {
            let map = CidEncoding::parse(&pdf, &Object::Name(name.as_bytes().to_vec()))
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(
                map.is_vertical(),
                name.ends_with("-V") || *name == "V",
                "{name}"
            );
            assert_eq!(
                map.ordering(),
                (!matches!(*name, "Identity-H" | "Identity-V")).then_some(*ordering),
                "{name}"
            );
        }
        assert_eq!(named::NAMES.len(), 189);
        assert_eq!(
            named::NAMES
                .iter()
                .filter(|(name, _)| name.ends_with("-V") || *name == "V")
                .count(),
            73
        );
    }

    #[test]
    fn named_vertical_maps_override_punctuation_and_preserve_horizontal_bases() {
        let pdf = PdfDocument::new();
        for (horizontal, vertical, code, expected) in [
            ("UniCNS-UTF16-H", "UniCNS-UTF16-V", 0x3008_u16, 150),
            ("UniGB-UTF16-H", "UniGB-UTF16-V", 0x3001, 575),
            ("UniJIS-UTF16-H", "UniJIS-UTF16-V", 0x3001, 7887),
        ] {
            let h =
                CidEncoding::parse(&pdf, &Object::Name(horizontal.as_bytes().to_vec())).unwrap();
            let v = CidEncoding::parse(&pdf, &Object::Name(vertical.as_bytes().to_vec())).unwrap();
            assert!(!h.is_vertical());
            assert!(v.is_vertical());
            assert_eq!(v.decode(&code.to_be_bytes()).unwrap()[0].cid, expected);
            assert_ne!(h.decode(&code.to_be_bytes()).unwrap()[0].cid, expected);
            // A cached vertical map retains its own mode after the base is loaded.
            assert!(
                CidEncoding::parse(&pdf, &Object::Name(vertical.as_bytes().to_vec()))
                    .unwrap()
                    .is_vertical()
            );
        }
    }

    #[test]
    fn named_cmap_inheritance_preserves_base_and_charges_cached_work() {
        let pdf = PdfDocument::new();
        let name = Object::Name(b"GBK-EUC-H".to_vec());
        let first = CidEncoding::parse(&pdf, &name).unwrap();
        let second = CidEncoding::parse(&pdf, &name).unwrap();
        assert!(first.cost.assignments > 10_000);
        let child = Object::Stream(Stream::new(
            dictionary! {"UseCMap" => name.clone()},
            b"begincmap /GBK-EUC-H usecmap 1 begincidchar <d6d0> 123 endcidchar endcmap".to_vec(),
        ));
        let child = CidEncoding::parse(&pdf, &child).unwrap();
        assert_eq!(child.decode(&[0xd6, 0xd0]).unwrap()[0].cid, 123);
        assert_eq!(first.decode(&[0xd6, 0xd0]).unwrap()[0].cid, 4559);
        assert_eq!(second.decode(&[0xd6, 0xd0]).unwrap()[0].cid, 4559);
        let mut full_budget = Budget {
            bytes: MAX_BYTES,
            assignments: 0,
        };
        assert!(matches!(
            CidEncoding::parse_inner(&pdf, &name, 0, &mut full_budget),
            Err(OfdError::Limit(_))
        ));
    }

    #[test]
    fn cmap_comments_do_not_change_strings_or_split_section_operands() {
        let source = b"% header\n\n/Label (50% \\(nested\\)) def % trailer\n/Hex <25> def";
        let cleaned = cmap_source(source).unwrap();
        assert!(cleaned
            .windows(b"(50% \\(nested\\))".len())
            .any(|bytes| bytes == b"(50% \\(nested\\))"));
        assert!(cleaned.windows(b"<25>".len()).any(|bytes| bytes == b"<25>"));
        let cmap = parse("% section\n\n1 begincodespacerange <00> % low bound\n<ff> endcodespacerange\n1 begincidchar <41> % source\n12 endcidchar").unwrap();
        assert_eq!(cmap.decode(b"A").unwrap()[0].cid, 12);
        for bytes in [
            b"[".as_slice(),
            b"<<",
            b"(unterminated",
            b"<00",
            b"]",
            b">>",
            b"[>>",
        ] {
            assert!(cmap_source(bytes).is_err());
        }
        for bytes in [b"[".repeat(65), b"(".repeat(65), b"<<".repeat(65)] {
            assert!(matches!(cmap_source(&bytes), Err(OfdError::Limit(_))));
        }
    }
}
