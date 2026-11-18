use std::error::Error;

use crate::vga::{BitmapFont, Picture, VgaArchive};

#[derive(Clone, Debug, Default)]
pub struct EndingPage {
    ops: Vec<DrawOp>,
    layout_delay_tics: usize,
}

#[derive(Clone, Debug)]
pub(crate) enum DrawOp {
    Text {
        x: usize,
        y: usize,
        text: String,
        color: u8,
        show_after_tics: usize,
    },
    Picture {
        x: usize,
        y: usize,
        picture: Picture,
        show_after_tics: usize,
    },
    Bar {
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        color: u8,
        show_after_tics: usize,
    },
}

pub fn parse(
    article: &[u8],
    archive: &VgaArchive,
    font: &BitmapFont,
) -> Result<Vec<EndingPage>, Box<dyn Error>> {
    const TOP: usize = 16;
    const ROW_HEIGHT: usize = 10;
    const ROWS: usize = 15;
    const LEFT: usize = 16;
    const RIGHT: usize = 304;
    const PICTURE_GAP: usize = 8;

    let mut pages = Vec::new();
    let mut page = None;
    let mut left = [LEFT; ROWS];
    let mut right = [RIGHT; ROWS];
    let (mut x, mut y, mut row, mut color) = (LEFT, TOP, 0usize, 0u8);
    let mut cursor = 0usize;
    let mut timing_tics = 0usize;
    while cursor < article.len() {
        if article[cursor] == 0 {
            break;
        }
        if article[cursor] == b'^' {
            let command = article
                .get(cursor + 1)
                .copied()
                .unwrap_or_default()
                .to_ascii_uppercase();
            cursor = (cursor + 2).min(article.len());
            match command {
                b'P' => {
                    if let Some(old_page) = page.take() {
                        pages.push(old_page);
                    }
                    page = Some(EndingPage::default());
                    left.fill(LEFT);
                    right.fill(RIGHT);
                    (x, y, row, color) = (LEFT, TOP, 0, 0);
                    timing_tics = 0;
                    skip_line(article, &mut cursor);
                }
                b'E' => {
                    if let Some(last_page) = page.take() {
                        pages.push(last_page);
                    }
                    break;
                }
                b';' => skip_line(article, &mut cursor),
                b'C' => {
                    let first = article.get(cursor).copied().unwrap_or(b'0');
                    let second = article.get(cursor + 1).copied().unwrap_or(b'0');
                    color = (hex_digit(first) << 4) | hex_digit(second);
                    cursor = (cursor + 2).min(article.len());
                }
                b'>' => x = 160,
                b'L' => {
                    let numbers = command_numbers(article, &mut cursor, 2);
                    if numbers.len() == 2 {
                        row = numbers[0].saturating_sub(TOP) / ROW_HEIGHT;
                        row = row.min(ROWS - 1);
                        y = TOP + row * ROW_HEIGHT;
                        x = numbers[1];
                    }
                    skip_line(article, &mut cursor);
                }
                b'B' => {
                    let numbers = command_numbers(article, &mut cursor, 4);
                    if numbers.len() == 4 {
                        if let Some(page) = page.as_mut() {
                            page.ops.push(DrawOp::Bar {
                                x: numbers[1],
                                y: numbers[0],
                                width: numbers[2],
                                height: numbers[3],
                                color: 0x11,
                                show_after_tics: timing_tics,
                            });
                        }
                    }
                    skip_line(article, &mut cursor);
                }
                b'G' | b'T' => {
                    let numbers =
                        command_numbers(article, &mut cursor, if command == b'T' { 4 } else { 3 });
                    if numbers.len() >= 3 {
                        let (picture_y, picture_x, picture_id) =
                            (numbers[0], numbers[1] & !7, numbers[2]);
                        let delay_tics = if command == b'T' {
                            numbers.get(3).copied().unwrap_or_default()
                        } else {
                            0
                        };
                        if let Some(picture) = archive.picture(picture_id)? {
                            if let Some(page) = page.as_mut() {
                                page.ops.push(DrawOp::Picture {
                                    x: picture_x,
                                    y: picture_y,
                                    picture: picture.clone(),
                                    show_after_tics: timing_tics.saturating_add(delay_tics),
                                });
                            }
                            if command == b'G' {
                                let midpoint = picture_x.saturating_add(picture.width / 2);
                                let margin = if midpoint > 160 {
                                    picture_x.saturating_sub(PICTURE_GAP)
                                } else {
                                    picture_x
                                        .saturating_add(picture.width)
                                        .saturating_add(PICTURE_GAP)
                                };
                                let top = picture_y.saturating_sub(TOP) / ROW_HEIGHT;
                                let bottom =
                                    picture_y.saturating_add(picture.height).saturating_sub(TOP)
                                        / ROW_HEIGHT;
                                for affected_row in top.min(ROWS - 1)..=bottom.min(ROWS - 1) {
                                    if midpoint > 160 {
                                        right[affected_row] = margin;
                                    } else {
                                        left[affected_row] = margin;
                                    }
                                }
                                if x < left[row] {
                                    x = left[row];
                                }
                            }
                        }
                        timing_tics = timing_tics.saturating_add(delay_tics);
                        if let Some(page) = page.as_mut() {
                            page.layout_delay_tics = timing_tics;
                        }
                    }
                    skip_line(article, &mut cursor);
                }
                _ => skip_line(article, &mut cursor),
            }
            continue;
        }

        let byte = article[cursor];
        if byte == b'\n' {
            cursor += 1;
            row = (row + 1).min(ROWS - 1);
            y = TOP + row * ROW_HEIGHT;
            x = left[row];
        } else if byte == b'\t' {
            cursor += 1;
            x = (x + 8) & !7;
        } else if byte <= 32 {
            cursor += 1;
        } else {
            let start = cursor;
            while article
                .get(cursor)
                .is_some_and(|byte| *byte > 32 && *byte != b'^')
            {
                cursor += 1;
            }
            let word = String::from_utf8_lossy(&article[start..cursor]).into_owned();
            let width = font.text_width(&word);
            while x.saturating_add(width) > right[row] && row + 1 < ROWS {
                row += 1;
                y = TOP + row * ROW_HEIGHT;
                x = left[row];
            }
            if let Some(page) = page.as_mut() {
                page.ops.push(DrawOp::Text {
                    x,
                    y,
                    text: word,
                    color,
                    show_after_tics: timing_tics,
                });
            }
            x = x.saturating_add(width);
            while article.get(cursor) == Some(&b' ') {
                // WL_TEXT.C:HandleWord advances SPACEWIDTH for each space
                // after a word. Leading spaces go through HandleCtrls and do
                // not move the DOS cursor.
                cursor += 1;
                x = x.saturating_add(7);
            }
        }
    }
    Ok(pages)
}

fn command_numbers(article: &[u8], cursor: &mut usize, maximum: usize) -> Vec<usize> {
    let mut numbers = Vec::with_capacity(maximum);
    while *cursor < article.len() && numbers.len() < maximum {
        while article
            .get(*cursor)
            .is_some_and(|byte| !byte.is_ascii_digit() && *byte != b'\n')
        {
            *cursor += 1;
        }
        let start = *cursor;
        while article
            .get(*cursor)
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            *cursor += 1;
        }
        if start < *cursor {
            let value = std::str::from_utf8(&article[start..*cursor])
                .ok()
                .and_then(|value| value.parse().ok());
            if let Some(value) = value {
                numbers.push(value);
            }
        } else {
            break;
        }
    }
    numbers
}

fn skip_line(article: &[u8], cursor: &mut usize) {
    while *cursor < article.len() && article[*cursor] != b'\n' {
        *cursor += 1;
    }
    *cursor = (*cursor + 1).min(article.len());
}

fn hex_digit(byte: u8) -> u8 {
    match byte.to_ascii_uppercase() {
        b'0'..=b'9' => byte - b'0',
        b'A'..=b'F' => byte.to_ascii_uppercase() - b'A' + 10,
        _ => 0,
    }
}

impl EndingPage {
    pub fn render(
        &self,
        pixels: &mut [u32],
        font: &BitmapFont,
        frame: &[Picture; 4],
        page_number: usize,
        page_count: usize,
        elapsed_tics: f64,
    ) {
        crate::world::render_ending_page(
            pixels,
            font,
            frame,
            self,
            page_number,
            page_count,
            elapsed_tics,
        );
    }

    pub(crate) fn ops(&self) -> &[DrawOp] {
        &self.ops
    }

    pub(crate) fn layout_delay_tics(&self) -> usize {
        self.layout_delay_tics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_numbers_skip_separators_and_stop_at_newline() {
        let bytes = b" 12, 34, 56\nnext";
        let mut cursor = 0;
        assert_eq!(command_numbers(bytes, &mut cursor, 3), vec![12, 34, 56]);
        assert_eq!(bytes[cursor], b'\n');
    }

    #[test]
    fn hexadecimal_colors_are_decoded() {
        assert_eq!(hex_digit(b'F'), 15);
        assert_eq!(hex_digit(b'a'), 10);
        assert_eq!(hex_digit(b'?'), 0);
    }
}
