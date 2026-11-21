use std::path::Path;

const SIZES: [u32; 6] = [256, 128, 64, 48, 32, 16];

// Значок в нескольких размерах: (сторона, PNG)
pub struct IconSet {
    pub sizes: Vec<(u32, Vec<u8>)>,
}

fn decode(png_bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let size = reader.output_buffer_size().ok_or("неверный PNG")?;
    let mut buf = vec![0; size];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let data = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks(2)
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::Indexed => return Err("PNG с палитрой не поддержан".into()),
    };
    Ok((info.width, info.height, rgba))
}

fn encode(side: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, side, side);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(rgba).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

// Картинка → квадрат (прозрачные поля)
fn square(w: u32, h: u32, rgba: &[u8]) -> (u32, Vec<u8>) {
    let side = w.max(h);
    let mut out = vec![0u8; (side * side * 4) as usize];
    let (ox, oy) = ((side - w) / 2, (side - h) / 2);
    for y in 0..h {
        let src = (y * w * 4) as usize;
        let dst = (((y + oy) * side + ox) * 4) as usize;
        out[dst..dst + (w * 4) as usize].copy_from_slice(&rgba[src..src + (w * 4) as usize]);
    }
    (side, out)
}

// Уменьшение усреднением блока, увеличение — ближайший пиксель
fn resize(side: u32, rgba: &[u8], target: u32) -> Vec<u8> {
    let mut out = vec![0u8; (target * target * 4) as usize];
    for y in 0..target {
        for x in 0..target {
            let (x0, x1) = (
                x * side / target,
                ((x + 1) * side)
                    .div_ceil(target)
                    .min(side)
                    .max(x * side / target + 1),
            );
            let (y0, y1) = (
                y * side / target,
                ((y + 1) * side)
                    .div_ceil(target)
                    .min(side)
                    .max(y * side / target + 1),
            );
            let (mut acc, mut n, mut a_sum) = ([0.0f32; 3], 0.0f32, 0.0f32);
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let o = ((sy * side + sx) * 4) as usize;
                    let a = rgba[o + 3] as f32 / 255.0;
                    for c in 0..3 {
                        acc[c] += rgba[o + c] as f32 * a;
                    }
                    a_sum += a;
                    n += 1.0;
                }
            }
            let d = ((y * target + x) * 4) as usize;
            if a_sum > 0.0 {
                for c in 0..3 {
                    out[d + c] = (acc[c] / a_sum).round() as u8;
                }
            }
            out[d + 3] = (a_sum / n * 255.0).round() as u8;
        }
    }
    out
}

impl IconSet {
    pub fn from_png(png_bytes: &[u8]) -> Result<Self, String> {
        let (w, h, rgba) = decode(png_bytes)?;
        if w == 0 || h == 0 {
            return Err("пустая картинка".into());
        }
        let (side, square_rgba) = square(w, h, &rgba);
        let mut sizes = Vec::new();
        for target in SIZES {
            let pixels = if side == target {
                square_rgba.clone()
            } else {
                resize(side, &square_rgba, target)
            };
            sizes.push((target, encode(target, &pixels)?));
        }
        Ok(Self { sizes })
    }

    // Файл .ico с PNG внутри
    pub fn to_ico(&self) -> Vec<u8> {
        let mut out = vec![0, 0, 1, 0];
        out.extend((self.sizes.len() as u16).to_le_bytes());
        let mut offset = 6 + 16 * self.sizes.len() as u32;
        for (side, data) in &self.sizes {
            let b = if *side >= 256 { 0 } else { *side as u8 };
            out.extend([b, b, 0, 0, 1, 0, 32, 0]);
            out.extend((data.len() as u32).to_le_bytes());
            out.extend(offset.to_le_bytes());
            offset += data.len() as u32;
        }
        for (_, data) in &self.sizes {
            out.extend(data);
        }
        out
    }

    // Ресурс RT_GROUP_ICON: записи ссылаются на RT_ICON с номерами 1..n
    pub fn to_group(&self) -> Vec<u8> {
        let mut out = vec![0, 0, 1, 0];
        out.extend((self.sizes.len() as u16).to_le_bytes());
        for (i, (side, data)) in self.sizes.iter().enumerate() {
            let b = if *side >= 256 { 0 } else { *side as u8 };
            out.extend([b, b, 0, 0, 1, 0, 32, 0]);
            out.extend((data.len() as u32).to_le_bytes());
            out.extend(((i + 1) as u16).to_le_bytes());
        }
        out
    }
}

// Подмена значка в готовом exe (только Windows): ресурсы правятся до дописывания данных игры
#[cfg(any(windows, ic_winapi_check))]
pub fn set_exe_icon(exe: &Path, set: &IconSet) -> Result<(), String> {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    extern "system" {
        fn BeginUpdateResourceW(file: *const u16, delete_existing: i32) -> *mut c_void;
        fn UpdateResourceW(
            update: *mut c_void,
            kind: *const u16,
            name: *const u16,
            language: u16,
            data: *const c_void,
            size: u32,
        ) -> i32;
        fn EndUpdateResourceW(update: *mut c_void, discard: i32) -> i32;
    }
    const RT_ICON: usize = 3;
    const RT_GROUP_ICON: usize = 14;
    const LANG_EN_US: u16 = 0x0409;
    let wide: Vec<u16> = exe
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    unsafe {
        let handle = BeginUpdateResourceW(wide.as_ptr(), 0);
        if handle.is_null() {
            return Err("не удалось открыть exe для правки ресурсов".into());
        }
        for (i, (_, data)) in set.sizes.iter().enumerate() {
            let ok = UpdateResourceW(
                handle,
                RT_ICON as *const u16,
                (i + 1) as *const u16,
                LANG_EN_US,
                data.as_ptr() as *const c_void,
                data.len() as u32,
            );
            if ok == 0 {
                EndUpdateResourceW(handle, 1);
                return Err("не удалось записать значок".into());
            }
        }
        let group = set.to_group();
        let ok = UpdateResourceW(
            handle,
            RT_GROUP_ICON as *const u16,
            1 as *const u16,
            LANG_EN_US,
            group.as_ptr() as *const c_void,
            group.len() as u32,
        );
        if ok == 0 || EndUpdateResourceW(handle, 0) == 0 {
            return Err("не удалось сохранить значок в exe".into());
        }
    }
    Ok(())
}

#[cfg(not(any(windows, ic_winapi_check)))]
pub fn set_exe_icon(_exe: &Path, _set: &IconSet) -> Result<(), String> {
    Err("значок exe можно встроить только при сборке на Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_png(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut wr = enc.write_header().unwrap();
            let data: Vec<u8> = (0..w * h)
                .flat_map(|i| [(i % 255) as u8, 80, 200, 255])
                .collect();
            wr.write_image_data(&data).unwrap();
        }
        out
    }

    #[test]
    fn builds_six_sizes_with_png_payloads() {
        let set = IconSet::from_png(&sample_png(100, 60)).unwrap();
        assert_eq!(
            set.sizes.iter().map(|s| s.0).collect::<Vec<_>>(),
            vec![256, 128, 64, 48, 32, 16]
        );
        for (_, data) in &set.sizes {
            assert_eq!(&data[..8], b"\x89PNG\r\n\x1a\n");
        }
    }

    #[test]
    fn ico_header_and_offsets_are_consistent() {
        let set = IconSet::from_png(&sample_png(64, 64)).unwrap();
        let ico = set.to_ico();
        assert_eq!(&ico[..6], &[0, 0, 1, 0, 6, 0]);
        let first_offset = u32::from_le_bytes(ico[18..22].try_into().unwrap()) as usize;
        assert_eq!(first_offset, 6 + 16 * 6);
        assert_eq!(&ico[first_offset..first_offset + 4], b"\x89PNG");
        assert_eq!(ico[6], 0, "256 px кодируется нулём");
    }

    #[test]
    fn group_resource_points_to_numbered_icons() {
        let set = IconSet::from_png(&sample_png(32, 32)).unwrap();
        let group = set.to_group();
        assert_eq!(group.len(), 6 + 14 * 6);
        let id_of_first = u16::from_le_bytes(group[6 + 12..6 + 14].try_into().unwrap());
        assert_eq!(id_of_first, 1);
        let id_of_last =
            u16::from_le_bytes(group[6 + 14 * 5 + 12..6 + 14 * 5 + 14].try_into().unwrap());
        assert_eq!(id_of_last, 6);
    }

    #[test]
    fn rejects_garbage() {
        assert!(IconSet::from_png(b"not a png").is_err());
    }
}
