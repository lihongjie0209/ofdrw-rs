//! CIDFont vertical metrics (ISO 32000-1, 9.7.4.3).

use super::{pdf_resolved_value, HashMap, Object, OfdError, PdfDocument};

pub(super) struct VerticalMetrics {
    defaults: [f64; 2],
    values: HashMap<u16, [f64; 3]>,
}

fn invalid() -> OfdError {
    OfdError::Invalid("PDF CID vertical metrics are invalid".into())
}

fn number(pdf: &PdfDocument, value: &Object) -> Result<f64, OfdError> {
    let value = f64::from(
        pdf_resolved_value(Some(pdf), value, "CID vertical metric")?
            .as_float()
            .map_err(|_| invalid())?,
    );
    if !value.is_finite() || value.abs() > 10_000.0 {
        return Err(invalid());
    }
    Ok(value)
}

fn cid(pdf: &PdfDocument, value: &Object) -> Result<u16, OfdError> {
    let value = pdf_resolved_value(Some(pdf), value, "CID W2 range")?
        .as_i64()
        .map_err(|_| invalid())?;
    u16::try_from(value).map_err(|_| invalid())
}

fn triple(pdf: &PdfDocument, values: &[Object]) -> Result<[f64; 3], OfdError> {
    if values.len() != 3 {
        return Err(invalid());
    }
    Ok([
        number(pdf, &values[0])?,
        number(pdf, &values[1])?,
        number(pdf, &values[2])?,
    ])
}

impl VerticalMetrics {
    pub fn parse(pdf: &PdfDocument, font: &lopdf::Dictionary) -> Result<Self, OfdError> {
        let defaults = if let Ok(value) = font.get(b"DW2") {
            let values = pdf_resolved_value(Some(pdf), value, "CID DW2")?
                .as_array()
                .map_err(|_| invalid())?;
            if values.len() != 2 {
                return Err(invalid());
            }
            [number(pdf, &values[0])?, number(pdf, &values[1])?]
        } else {
            [880.0, -1000.0]
        };
        let mut result = Self {
            defaults,
            values: HashMap::new(),
        };
        let Ok(value) = font.get(b"W2") else {
            return Ok(result);
        };
        let values = pdf_resolved_value(Some(pdf), value, "CID W2")?
            .as_array()
            .map_err(|_| invalid())?;
        let mut index = 0;
        let mut assignments = 0_usize;
        while index < values.len() {
            let first = cid(pdf, &values[index])?;
            let next = values.get(index + 1).ok_or_else(invalid)?;
            let next = pdf_resolved_value(Some(pdf), next, "CID W2 entry")?;
            if let Object::Array(array) = next {
                if array.is_empty()
                    || array.len() % 3 != 0
                    || array.len() / 3 > 65536 - usize::from(first)
                {
                    return Err(invalid());
                }
                assignments += array.len() / 3;
                check_assignments(assignments)?;
                for (offset, values) in array.chunks_exact(3).enumerate() {
                    result
                        .values
                        .insert(first + offset as u16, triple(pdf, values)?);
                }
                index += 2;
            } else {
                let last = cid(pdf, next)?;
                if last < first {
                    return Err(invalid());
                }
                let metrics = triple(pdf, values.get(index + 2..index + 5).ok_or_else(invalid)?)?;
                assignments += usize::from(last) - usize::from(first) + 1;
                check_assignments(assignments)?;
                for cid in first..=last {
                    result.values.insert(cid, metrics);
                }
                index += 5;
            }
        }
        Ok(result)
    }

    pub fn get(&self, cid: u16, width: f64) -> [f64; 3] {
        self.values
            .get(&cid)
            .copied()
            .unwrap_or([self.defaults[1], width / 2.0, self.defaults[0]])
    }
}

fn check_assignments(count: usize) -> Result<(), OfdError> {
    if count > 1_000_000 {
        return Err(OfdError::Limit(
            "PDF CID W2 has too many metric assignments".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    #[test]
    fn defaults_use_horizontal_width_and_standard_vertical_origin() {
        let metrics = VerticalMetrics::parse(&PdfDocument::new(), &dictionary! {}).unwrap();
        assert_eq!(metrics.get(12, 750.0), [-1000.0, 375.0, 880.0]);
    }

    #[test]
    fn arrays_ranges_overrides_and_indirect_values_resolve() {
        let mut pdf = PdfDocument::new();
        let displacement = pdf.add_object(-900);
        let defaults = pdf.add_object(Object::Array(vec![800.into(), (-1100).into()]));
        let array = pdf.add_object(Object::Array(vec![
            displacement.into(),
            200.into(),
            700.into(),
        ]));
        let entries = pdf.add_object(Object::Array(vec![
            1.into(),
            3.into(),
            (-1000).into(),
            500.into(),
            880.into(),
            2.into(),
            array.into(),
        ]));
        let metrics =
            VerticalMetrics::parse(&pdf, &dictionary! {"DW2" => defaults, "W2" => entries})
                .unwrap();
        assert_eq!(metrics.get(1, 200.0), [-1000.0, 500.0, 880.0]);
        assert_eq!(metrics.get(2, 200.0), [-900.0, 200.0, 700.0]);
        assert_eq!(metrics.get(4, 600.0), [-1100.0, 300.0, 800.0]);
    }

    #[test]
    fn malformed_ranges_and_metrics_are_rejected_eagerly() {
        let pdf = PdfDocument::new();
        for entries in [
            vec![1.into()],
            vec![1.into(), Object::Array(vec![])],
            vec![1.into(), Object::Array(vec![(-1000).into()])],
            vec![65535.into(), Object::Array(vec![0.into(); 6])],
            vec![2.into(), 1.into(), 0.into(), 0.into(), 0.into()],
            vec![1.into(), 2.into(), (-10001).into(), 0.into(), 0.into()],
        ] {
            assert!(
                VerticalMetrics::parse(&pdf, &dictionary! {"W2" => Object::Array(entries)})
                    .is_err()
            );
        }
        assert!(VerticalMetrics::parse(&pdf, &dictionary! {"DW2" => vec![0.into()]}).is_err());
    }

    #[test]
    fn repeated_overwrites_count_toward_work_limit() {
        let mut entries = Vec::new();
        for _ in 0..16 {
            entries.extend([
                0.into(),
                65535.into(),
                (-1000).into(),
                500.into(),
                880.into(),
            ]);
        }
        assert!(matches!(
            VerticalMetrics::parse(&PdfDocument::new(), &dictionary! {"W2" => entries}),
            Err(OfdError::Limit(_))
        ));
    }
}
