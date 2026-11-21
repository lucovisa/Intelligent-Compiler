use ic_core::AtlasImage;
use std::{collections::HashMap, io::Cursor, path::Path};

const FONT: &[u8] = include_bytes!("../assets/font.ttf");
// Размеры, в которых буквы растеризуются заранее; остальные масштабируются
pub const FONT_SIZES: [u16; 6] = [14, 20, 28, 40, 56, 80];
const MAX_IMAGE_SIDE: u32 = 2048;

#[derive(Clone, Copy, Debug)]
pub struct Glyph {
    pub uv: [f32; 4],
    pub w: f32,
    pub h: f32,
    pub xmin: f32,
    pub ymin: f32,
    pub advance: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ImageInfo {
    pub uv: [f32; 4],
    pub w: u32,
    pub h: u32,
}

#[derive(Default)]
pub struct AtlasData {
    pub images: HashMap<String, ImageInfo>,
    pub glyphs: HashMap<(u16, char), Glyph>,
    // (ascent, descent) для каждого размера
    pub lines: HashMap<u16, (f32, f32)>,
}

pub struct Built {
    pub image: AtlasImage,
    pub data: AtlasData,
    pub warnings: Vec<String>,
}

struct Item {
    kind: Kind,
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

enum Kind {
    Image(String),
    Glyph(u16, char, f32, f32, f32, f32),
}

fn charset() -> Vec<char> {
    let mut v: Vec<char> = (32u32..=126).filter_map(char::from_u32).collect();
    v.extend((0x410u32..=0x44F).filter_map(char::from_u32));
    v.extend([
        'Ё', 'ё', '«', '»', '—', '–', '…', '№', '°', '±', '×', '÷', '•', '←', '↑', '→', '↓', '€',
        '£', '©',
    ]);
    v
}

pub fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let size = reader.output_buffer_size().ok_or("неверный размер PNG")?;
    let mut buf = vec![0; size];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let data = &buf[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
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

// Раскладка «полками»: возвращает позиции или None, если не влезло
fn pack(items: &[Item], side: u32) -> Option<Vec<(u32, u32)>> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|a, b| items[*b].h.cmp(&items[*a].h));
    let mut pos = vec![(0, 0); items.len()];
    // Левый верхний угол занят белым квадратом 2×2
    let (mut x, mut y, mut row) = (4u32, 0u32, 2u32);
    for i in order {
        let (w, h) = (items[i].w + 1, items[i].h + 1);
        if x + w > side {
            x = 0;
            y += row;
            row = 0;
        }
        if y + h > side {
            return None;
        }
        pos[i] = (x, y);
        x += w;
        row = row.max(h);
    }
    Some(pos)
}

pub fn build(images: &[(String, Vec<u8>)]) -> Built {
    let mut warnings = Vec::new();
    let mut items: Vec<Item> = Vec::new();
    for (name, bytes) in images {
        match decode_png(bytes) {
            Ok((w, h, rgba)) if w <= MAX_IMAGE_SIDE && h <= MAX_IMAGE_SIDE => items.push(Item {
                kind: Kind::Image(name.clone()),
                w,
                h,
                rgba,
            }),
            Ok(_) => warnings.push(format!(
                "{name}: картинка больше {MAX_IMAGE_SIDE}×{MAX_IMAGE_SIDE}"
            )),
            Err(e) => warnings.push(format!("{name}: {e}")),
        }
    }

    let mut data = AtlasData::default();
    if let Ok(font) = fontdue::Font::from_bytes(FONT, fontdue::FontSettings::default()) {
        for size in FONT_SIZES {
            if let Some(m) = font.horizontal_line_metrics(size as f32) {
                data.lines.insert(size, (m.ascent, m.descent));
            }
            for ch in charset() {
                if font.lookup_glyph_index(ch) == 0 && ch != ' ' {
                    continue;
                }
                let (m, coverage) = font.rasterize(ch, size as f32);
                let mut rgba = Vec::with_capacity(coverage.len() * 4);
                for c in &coverage {
                    rgba.extend_from_slice(&[255, 255, 255, *c]);
                }
                items.push(Item {
                    kind: Kind::Glyph(
                        size,
                        ch,
                        m.xmin as f32,
                        m.ymin as f32,
                        m.advance_width,
                        m.height as f32,
                    ),
                    w: m.width as u32,
                    h: m.height as u32,
                    rgba,
                });
            }
        }
    }

    let mut side = 512;
    let positions = loop {
        if let Some(p) = pack(&items, side) {
            break p;
        }
        side *= 2;
        if side > 4096 {
            warnings.push("картинки не помещаются в атлас 4096×4096".into());
            items.retain(|i| matches!(i.kind, Kind::Glyph(..)));
            side = 4096;
            if let Some(p) = pack(&items, side) {
                break p;
            }
            break Vec::new();
        }
    };

    let mut rgba = vec![0u8; (side * side * 4) as usize];
    for y in 0..2 {
        for x in 0..2 {
            let o = ((y * side + x) * 4) as usize;
            rgba[o..o + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
    let s = side as f32;
    for (item, (px, py)) in items.iter().zip(positions.iter()) {
        for row in 0..item.h {
            let src = (row * item.w * 4) as usize;
            let dst = (((py + row) * side + px) * 4) as usize;
            rgba[dst..dst + (item.w * 4) as usize]
                .copy_from_slice(&item.rgba[src..src + (item.w * 4) as usize]);
        }
        // Половина пикселя внутрь, чтобы фильтр не цеплял соседей
        let uv = if item.w == 0 || item.h == 0 {
            [0.0; 4]
        } else {
            [
                (*px as f32 + 0.5) / s,
                (*py as f32 + 0.5) / s,
                ((px + item.w) as f32 - 0.5) / s,
                ((py + item.h) as f32 - 0.5) / s,
            ]
        };
        match &item.kind {
            Kind::Image(name) => {
                data.images.insert(
                    name.clone(),
                    ImageInfo {
                        uv,
                        w: item.w,
                        h: item.h,
                    },
                );
            }
            Kind::Glyph(size, ch, xmin, ymin, advance, _) => {
                data.glyphs.insert(
                    (*size, *ch),
                    Glyph {
                        uv,
                        w: item.w as f32,
                        h: item.h as f32,
                        xmin: *xmin,
                        ymin: *ymin,
                        advance: *advance,
                    },
                );
            }
        }
    }
    Built {
        image: AtlasImage {
            width: side,
            height: side,
            rgba,
        },
        data,
        warnings,
    }
}

// Картинки проекта на диске (режим разработки): все *.png рядом с main.lua
pub fn collect_disk_images(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, depth: u32, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if depth < 4
                    && !matches!(
                        name.as_str(),
                        "build" | "target" | ".ic" | ".git" | "node_modules"
                    )
                {
                    walk(root, &path, depth + 1, out);
                }
            } else if name.to_ascii_lowercase().ends_with(".png") {
                if let (Ok(rel), Ok(bytes)) = (path.strip_prefix(root), std::fs::read(&path)) {
                    out.push((rel.to_string_lossy().replace('\\', "/"), bytes));
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, 0, &mut out);
    out
}

pub struct Quad {
    pub dx: f32,
    pub dy: f32,
    pub w: f32,
    pub h: f32,
    pub uv: [f32; 4],
}

// Ближайший растеризованный размер
pub fn nearest_size(size: f32) -> u16 {
    *FONT_SIZES
        .iter()
        .min_by(|a, b| {
            let da = (**a as f32 / size.max(1.0)).ln().abs();
            let db = (**b as f32 / size.max(1.0)).ln().abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(&FONT_SIZES[0])
}

// Буквы строки: смещения от начала строки (x вправо, y вверх от центра строки) и ширина
pub fn layout(data: &AtlasData, text: &str, size: f32) -> (Vec<Quad>, f32) {
    let raster = nearest_size(size);
    let k = size / raster as f32;
    let (ascent, descent) = data
        .lines
        .get(&raster)
        .copied()
        .unwrap_or((raster as f32 * 0.9, -(raster as f32) * 0.2));
    let baseline = -(ascent + descent) / 2.0 * k;
    let mut pen = 0.0;
    let mut quads = Vec::new();
    for ch in text.chars() {
        let Some(g) = data
            .glyphs
            .get(&(raster, ch))
            .or_else(|| data.glyphs.get(&(raster, '?')))
        else {
            pen += raster as f32 * 0.5 * k;
            continue;
        };
        if g.w > 0.0 && g.h > 0.0 {
            quads.push(Quad {
                dx: pen + (g.xmin + g.w / 2.0) * k,
                dy: baseline + (g.ymin + g.h / 2.0) * k,
                w: g.w * k,
                h: g.h * k,
                uv: g.uv,
            });
        }
        pen += g.advance * k;
    }
    (quads, pen)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, color: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().unwrap();
            let data: Vec<u8> = (0..w * h).flat_map(|_| color).collect();
            writer.write_image_data(&data).unwrap();
        }
        out
    }

    #[test]
    fn atlas_contains_images_glyphs_and_white_pixel() {
        let built = build(&[
            ("assets/a.png".into(), png(32, 16, [255, 0, 0, 255])),
            ("b.png".into(), png(8, 8, [0, 255, 0, 255])),
        ]);
        assert!(built.warnings.is_empty(), "{:?}", built.warnings);
        let img = built.data.images["assets/a.png"];
        assert_eq!((img.w, img.h), (32, 16));
        assert!(img.uv[0] < img.uv[2] && img.uv[1] < img.uv[3] && img.uv[2] <= 1.0);
        assert!(built.data.glyphs.contains_key(&(28, 'A')));
        assert!(built.data.glyphs.contains_key(&(28, 'Ж')));
        assert_eq!(&built.image.rgba[0..4], &[255, 255, 255, 255]);
        let side = built.image.width as f32;
        let (x, y) = ((img.uv[0] * side) as usize, (img.uv[1] * side) as usize);
        let o = (y * side as usize + x) * 4;
        assert_eq!(&built.image.rgba[o..o + 4], &[255, 0, 0, 255]);
    }

    #[test]
    fn bad_png_gives_warning_not_failure() {
        let built = build(&[("x.png".into(), b"nope".to_vec())]);
        assert_eq!(built.warnings.len(), 1);
        assert!(built.data.glyphs.contains_key(&(14, 'a')));
    }

    #[test]
    fn text_layout_scales_with_size() {
        let built = build(&[]);
        let (q1, w1) = layout(&built.data, "Hello", 20.0);
        let (_, w2) = layout(&built.data, "Hello", 40.0);
        assert_eq!(q1.len(), 5);
        assert!((w2 / w1 - 2.0).abs() < 0.1, "{w1} {w2}");
        let (spaces, w) = layout(&built.data, "a b", 28.0);
        assert_eq!(spaces.len(), 2);
        assert!(w > 0.0);
        assert!(layout(&built.data, "Привет", 28.0).0.len() == 6);
    }

    #[test]
    fn nearest_size_picks_closest() {
        assert_eq!(nearest_size(15.0), 14);
        assert_eq!(nearest_size(100.0), 80);
        assert_eq!(nearest_size(30.0), 28);
    }
}
