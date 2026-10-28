use crate::{OfdError, MAX_ENTRY_BYTES};

/// Row-aligned one-bit samples, with one representing white.
pub(crate) struct BilevelSamples {
    samples: Vec<u8>,
    width: u32,
    height: u32,
    row_bytes: usize,
    row: u32,
    column: u32,
    invalid: bool,
}

impl BilevelSamples {
    pub(crate) fn new(width: u32, height: u32) -> Result<Self, OfdError> {
        let row_bytes = (width as usize).div_ceil(8);
        let length = row_bytes
            .checked_mul(height as usize)
            .filter(|length| *length as u64 <= MAX_ENTRY_BYTES)
            .ok_or_else(|| OfdError::Limit("bilevel image is too large".into()))?;
        if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 64_000_000 {
            return Err(OfdError::Limit(
                "bilevel image dimensions are too large".into(),
            ));
        }
        Ok(Self {
            samples: vec![0; length],
            width,
            height,
            row_bytes,
            row: 0,
            column: 0,
            invalid: false,
        })
    }

    pub(crate) fn finish(self) -> Result<Vec<u8>, OfdError> {
        if self.invalid || self.row != self.height || self.column != 0 {
            return Err(OfdError::Invalid(
                "decoded bilevel image has incomplete rows".into(),
            ));
        }
        Ok(self.samples)
    }

    fn push_samples(&mut self, one: bool, count: u32) {
        let Some(end) = self.column.checked_add(count) else {
            self.invalid = true;
            return;
        };
        if self.row >= self.height || end > self.width {
            self.invalid = true;
            return;
        }
        if one {
            let offset = self.row as usize * self.row_bytes;
            for column in self.column..end {
                self.samples[offset + column as usize / 8] |= 0x80 >> (column % 8);
            }
        }
        self.column = end;
    }

    fn finish_line(&mut self) {
        if self.column != self.width || self.row >= self.height {
            self.invalid = true;
        }
        self.row = self.row.saturating_add(1);
        self.column = 0;
    }
}

impl hayro_ccitt::Decoder for BilevelSamples {
    fn push_pixels(&mut self, one: bool, count: u32) {
        // BlackIs1 was applied by the fax decoder; `one` is a PDF sample bit.
        self.push_samples(one, count);
    }

    fn next_line(&mut self) {
        self.finish_line();
    }
}

impl hayro_jbig2::Decoder for BilevelSamples {
    fn push_pixel(&mut self, black: bool) {
        self.push_samples(!black, 1);
    }

    fn push_pixel_chunk(&mut self, black: bool, chunk_count: u32) {
        if let Some(count) = chunk_count.checked_mul(8) {
            self.push_samples(!black, count);
        } else {
            self.invalid = true;
        }
    }

    fn next_line(&mut self) {
        self.finish_line();
    }
}
