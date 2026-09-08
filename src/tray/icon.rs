//! Tray images. macOS gets a template image (black strokes on transparency, so the system
//! tints it for light and dark menu bars): the Lucide "arrow-right-left" mark, pre-rendered at
//! 2x. Windows and Linux trays are full color, so they show the app icon.

use tray_icon::Icon;

#[cfg(target_os = "macos")]
const TRAY_PNG: &[u8] = include_bytes!("../../assets/tray-macos@2x.png");
#[cfg(not(target_os = "macos"))]
const TRAY_PNG: &[u8] = include_bytes!("../../assets/tray-32.png");

pub fn tray_icon() -> Icon {
    let (rgba, w, h) = decode(TRAY_PNG).expect("embedded tray icon is a valid PNG");
    Icon::from_rgba(rgba, w, h).expect("valid icon buffer")
}

fn decode(png: &[u8]) -> Result<(Vec<u8>, u32, u32), image::ImageError> {
    let img = image::load_from_memory(png)?.into_rgba8();
    let (w, h) = img.dimensions();
    Ok((img.into_raw(), w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_icons_decode_with_transparency() {
        for png in [
            &include_bytes!("../../assets/tray-macos@2x.png")[..],
            &include_bytes!("../../assets/tray-macos.png")[..],
            &include_bytes!("../../assets/tray-32.png")[..],
        ] {
            let (rgba, w, h) = decode(png).unwrap();
            assert_eq!(rgba.len(), (w * h * 4) as usize);
            assert!(w >= 22 && h >= 22);
        }
        // The template must have real transparency around the strokes.
        let (rgba, _, _) = decode(include_bytes!("../../assets/tray-macos@2x.png")).unwrap();
        let transparent = rgba.chunks(4).filter(|p| p[3] == 0).count();
        let opaque = rgba.chunks(4).filter(|p| p[3] > 200 && p[0] < 30).count();
        assert!(transparent > opaque && opaque > 50, "transparent={transparent} opaque={opaque}");
    }
}
