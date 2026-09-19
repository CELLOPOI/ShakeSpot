//! 固定行缓冲的三倍超采样；不创建整张超采样位图，也不分配堆内存。
pub const MAX_WIDTH: usize = 192;
pub const MAX_HEIGHT: usize = 256;
const SUPERSAMPLE: usize = 3;
type Polygon = [(f64, f64); 7];

pub fn dimensions(height: usize) -> Option<(usize, usize)> {
    if !(24..=MAX_HEIGHT).contains(&height) {
        return None;
    }
    Some((
        (26.0 * (height - 4) as f64 / 35.0).ceil() as usize + 4,
        height,
    ))
}

fn transformed(vertices: Polygon, scale: f64) -> Polygon {
    vertices.map(|(x, y)| {
        (
            ((2.0 + x * scale) * 3.0).round(),
            ((2.0 + y * scale) * 3.0).round(),
        )
    })
}

fn scanline(row: &mut [u8], y: f64, vertices: &Polygon, value: u8) {
    let mut intersections = [0.0; 7];
    let mut count = 0;
    for i in 0..vertices.len() {
        let (x1, y1) = vertices[i];
        let (x2, y2) = vertices[(i + 1) % vertices.len()];
        // 半开区间使公共顶点仅计入一条边，水平边不参与相交。
        if (y1 <= y && y < y2) || (y2 <= y && y < y1) {
            intersections[count] = x1 + (y - y1) * (x2 - x1) / (y2 - y1);
            count += 1;
        }
    }
    intersections[..count].sort_unstable_by(f64::total_cmp);
    for pair in intersections[..count].chunks_exact(2) {
        let left = (pair[0] - 0.5).ceil().clamp(0.0, row.len() as f64) as usize;
        let right = (pair[1] - 0.5).ceil().clamp(0.0, row.len() as f64) as usize;
        row[left..right].fill(value);
    }
}

pub fn render(
    pixels: &mut [u32],
    height: usize,
    flip_x: bool,
    flip_y: bool,
) -> Result<(), &'static str> {
    let (width, height) = dimensions(height).ok_or("Invalid cursor dimensions.")?;
    if pixels.len() != width * height {
        return Err("Cursor buffer size mismatch.");
    }
    let scale = (height - 4) as f64 / 35.0;
    let outer = transformed(
        [
            (0.0, 0.0),
            (0.0, 29.0),
            (8.0, 22.0),
            (14.0, 35.0),
            (21.0, 31.0),
            (14.5, 19.0),
            (26.0, 19.0),
        ],
        scale,
    );
    let inner = transformed(
        [
            (1.6, 3.4),
            (1.6, 25.5),
            (8.5, 19.5),
            (14.8, 32.7),
            (18.7, 30.3),
            (11.9, 17.4),
            (21.3, 17.4),
        ],
        scale,
    );
    let mut row = [0u8; MAX_WIDTH * SUPERSAMPLE];
    for y in 0..height {
        let mut coverage = [0u16; MAX_WIDTH];
        let mut white = [0u16; MAX_WIDTH];
        for dy in 0..SUPERSAMPLE {
            let row = &mut row[..width * SUPERSAMPLE];
            row.fill(0);
            let sample_y = (y * SUPERSAMPLE + dy) as f64 + 0.5;
            scanline(row, sample_y, &outer, 1);
            scanline(row, sample_y, &inner, 2);
            for (x, samples) in row.chunks_exact(SUPERSAMPLE).enumerate() {
                for &sample in samples {
                    coverage[x] += u16::from(sample != 0);
                    white[x] += u16::from(sample == 1);
                }
            }
        }
        for x in 0..width {
            let alpha = u32::from(coverage[x]) * 255 / 9;
            let white = u32::from(white[x]) * 255 / 9;
            let tx = if flip_x { width - 1 - x } else { x };
            let ty = if flip_y { height - 1 - y } else { y };
            pixels[ty * width + tx] = (alpha << 24) | (white << 16) | (white << 8) | white;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_size_is_premultiplied_antialiased_and_mirrors_exactly() {
        for height in 24..=MAX_HEIGHT {
            let (width, _) = dimensions(height).unwrap();
            let mut normal = vec![0; width * height];
            render(&mut normal, height, false, false).unwrap();
            assert!(normal.contains(&0xff000000));
            assert!(normal.contains(&0xffffffff));
            assert!(normal.iter().any(|&p| p >> 24 > 0 && p >> 24 < 255));
            assert!(normal.iter().all(|&p| (p & 255) <= p >> 24));
            for (flip_x, flip_y) in [(true, false), (false, true), (true, true)] {
                let mut mirrored = vec![0; normal.len()];
                render(&mut mirrored, height, flip_x, flip_y).unwrap();
                for y in 0..height {
                    for x in 0..width {
                        let sx = if flip_x { width - 1 - x } else { x };
                        let sy = if flip_y { height - 1 - y } else { y };
                        assert_eq!(mirrored[y * width + x], normal[sy * width + sx]);
                    }
                }
            }
        }
    }
    #[test]
    fn invalid_size_does_not_write_outside_buffer() {
        let mut buffer = [0x12345678; 8];
        assert!(render(&mut buffer, 256, false, false).is_err());
        assert_eq!(buffer, [0x12345678; 8]);
        assert!(dimensions(0).is_none());
        assert!(dimensions(usize::MAX).is_none());
        assert_eq!(dimensions(256), Some((192, 256)));
    }
}
