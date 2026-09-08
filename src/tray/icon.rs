//! The tray glyph is drawn in code (two opposing arrows), so no image assets are needed and
//! the macOS template rendering gets a clean alpha mask.

use tray_icon::Icon;

pub fn tray_icon() -> Icon {
    let size: u32 = if cfg!(target_os = "macos") { 44 } else { 32 };
    let rgba = draw(size);
    Icon::from_rgba(rgba, size, size).expect("valid icon buffer")
}

/// Black glyph on transparent background: an upper arrow pointing right, a lower one pointing left.
fn draw(size: u32) -> Vec<u8> {
    let mut px = vec![0u8; (size * size * 4) as usize];
    let s = size as f32;
    let stroke = (s * 0.14).max(2.0);
    let mut put = |x: u32, y: u32| {
        if x < size && y < size {
            let i = ((y * size + x) * 4) as usize;
            px[i] = 0;
            px[i + 1] = 0;
            px[i + 2] = 0;
            px[i + 3] = 255;
        }
    };
    let arrow = |put: &mut dyn FnMut(u32, u32), cy: f32, to_right: bool| {
        let x0 = s * 0.14;
        let x1 = s * 0.86;
        for y in 0..size {
            let fy = y as f32 + 0.5;
            if (fy - cy).abs() > stroke / 2.0 {
                continue;
            }
            for x in 0..size {
                let fx = x as f32 + 0.5;
                if fx >= x0 && fx <= x1 {
                    put(x, y);
                }
            }
        }
        // arrow head: a triangle of height ~0.3s at the pointing end
        let head = s * 0.22;
        for y in 0..size {
            let fy = y as f32 + 0.5;
            let dy = (fy - cy).abs();
            if dy > head {
                continue;
            }
            let depth = head - dy; // how far the head extends inward at this row
            for x in 0..size {
                let fx = x as f32 + 0.5;
                let inside = if to_right { fx <= x1 && fx >= x1 - depth } else { fx >= x0 && fx <= x0 + depth };
                if inside {
                    put(x, y);
                }
            }
        }
    };
    arrow(&mut put, s * 0.32, true);
    arrow(&mut put, s * 0.68, false);
    px
}

#[cfg(test)]
mod tests {
    #[test]
    fn icon_has_opaque_pixels() {
        let px = super::draw(32);
        assert!(px.chunks(4).filter(|p| p[3] == 255).count() > 100);
        assert!(px.chunks(4).filter(|p| p[3] == 0).count() > 100);
    }
}
