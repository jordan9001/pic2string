use image::DynamicImage;
use image::io::Reader as ImageReader;
use imageproc::drawing;
use std::env;

struct StringConfig {
    pegs_x: u32,
    pegs_y: u32,
    passes: u32,
}

impl Default for StringConfig {
    fn default() -> StringConfig {
        StringConfig {
            pegs_x: 64,
            pegs_y: 64,
            passes: 512,
        }
    }
}

fn gen_img(src: &DynamicImage, dst: &mut DynamicImage, options: &StringConfig) {
    // for given number of passes, go from current peg and find what line most brings crossed pixels closer to the image's value
}

fn usage() {
    println!("Usage: pic2string <path/to/input/img> <path/for/output/img>")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Take argument as image
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        usage();
        return Err("Invalid Command Arguments".into());
    }

    let imgin = ImageReader::open(&args[1])?.decode()?.grayscale();

    let mut img = DynamicImage::new_rgba8(imgin.width(), imgin.height());

    gen_img(&imgin, &mut img, &StringConfig {
        ..Default::default()
    });

    img.save(&args[2])?;

    Ok(())
}
