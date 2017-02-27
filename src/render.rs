use std;
use sdl2;
use image;
use sdl2_ttf;
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

fn solid_to_luma(surface: &mut sdl2::surface::Surface) -> Result<image::ImageBuffer<image::Luma<u8>, Vec<u8>>> {
    let (p, w, h) = (surface.pitch(), surface.width(), surface.height());
    let mut luma_buffer = try!(surface.without_lock_mut().ok_or(ErrorKind::RLE)).to_owned();
    for p in luma_buffer.iter_mut() {
        //*p = !p.wrapping_sub(1); // 0 => BG => 0; 1 => FG => 255
        *p = p.wrapping_sub(1); // 0 => BG => 255; 1 => FG => 0
    }
    Ok(image::imageops::crop(&mut image::ImageBuffer::from_raw(p, h, luma_buffer).unwrap(), 0, 0, w, h).to_image())
}

fn luma_to_lumaa(src: image::ImageBuffer<image::Luma<u8>, Vec<u8>>) -> image::ImageBuffer<image::LumaA<u8>, Vec<u8>> {
    //image::ImageBuffer::<image::LumaA<u8>, Vec<u8>>::from_raw(src.width(), src.height(), src.iter().cloned().flat_map(|p| vec![p, 0u8]).collect::<Vec<_>>()).unwrap()
    image::ImageBuffer::from_fn(src.width(), src.height(), |x, y| {
        let mut p = src.get_pixel(x,y).to_luma_alpha();
        p.channels_mut()[1] = p.channels()[0];//.saturating_sub(32);
        p
    })
    //image::ImageBuffer::<image::LumaA<u8>, Vec<u8>>::from_vec(src.width(), src.height(), src.into_vec().into_iter().flat_map(|p| vec![p, 0u8]).collect()).unwrap()
}

pub fn render(s: String) -> Result<PngData> {
    let text = chunkify(s, 80).concat();
    println!("{}", text);
    let _sdl_context = try!(sdl2::init());
    let ttf_context = try!(sdl2_ttf::init());
    let font = try!(ttf_context.load_font(std::path::Path::new("Anonymous Pro.ttf"), 11));
    //font.set_hinting(std2_ttf::Hinting::None);
    let (ws, hs): (Vec<_>, Vec<_>) = try!(text.trim().lines().map(|x| font.size_of(x)).collect::<std::result::Result<Vec<(u32, u32)>, _>>()).into_iter().unzip();
    let (w, h) = (ws.into_iter().max().unwrap(), hs.into_iter().map(|h| std::cmp::max(h, font.recommended_line_spacing() as u32)).sum::<u32>());
    let mut image = image::ImageBuffer::from_pixel(w, h, image::LumaA::<u8> { data: [0u8, 0u8] });
    for (i, line) in text.trim().lines().enumerate() {
        if try!(font.size_of(line)).0 == 0 { continue; }
        let mut s = try!(font.render(line).solid(sdl2::pixels::Color::RGBA(0, 0, 0, 255))); // 0 => BG, 1 => FG
        let ib = try!(solid_to_luma(&mut s));
        image::imageops::overlay(&mut image, &luma_to_lumaa(ib), 0, (i as u32) * (font.recommended_line_spacing() as u32));
    }
    //for p in image.iter_mut() {
    //    *p = !(p.wrapping_sub(1));// >> 1 << 1;
    //}
    //let mut image = luma_to_lumaa(image);
    //for p in image.pixels_mut() {
    //    p.channels_mut()[1] = p.channels()[0];//.saturating_sub(32);
    //    //println!("{:?}", p);
    //}
    //let mut square = {
    //    let l = std::cmp::max(image.width(), image.height());
    //    image::ImageBuffer::from_pixel(l, l, image.pixels().next().unwrap().clone())
    //};
    //image::imageops::overlay(&mut square, &image, 0, 0);

    let mut png_buffer = Vec::<u8>::new();
    try!(image::png::PNGEncoder::new(&mut png_buffer).encode(&image, image.width(), image.height(), image::GrayA(8)));
    //try!(try!(std::fs::File::create("/tmp/annx.png")).write_all(png_buffer.clone().as_slice()));
    //panic!();
    Ok(png_buffer)
}

