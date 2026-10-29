pub fn assert_raster_matches(actual: &image::RgbImage, expected: &image::RgbImage, label: &str) {
    assert_eq!(actual.dimensions(), expected.dimensions(), "{label}");
    let mut error = 0_u64;
    let mut samples = 0_u64;
    // Independent rasterizers can place a stroked edge one pixel apart.
    // Compare colors symmetrically within that neighborhood, so missing or
    // additional regions wider than one pixel still contribute their error.
    for (first, second) in [(actual, expected), (expected, actual)] {
        for y in 1..first.height() - 1 {
            for x in 1..first.width() - 1 {
                if (y - 1..=y + 1)
                    .any(|y| (x - 1..=x + 1).any(|x| first.get_pixel(x, y).0 == [255; 3]))
                {
                    continue;
                }
                let best = (y - 1..=y + 1)
                    .flat_map(|y| (x - 1..=x + 1).map(move |x| (x, y)))
                    .map(|(xx, yy)| {
                        (0..3)
                            .map(|channel| {
                                u64::from(
                                    first.get_pixel(x, y)[channel]
                                        .abs_diff(second.get_pixel(xx, yy)[channel]),
                                )
                            })
                            .sum::<u64>()
                    })
                    .min()
                    .unwrap();
                error += best;
                samples += 3;
            }
        }
    }
    let mean = error as f64 / samples as f64;
    assert!(
        samples > 100 && mean < 4.,
        "{label}: mean color error {mean} over {samples} samples"
    );
    // Color agreement inside the shape alone would miss a lost clip that
    // paints outside it. Check reference-white interiors independently.
    let mut extra_ink = 0_usize;
    for y in 1..expected.height() - 1 {
        for x in 1..expected.width() - 1 {
            if (y - 1..=y + 1)
                .all(|y| (x - 1..=x + 1).all(|x| expected.get_pixel(x, y).0 == [255; 3]))
            {
                extra_ink += usize::from(
                    actual
                        .get_pixel(x, y)
                        .0
                        .iter()
                        .any(|channel| *channel < 248),
                );
            }
        }
    }
    let fraction = extra_ink as f64 / f64::from(expected.width() * expected.height());
    assert!(
        fraction < 0.005,
        "{label}: unexpected paint over {extra_ink} white interior pixels ({fraction})"
    );
}
