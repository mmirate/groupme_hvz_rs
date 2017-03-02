use std;
//use sdl2;
use image;
//use sdl2_ttf;
use rusttype;
use errors::*;
use image::Pixel;

pub type PngData = Vec<u8>;

fn chunkify(s: String, len: usize) -> Vec<String> {
    let mut ret = vec![];
    let mut buf = String::new();
    for line in s.lines() {
        for word in line.split_whitespace() {
            if buf.len() + word.len() > len - 2 {
                ret.push(std::mem::replace(&mut buf, String::new()));
                ret.push("\n".to_string());
            }
            buf += word;
            buf += " ";
        }
        ret.push(std::mem::replace(&mut buf, String::new()));
        ret.push("\n".to_string());
    }
    if !buf.is_empty() {
        ret.push(buf);
    }
    ret
}

// TODO 2-layer chunkify

fn luma_to_lumaa(src: image::ImageBuffer<image::Luma<u8>, Vec<u8>>) -> image::ImageBuffer<image::LumaA<u8>, Vec<u8>> {
    //image::ImageBuffer::<image::LumaA<u8>, Vec<u8>>::from_raw(src.width(), src.height(), src.iter().cloned().flat_map(|p| vec![p, 0u8]).collect::<Vec<_>>()).unwrap()
    image::ImageBuffer::from_fn(src.width(), src.height(), |x, y| {
        let mut p = src.get_pixel(x,y).to_luma_alpha();
        p.channels_mut()[1] = !p.channels()[0];//.saturating_sub(32);
        //p.channels_mut()[1] = 255u8;
        p
    })
    //image::ImageBuffer::<image::LumaA<u8>, Vec<u8>>::from_vec(src.width(), src.height(), src.into_vec().into_iter().flat_map(|p| vec![p, 0u8]).collect()).unwrap()
}

lazy_static!{
    static ref FONT: rusttype::Font<'static> = {
        let font_data = include_bytes!("../Anonymous Pro.ttf");
        let collection = rusttype::FontCollection::from_bytes(font_data as &[u8]);
        collection.into_font().unwrap() // only succeeds if collection consists of one font
    };
}

fn render_one_line(input: &str) -> image::ImageBuffer<image::Luma<u8>, Vec<u8>> {

    // Desired font pixel height
    let height: f32 = 14.66666666;
    let pixel_height = height.ceil() as usize;

    let scale = rusttype::Scale { x: height, y: height };

    // The origin of a line of text is at the baseline (roughly where non-descending letters sit).
    // We don't want to clip the text, so we shift it down with an offset when laying it out.
    // v_metrics.ascent is the distance between the baseline and the highest edge of any glyph in
    // the font. That's enough to guarantee that there's no clipping.
    let v_metrics = FONT.v_metrics(scale);
    let offset = rusttype::point(0.0, v_metrics.ascent);

    let glyphs: Vec<rusttype::PositionedGlyph> = FONT.layout(input, scale, offset).collect();

    // Find the most visually pleasing width to display
    let width = glyphs.iter().rev()
        .filter_map(|g| g.pixel_bounding_box()
                    .map(|b| b.min.x as f32 + g.unpositioned().h_metrics().advance_width))
        .next().unwrap_or(0.0).ceil() as usize;

    println!("width: {}, height: {}", width, pixel_height);

    let mut pixel_data = vec![255u8; width * pixel_height];
    for g in glyphs {
        if let Some(bb) = g.pixel_bounding_box() {
            g.draw(|x, y, v| {
                let c = !((v * 250.0).ceil() as u8);
                let x = x as i32 + bb.min.x;
                let y = y as i32 + bb.min.y;
                if x >= 0 && x < width as i32 && y >= 0 && y < pixel_height as i32 {
                    let x = x as usize;
                    let y = y as usize;
                    pixel_data[(x + y * width)] = c;
                }
            })
        }
    }

    return image::ImageBuffer::from_vec(width as u32, pixel_height as u32, pixel_data).expect("math error");
}

pub fn render(input: String) -> Result<PngData> {
    let input = chunkify(input, 80).concat();
    let rendered_lines = input.trim().lines().map(str::trim).map(render_one_line).collect::<Vec<_>>();
    let (ws, hs) : (Vec<_>, Vec<_>) = rendered_lines.iter().map(|ib| (ib.width(), ib.height())).unzip();
    let (w, h, lh): (u32, u32, u32) = (*ws.iter().max().unwrap(), hs.iter().sum(), *hs.iter().max().unwrap());
    let mut image = image::ImageBuffer::from_pixel(w, h, image::LumaA::<u8> { data: [255u8, 0u8] });
    for (i, line) in rendered_lines.into_iter().enumerate() {
        image::imageops::overlay(&mut image, &luma_to_lumaa(line), 0, (i as u32) * (lh as u32));
    }
    let mut png_buffer = Vec::<u8>::new();
    try!(image::png::PNGEncoder::new(&mut png_buffer).encode(&image, image.width(), image.height(), image::GrayA(8)));
    Ok(png_buffer)
}


