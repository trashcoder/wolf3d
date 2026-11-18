use crate::{
    settings::VideoFilter,
    world::{HEIGHT, WIDTH},
};

pub const SCALE: usize = 4;
pub const OUTPUT_WIDTH: usize = WIDTH * SCALE;
pub const OUTPUT_HEIGHT: usize = HEIGHT * SCALE;

pub fn process_frame(source: &[u32], filter: VideoFilter) -> Vec<u32> {
    if source.len() != WIDTH * HEIGHT {
        return vec![0xff00_0000; OUTPUT_WIDTH * OUTPUT_HEIGHT];
    }

    match filter {
        VideoFilter::Classic => nearest(source),
        VideoFilter::Hqx => {
            let mut output = vec![0; OUTPUT_WIDTH * OUTPUT_HEIGHT];
            hqx::hq4x(source, &mut output, WIDTH as u32, HEIGHT as u32);
            output
        }
        VideoFilter::Xbrz => xbrz(source),
        VideoFilter::Crt => crt(source),
    }
}

fn nearest(source: &[u32]) -> Vec<u32> {
    let mut output = vec![0; OUTPUT_WIDTH * OUTPUT_HEIGHT];
    for y in 0..OUTPUT_HEIGHT {
        let source_row = (y / SCALE) * WIDTH;
        let output_row = y * OUTPUT_WIDTH;
        for x in 0..OUTPUT_WIDTH {
            output[output_row + x] = source[source_row + x / SCALE];
        }
    }
    output
}

fn xbrz(source: &[u32]) -> Vec<u32> {
    let mut rgba = Vec::with_capacity(source.len() * 4);
    for pixel in source {
        rgba.push((pixel >> 16) as u8);
        rgba.push((pixel >> 8) as u8);
        rgba.push(*pixel as u8);
        rgba.push((pixel >> 24) as u8);
    }
    let scaled = xbrz::scale_rgba(&rgba, WIDTH, HEIGHT, SCALE);
    scaled
        .chunks_exact(4)
        .map(|pixel| {
            (u32::from(pixel[3]) << 24)
                | (u32::from(pixel[0]) << 16)
                | (u32::from(pixel[1]) << 8)
                | u32::from(pixel[2])
        })
        .collect()
}

fn crt(source: &[u32]) -> Vec<u32> {
    let mut output = nearest(source);
    for y in 0..OUTPUT_HEIGHT {
        let scanline_gain: u32 = if y % SCALE == SCALE - 1 { 0x9a } else { 0xff };
        let row = y * OUTPUT_WIDTH;
        for x in 0..OUTPUT_WIDTH {
            let pixel = output[row + x];
            let phosphor = x % 3;
            let mask: [u32; 3] = [
                if phosphor == 0 { 0xff } else { 0xd0 },
                if phosphor == 1 { 0xff } else { 0xd0 },
                if phosphor == 2 { 0xff } else { 0xd0 },
            ];
            let red = (((pixel >> 16) & 0xff) * mask[0] * scanline_gain / (0xff * 0xff)) & 0xff;
            let green = (((pixel >> 8) & 0xff) * mask[1] * scanline_gain / (0xff * 0xff)) & 0xff;
            let blue = ((pixel & 0xff) * mask[2] * scanline_gain / (0xff * 0xff)) & 0xff;
            output[row + x] = 0xff00_0000 | (red << 16) | (green << 8) | blue;
        }
    }
    output
}
