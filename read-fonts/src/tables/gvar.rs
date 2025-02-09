//! The [gvar (Glyph Variations)](https://learn.microsoft.com/en-us/typography/opentype/spec/gvar)
//! table

include!("../../generated/generated_gvar.rs");

use super::variations::{
    DeltaRunIter, PackedDeltas, PackedPointNumbers, PackedPointNumbersIter, Tuple,
    TupleVariationCount, TupleVariationHeader, TupleVariationHeaderIter,
};

#[derive(Clone, Copy, Debug)]
pub struct U16Or32(u32);

impl ReadArgs for U16Or32 {
    type Args = GvarFlags;
}

impl ComputeSize for U16Or32 {
    fn compute_size(args: &GvarFlags) -> usize {
        if args.contains(GvarFlags::LONG_OFFSETS) {
            4
        } else {
            2
        }
    }
}

impl FontReadWithArgs<'_> for U16Or32 {
    fn read_with_args(data: FontData<'_>, args: &Self::Args) -> Result<Self, ReadError> {
        if args.contains(GvarFlags::LONG_OFFSETS) {
            data.read_at::<u32>(0).map(Self)
        } else {
            data.read_at::<u16>(0).map(|v| Self(v as u32 * 2))
        }
    }
}

impl U16Or32 {
    fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone)]
pub struct GlyphVariationData<'a> {
    axis_count: u16,
    shared_tuples: SharedTuples<'a>,
    shared_point_numbers: Option<PackedPointNumbers<'a>>,
    tuple_count: TupleVariationCount,
    // the data for all the tuple variation headers
    header_data: FontData<'a>,
    // the data for all the tuple bodies
    serialized_data: FontData<'a>,
}

impl<'a> GlyphVariationDataHeader<'a> {
    fn raw_tuple_header_data(&self) -> FontData<'a> {
        let range = self.shape.tuple_variation_headers_byte_range();
        self.data.split_off(range.start).unwrap()
    }
}

impl<'a> Gvar<'a> {
    fn data_for_gid(&self, gid: GlyphId) -> Result<FontData<'a>, ReadError> {
        let start_idx = gid.to_u16() as usize;
        let end_idx = start_idx + 1;
        let data_start = self.glyph_variation_data_array_offset();
        let start = data_start + self.glyph_variation_data_offsets().get(start_idx)?.get();
        let end = data_start + self.glyph_variation_data_offsets().get(end_idx)?.get();

        self.data
            .slice(start as usize..end as usize)
            .ok_or(ReadError::OutOfBounds)
    }

    /// Get the variation data for a specific glyph.
    pub fn glyph_variation_data(&self, gid: GlyphId) -> Result<GlyphVariationData<'a>, ReadError> {
        let shared_tuples = self.shared_tuples()?;
        let axis_count = self.axis_count();
        let data = self.data_for_gid(gid)?;
        GlyphVariationData::new(data, axis_count, shared_tuples)
    }
}

impl<'a> GlyphVariationData<'a> {
    pub(crate) fn new(
        data: FontData<'a>,
        axis_count: u16,
        shared_tuples: SharedTuples<'a>,
    ) -> Result<Self, ReadError> {
        let header = GlyphVariationDataHeader::read(data)?;

        let header_data = header.raw_tuple_header_data();
        let count = header.tuple_variation_count();
        let data = header.serialized_data()?;

        // if there are shared point numbers, get them now
        let (shared_point_numbers, serialized_data) =
            if header.tuple_variation_count().shared_point_numbers() {
                let (packed, data) = PackedPointNumbers::split_off_front(data);
                (Some(packed), data)
            } else {
                (None, data)
            };

        Ok(GlyphVariationData {
            tuple_count: count,
            axis_count,
            shared_tuples,
            shared_point_numbers,
            header_data,
            serialized_data,
        })
    }

    /// Return an iterator over all of the variation tuples for this glyph.
    pub fn tuples(&self) -> TupleVariationIter<'a> {
        TupleVariationIter {
            current: 0,
            parent: self.clone(),
            header_iter: TupleVariationHeaderIter::new(
                self.header_data,
                self.tuple_count(),
                self.axis_count,
            ),
            serialized_data: self.serialized_data,
        }
    }

    fn tuple_count(&self) -> usize {
        self.tuple_count.count() as usize
    }
}

/// An iterator over the [`TupleVariation`]s for a specific glyph.
pub struct TupleVariationIter<'a> {
    current: usize,
    parent: GlyphVariationData<'a>,
    header_iter: TupleVariationHeaderIter<'a>,
    serialized_data: FontData<'a>,
}

impl<'a> TupleVariationIter<'a> {
    fn next_tuple(&mut self) -> Option<TupleVariation<'a>> {
        if self.parent.tuple_count() == self.current {
            return None;
        }
        self.current += 1;

        // FIXME: is it okay to discard an error here?
        let header = self.header_iter.next()?.ok()?;
        let data_len = header.variation_data_size() as usize;
        let var_data = self.serialized_data.take_up_to(data_len)?;

        let (point_numbers, packed_deltas) = if header.tuple_index().private_point_numbers() {
            PackedPointNumbers::split_off_front(var_data)
        } else {
            (self.parent.shared_point_numbers.clone()?, var_data)
        };
        Some(TupleVariation {
            axis_count: self.parent.axis_count,
            header,
            shared_tuples: self.parent.shared_tuples.clone(),
            packed_deltas: PackedDeltas::new(packed_deltas),
            point_numbers,
        })
    }
}

impl<'a> Iterator for TupleVariationIter<'a> {
    type Item = TupleVariation<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_tuple()
    }
}

/// A single set of tuple variation data
#[derive(Clone)]
pub struct TupleVariation<'a> {
    axis_count: u16,
    header: TupleVariationHeader<'a>,
    shared_tuples: SharedTuples<'a>,
    packed_deltas: PackedDeltas<'a>,
    point_numbers: PackedPointNumbers<'a>,
}

impl<'a> TupleVariation<'a> {
    /// Returns true if this tuple provides deltas for all points in a glyph.
    pub fn all_points(&self) -> bool {
        self.point_numbers.count() == 0
    }

    /// Returns the 'peak' tuple for this variation
    pub fn peak(&self) -> Tuple<'a> {
        self.header
            .tuple_index()
            .tuple_records_index()
            .and_then(|idx| self.shared_tuples.tuples().get(idx as usize).ok())
            .or_else(|| self.header.peak_tuple())
            .unwrap_or_default()
    }

    // transcribed from pinot/moscato
    /// Compute the scalar for a this tuple at a given point in design space.
    ///
    /// The `coords` slice must be of lesser or equal length to the number of axes.
    /// If it is less, missing (trailing) axes will be assumed to have zero values.
    ///
    /// Returns `None` if this tuple is not applicable at the provided coordinates.
    pub fn compute_scalar(&self, coords: &[F2Dot14]) -> Option<Fixed> {
        const ZERO: Fixed = Fixed::ZERO;
        let mut scalar = Fixed::ONE;
        let peak = self.peak();
        let inter_start = self.header.intermediate_start_tuple();
        let inter_end = self.header.intermediate_end_tuple();
        if peak.len() != self.axis_count as usize {
            return None;
        }

        for i in 0..self.axis_count {
            let i = i as usize;
            let coord = coords.get(i).copied().unwrap_or_default().to_fixed();
            let peak = peak.get(i).unwrap_or_default().to_fixed();
            if peak == ZERO || peak == coord {
                continue;
            }

            if coord == ZERO {
                return None;
            }

            if let (Some(inter_start), Some(inter_end)) = (&inter_start, &inter_end) {
                let start = inter_start.get(i).unwrap_or_default().to_fixed();
                let end = inter_end.get(i).unwrap_or_default().to_fixed();
                if coord <= start || coord >= end {
                    return None;
                }
                if coord < peak {
                    scalar = scalar.mul_div(coord - start, peak - start);
                } else {
                    scalar = scalar.mul_div(end - coord, end - peak);
                }
            } else {
                if coord < peak.min(ZERO) || coord > peak.max(ZERO) {
                    return None;
                }
                scalar = scalar.mul_div(coord, peak);
            }
        }
        Some(scalar)
    }

    /// Iterate over the deltas for this tuple.
    ///
    /// This does not account for scaling.
    pub fn deltas(&self) -> DeltaIter<'a> {
        let total = self.packed_deltas.count() / 2;
        let x_iter = self.packed_deltas.iter();
        let mut y_iter = self.packed_deltas.iter();
        for _ in 0..total {
            y_iter.next();
        }
        DeltaIter {
            cur: 0,
            total,
            points: self.point_numbers.iter(),
            x_iter,
            y_iter,
        }
    }
}

/// An iterator over the deltas for a glyph.
#[derive(Clone, Debug)]
pub struct DeltaIter<'a> {
    cur: usize,
    total: usize,
    points: PackedPointNumbersIter<'a>,
    x_iter: DeltaRunIter<'a>,
    y_iter: DeltaRunIter<'a>,
}

/// Delta information for a single point or component in a glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphDelta {
    /// the point or component id
    pub position: u16,
    /// The x delta
    pub x_delta: i16,
    /// The y delta
    pub y_delta: i16,
}

impl<'a> Iterator for DeltaIter<'a> {
    type Item = GlyphDelta;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur == self.total {
            return None;
        }
        self.cur += 1;

        let position = self.points.next()?;
        let x_delta = self.x_iter.next()?;
        let y_delta = self.y_iter.next()?;
        Some(GlyphDelta {
            position,
            x_delta,
            y_delta,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FontRef, TableProvider};

    // Shared tuples in the 'gvar' table of the Skia font, as printed
    // in Apple's TrueType specification.
    // https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6gvar.html
    static SKIA_GVAR_SHARED_TUPLES_DATA: FontData = FontData::new(&[
        0x40, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0xC0,
        0x00, 0xC0, 0x00, 0xC0, 0x00, 0x40, 0x00, 0xC0, 0x00, 0x40, 0x00, 0x40, 0x00, 0xC0, 0x00,
        0x40, 0x00,
    ]);

    static SKIA_GVAR_I_DATA: FontData = FontData::new(&[
        0x00, 0x08, 0x00, 0x24, 0x00, 0x33, 0x20, 0x00, 0x00, 0x15, 0x20, 0x01, 0x00, 0x1B, 0x20,
        0x02, 0x00, 0x24, 0x20, 0x03, 0x00, 0x15, 0x20, 0x04, 0x00, 0x26, 0x20, 0x07, 0x00, 0x0D,
        0x20, 0x06, 0x00, 0x1A, 0x20, 0x05, 0x00, 0x40, 0x01, 0x01, 0x01, 0x81, 0x80, 0x43, 0xFF,
        0x7E, 0xFF, 0x7E, 0xFF, 0x7E, 0xFF, 0x7E, 0x00, 0x81, 0x45, 0x01, 0x01, 0x01, 0x03, 0x01,
        0x04, 0x01, 0x04, 0x01, 0x04, 0x01, 0x02, 0x80, 0x40, 0x00, 0x82, 0x81, 0x81, 0x04, 0x3A,
        0x5A, 0x3E, 0x43, 0x20, 0x81, 0x04, 0x0E, 0x40, 0x15, 0x45, 0x7C, 0x83, 0x00, 0x0D, 0x9E,
        0xF3, 0xF2, 0xF0, 0xF0, 0xF0, 0xF0, 0xF3, 0x9E, 0xA0, 0xA1, 0xA1, 0xA1, 0x9F, 0x80, 0x00,
        0x91, 0x81, 0x91, 0x00, 0x0D, 0x0A, 0x0A, 0x09, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A,
        0x0A, 0x0A, 0x0A, 0x0B, 0x80, 0x00, 0x15, 0x81, 0x81, 0x00, 0xC4, 0x89, 0x00, 0xC4, 0x83,
        0x00, 0x0D, 0x80, 0x99, 0x98, 0x96, 0x96, 0x96, 0x96, 0x99, 0x80, 0x82, 0x83, 0x83, 0x83,
        0x81, 0x80, 0x40, 0xFF, 0x18, 0x81, 0x81, 0x04, 0xE6, 0xF9, 0x10, 0x21, 0x02, 0x81, 0x04,
        0xE8, 0xE5, 0xEB, 0x4D, 0xDA, 0x83, 0x00, 0x0D, 0xCE, 0xD3, 0xD4, 0xD3, 0xD3, 0xD3, 0xD5,
        0xD2, 0xCE, 0xCC, 0xCD, 0xCD, 0xCD, 0xCD, 0x80, 0x00, 0xA1, 0x81, 0x91, 0x00, 0x0D, 0x07,
        0x03, 0x04, 0x02, 0x02, 0x02, 0x03, 0x03, 0x07, 0x07, 0x08, 0x08, 0x08, 0x07, 0x80, 0x00,
        0x09, 0x81, 0x81, 0x00, 0x28, 0x40, 0x00, 0xA4, 0x02, 0x24, 0x24, 0x66, 0x81, 0x04, 0x08,
        0xFA, 0xFA, 0xFA, 0x28, 0x83, 0x00, 0x82, 0x02, 0xFF, 0xFF, 0xFF, 0x83, 0x02, 0x01, 0x01,
        0x01, 0x84, 0x91, 0x00, 0x80, 0x06, 0x07, 0x08, 0x08, 0x08, 0x08, 0x0A, 0x07, 0x80, 0x03,
        0xFE, 0xFF, 0xFF, 0xFF, 0x81, 0x00, 0x08, 0x81, 0x82, 0x02, 0xEE, 0xEE, 0xEE, 0x8B, 0x6D,
        0x00,
    ]);

    #[test]
    fn test_shared_tuples() {
        #[allow(overflowing_literals)]
        const MINUS_ONE: F2Dot14 = F2Dot14::from_bits(0xC000);
        assert_eq!(MINUS_ONE, F2Dot14::from_f32(-1.0));

        static EXPECTED: &[(F2Dot14, F2Dot14)] = &[
            (F2Dot14::ONE, F2Dot14::ZERO),
            (MINUS_ONE, F2Dot14::ZERO),
            (F2Dot14::ZERO, F2Dot14::ONE),
            (F2Dot14::ZERO, MINUS_ONE),
            (MINUS_ONE, MINUS_ONE),
            (F2Dot14::ONE, MINUS_ONE),
            (F2Dot14::ONE, F2Dot14::ONE),
            (MINUS_ONE, F2Dot14::ONE),
        ];

        const N_AXES: u16 = 2;

        let read_args = (EXPECTED.len() as u16, N_AXES);
        let tuples =
            SharedTuples::read_with_args(SKIA_GVAR_SHARED_TUPLES_DATA, &read_args).unwrap();
        let tuple_vec: Vec<_> = tuples
            .tuples()
            .iter()
            .map(|tup| {
                let values = tup.unwrap().values();
                assert_eq!(values.len(), N_AXES as usize);
                (values[0].get(), values[1].get())
            })
            .collect();

        assert_eq!(tuple_vec, EXPECTED);
    }

    // https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6gvar.html
    #[test]
    fn smoke_test() {
        let header = GlyphVariationDataHeader::read(SKIA_GVAR_I_DATA).unwrap();
        assert_eq!(header.serialized_data_offset(), 36);
        assert_eq!(header.tuple_variation_count().count(), 8);
        let shared_tuples =
            SharedTuples::read_with_args(SKIA_GVAR_SHARED_TUPLES_DATA, &(8, 2)).unwrap();

        let vardata = GlyphVariationData::new(SKIA_GVAR_I_DATA, 2, shared_tuples).unwrap();
        assert_eq!(vardata.tuple_count(), 8);
        let deltas = vardata
            .tuples()
            .next()
            .unwrap()
            .deltas()
            .collect::<Vec<_>>();
        assert_eq!(deltas.len(), 18);
        static EXPECTED: &[(i16, i16)] = &[
            (257, 0),
            (-127, 0),
            (-128, 58),
            (-130, 90),
            (-130, 62),
            (-130, 67),
            (-130, 32),
            (-127, 0),
            (257, 0),
            (259, 14),
            (260, 64),
            (260, 21),
            (260, 69),
            (258, 124),
            (0, 0),
            (130, 0),
            (0, 0),
            (0, 0),
        ];
        let expected = EXPECTED
            .iter()
            .copied()
            .enumerate()
            .map(|(pos, (x_delta, y_delta))| GlyphDelta {
                position: pos as _,
                x_delta,
                y_delta,
            })
            .collect::<Vec<_>>();

        for (a, b) in deltas.iter().zip(expected.iter()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn vazirmatn_var() {
        use crate::test_data::test_fonts;
        let gvar = FontRef::new(test_fonts::VAZIRMATN_VAR)
            .unwrap()
            .gvar()
            .unwrap();
        let a_glyph_var = gvar.glyph_variation_data(GlyphId::new(1)).unwrap();
        assert_eq!(a_glyph_var.axis_count, 1);
        let mut tuples = a_glyph_var.tuples();
        let tup1 = tuples.next().unwrap();
        assert_eq!(tup1.peak().values(), &[F2Dot14::from_f32(-1.0)]);
        assert_eq!(tup1.deltas().count(), 18);
        let x_vals = &[
            -90, -134, 4, -6, -81, 18, -25, -33, -109, -121, -111, -111, -22, -22, 0, -113, 0, 0,
        ];
        let y_vals = &[83, 0, 0, 0, 0, 0, 83, 0, 0, 0, -50, 54, 54, -50, 0, 0, 0, 0];
        assert_eq!(tup1.deltas().map(|d| d.x_delta).collect::<Vec<_>>(), x_vals);
        assert_eq!(tup1.deltas().map(|d| d.y_delta).collect::<Vec<_>>(), y_vals);
        let tup2 = tuples.next().unwrap();
        assert_eq!(tup2.peak().values(), &[F2Dot14::from_f32(1.0)]);
        let x_vals = &[
            20, 147, -33, -53, 59, -90, 37, -6, 109, 90, -79, -79, -8, -8, 0, 59, 0, 0,
        ];
        let y_vals = &[
            -177, 0, 0, 0, 0, 0, -177, 0, 0, 0, 4, -109, -109, 4, 0, 0, 0, 0,
        ];

        assert_eq!(tup2.deltas().map(|d| d.x_delta).collect::<Vec<_>>(), x_vals);
        assert_eq!(tup2.deltas().map(|d| d.y_delta).collect::<Vec<_>>(), y_vals);
        assert!(tuples.next().is_none());

        let agrave_glyph_var = gvar.glyph_variation_data(GlyphId::new(2)).unwrap();
        let mut tuples = agrave_glyph_var.tuples();
        let tup1 = tuples.next().unwrap();
        assert_eq!(
            tup1.deltas()
                .map(|d| (d.position, d.x_delta, d.y_delta))
                .collect::<Vec<_>>(),
            &[(1, -51, 8), (3, -113, 0)]
        );
        let tup2 = tuples.next().unwrap();
        assert_eq!(
            tup2.deltas()
                .map(|d| (d.position, d.x_delta, d.y_delta))
                .collect::<Vec<_>>(),
            &[(1, -54, -1), (3, 59, 0)]
        );
        let grave_glyph_var = gvar.glyph_variation_data(GlyphId::new(3)).unwrap();
        let mut tuples = grave_glyph_var.tuples();
        let tup1 = tuples.next().unwrap();
        let tup2 = tuples.next().unwrap();
        assert!(tuples.next().is_none());
        assert_eq!(tup1.deltas().count(), 8);
        assert_eq!(
            tup2.deltas().map(|d| d.y_delta).collect::<Vec<_>>(),
            &[0, -20, -20, 0, 0, 0, 0, 0]
        );
    }
}
