/// Returns a neutral pixel for the original symbol areas in the affected wall
/// pages. Coordinates follow the source texture's 64-by-64 pixel grid.
pub fn wall_pixel(show: bool, page: usize, x: usize, y: usize, original: u8) -> u8 {
    if show {
        return original;
    }

    // The Hitler portraits in WL1 VSWAP pages 6/7 and 20/21 have a tiny
    // swastika inside the light armband roundel. Replace its dark strokes
    // with the roundel's light gray, keeping the band and portrait intact.
    if matches!(page, 6 | 7 | 20 | 21)
        && (34..42).contains(&x)
        && (41..50).contains(&y)
        && matches!(original, 0x1e | 0x1b)
    {
        return 0x11;
    }

    // The bust mural variants on pages 18/19 place the same small symbol on
    // the lower pedestal. Replace its dark strokes while preserving the base.
    if matches!(page, 18 | 19)
        && (29..34).contains(&x)
        && (44..51).contains(&y)
        && matches!(original, 0x00 | 0x2e)
    {
        return if page == 18 { 0x25 } else { 0x2a };
    }

    let (center_y, radius_squared, fill) = match page {
        // The roundel on the Reich flag keeps its outer ring and cloth.
        4 => (32isize, 100isize, 0x17),
        5 => (32, 100, 0x1b),
        // The small roundel beneath the eagle keeps the surrounding wall art.
        10 => (50, 90, 0x00),
        11 => (50, 90, 0x00),
        // The central emblem in the gold wreath keeps the wreath and bricks.
        34 => (32, 144, 0x1e),
        35 => (32, 144, 0x1f),
        _ => return original,
    };
    let dx = x as isize - 32;
    let dy = y as isize - center_y;
    if dx * dx + dy * dy <= radius_squared {
        fill
    } else {
        original
    }
}
