use anyhow::Result;
use mime;
use std::fs::File;
use std::io::BufReader;
use std::io::Cursor;
use std::io::Write;
use std::path;
use thumbnailer::{create_thumbnails, ThumbnailSize};

static FILE: &str = "/Users/jamie/Desktop/FF0QjSoUcAUEciH.jpg";

pub fn main() {
    let start = std::time::Instant::now();
    let file = File::open(FILE).unwrap();
    let reader = BufReader::new(file);

    let mut thumbnails =
        create_thumbnails(reader, mime::IMAGE_JPEG, [ThumbnailSize::Small]).unwrap();

    let thumbnail = thumbnails.pop().unwrap();

    let mut buf = Cursor::new(Vec::new());

    thumbnail.write_png(&mut buf).unwrap();

    // write buf to file
    File::create(path::Path::new(
        "/Users/jamie/Desktop/FF0QjSoUcAUEciH_thumb.png",
    ))
    .unwrap()
    .write_all(&buf.get_ref())
    .unwrap();
    println!("done {:?}", start.elapsed());
}

use image::*;
use std::path::Path;
use webp::*;

fn main() {
    let _start = std::time::Instant::now();
    let start = std::time::Instant::now();
    // Using `image` crate, open the included .jpg file
    let img = image::open("/Users/jamie/Desktop/IMG_5812.jpeg").unwrap();
    // let (w, h) = img.dimensions();
    println!("loaded image {:?}", start.elapsed());
    let start = std::time::Instant::now();
    // Optionally, resize the existing photo and convert back into DynamicImage
    // let size_factor = 1.0;
    // let img: DynamicImage = image::DynamicImage::ImageRgba8(imageops::resize(
    //     &img,
    //     (w as f64 * size_factor) as u32,
    //     (h as f64 * size_factor) as u32,
    //     imageops::FilterType::Triangle,
    // ));
    println!("resized image {:?}", start.elapsed());
    let start = std::time::Instant::now();
    // Create the WebP encoder for the above image
    let encoder: Encoder = Encoder::from_image(&img).unwrap();

    // Encode the image at a specified quality 0-100
    let webp: WebPMemory = encoder.encode(30f32);
    println!("encoded image {:?}", start.elapsed());
    let start = std::time::Instant::now();
    // Define and write the WebP-encoded file to a given path
    let output_path = Path::new("/Users/jamie/Desktop/IMG_5812").with_extension("webp");
    std::fs::write(&output_path, &*webp).unwrap();

    println!("written to file webp {:?}", start.elapsed());
    println!("done in {:?}", _start.elapsed());
}
