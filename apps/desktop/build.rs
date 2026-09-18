use std::{fs, io, path::Path};

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        if let Err(error) = ensure_windows_icon() {
            panic!("failed to prepare Windows application icon: {error}");
        }
    }

    tauri_build::build();
}

fn ensure_windows_icon() -> io::Result<()> {
    let png_path = Path::new("icons/icon.png");
    let ico_path = Path::new("icons/icon.ico");

    let png = fs::read(png_path)?;
    if png.len() < 24 || &png[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "icons/icon.png is not a valid PNG",
        ));
    }

    let width = u32::from_be_bytes(png[16..20].try_into().expect("PNG width slice"));
    let height = u32::from_be_bytes(png[20..24].try_into().expect("PNG height slice"));
    let width_byte = if width >= 256 { 0 } else { width as u8 };
    let height_byte = if height >= 256 { 0 } else { height as u8 };

    let mut ico = Vec::with_capacity(22 + png.len());

    // ICONDIR: reserved=0, type=1 (icon), count=1.
    ico.extend_from_slice(&0_u16.to_le_bytes());
    ico.extend_from_slice(&1_u16.to_le_bytes());
    ico.extend_from_slice(&1_u16.to_le_bytes());

    // ICONDIRENTRY. PNG payloads are valid ICO image resources on modern Windows.
    ico.push(width_byte);
    ico.push(height_byte);
    ico.push(0); // palette
    ico.push(0); // reserved
    ico.extend_from_slice(&1_u16.to_le_bytes()); // color planes
    ico.extend_from_slice(&32_u16.to_le_bytes()); // bits per pixel
    ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22_u32.to_le_bytes()); // payload offset

    ico.extend_from_slice(&png);
    fs::write(ico_path, ico)
}
