use image::{DynamicImage, GrayImage, Luma, Pixel};
use image::io::Reader as ImageReader;
use std::env;

struct StringConfig {
    pegs_x: usize,
    pegs_y: usize,
    passes: usize,
    pass_val: u8,
    invert: bool,
}

impl Default for StringConfig {
    fn default() -> StringConfig {
        StringConfig {
            pegs_x: 93,
            pegs_y: 93,
            passes: 0xc00,
            pass_val: 0x24,
            invert: true,
        }
    }
}

#[derive(Copy, Clone)]
struct Peg {
    x: i32,
    y: i32,
}

fn checkpx(src: &GrayImage, dst: &mut GrayImage, x: u32, y: u32, val: u8, apply: bool) -> i64 {
    let Luma([spx,]) = src.get_pixel(x, y).to_luma();
    let Luma([dpx,]) = dst.get_pixel(x, y).to_luma();

    let newpx = match dpx.checked_add(val) {
        None => u8::MAX,
        Some(v) => v,
    };

    let old_dif = (i64::from(dpx) - i64::from(spx)).abs();
    let new_dif = (i64::from(newpx) - i64::from(spx)).abs();
    let err: i64 = old_dif - new_dif;

    if apply {
        dst.put_pixel(x, y, Luma([newpx;1]));
    }

    return err;
}

fn line_low(src: &GrayImage, dst: &mut GrayImage, start: Peg, end: Peg, val: u8, apply: bool) -> i64 {
    let dx = end.x - start.x;
    let mut dy = end.y - start.y;
    let mut yi = 1;

    if dy < 0 {
        yi = -1;
        dy = -dy;
    }

    let mut d = (2 * dy) - dx;

    let mut y = start.y;

    let mut total: i64 = 0;
    for x in start.x..(end.x+1) {
        total += checkpx(src, dst, x as u32, y as u32, val, apply);

        if d > 0 {
            y = y + yi;
            d = d + (2 * (dy - dx));
        } else {
            d = d + (2 * dy);
        }
    }

    return total;
}

fn line_high(src: &GrayImage, dst: &mut GrayImage, start: Peg, end: Peg, val: u8, apply: bool) -> i64 {
    let mut dx = end.x - start.x;
    let dy = end.y - start.y;
    let mut xi = 1;

    if dx < 0 {
        xi = -1;
        dx = -dx;
    }

    let mut d = (2 * dx) - dy;

    let mut x = start.x;

    let mut total: i64 = 0;
    for y in start.y..(end.y+1) {
        total += checkpx(src, dst, x as u32, y as u32, val, apply);

        if d > 0 {
            x = x + xi;
            d = d + (2 * (dx - dy));
        } else {
            d = d + (2 * dx);
        }
    }

    return total;
}

fn get_line(src: &GrayImage, dst: &mut GrayImage, start: Peg, end: Peg, val: u8, apply: bool) -> i64 {
    if (end.y - start.y).unsigned_abs() < (end.x - start.x).unsigned_abs() {
        // slope is low
        if start.x < end.x {
            // going positive
            line_low(src, dst, start, end, val, apply)
        } else {
            // swap start/end
            line_low(src, dst, end, start, val, apply)
        }
    } else {
        // slope is high
        if start.y < end.y {
            // going positive
            line_high(src, dst, start, end, val, apply)
        } else {
            // swap start/end
            line_high(src, dst, end, start, val, apply)
        }
    }
}

fn gen_img(src: &GrayImage, dst: &mut GrayImage, options: &StringConfig) {
    // for given number of passes, go from current peg and find what line most brings crossed pixels closer to the image's value

    // TODO also generate a coordinate path

    // set up the pegs
    let numpegs = (options.pegs_x * 2) + (options.pegs_y * 2) - 1;
    let mut pegs = vec![Peg{x:0, y:0}; numpegs];

    //TODO other shapes
    // for now we just rectangle
    let w: usize = src.width() as usize;
    let h: usize = src.height() as usize;

    let bottom: i32 = (h - 1) as i32;
    let right: i32 = (w - 1) as i32;

    let linecolor = options.pass_val;

    let xseg = w / options.pegs_x;
    for i in 0..options.pegs_x {
        let xpos = (xseg * i) as i32;
        pegs[i] = Peg{x: xpos, y: 0};
        pegs[i + options.pegs_x] = Peg{x: xpos, y: bottom};
    }

    let ysides_off = (options.pegs_x * 2) - 1;
    let yseg = h / options.pegs_y;
    for i in 0..options.pegs_y {
        let ypos = (yseg * i) as i32;
        if ypos != 0 {
            // don't make 0,0 twice
            pegs[i + ysides_off] = Peg{x: 0, y: ypos};
        }
        pegs[i + ysides_off + options.pegs_y] = Peg{x: right, y: ypos};
    }

    let mut current = pegs[0];
    for i in 0..options.passes {
        let mut best_err = 0;
        let mut best_peg = Peg{x: -1, y: -1};

        for p in &pegs {
            if  current.x == 0 && p.x == 0 ||
                current.x == right && p.x == right ||
                current.y == 0 && p.y == 0 ||
                current.y == bottom && p.y == bottom
            {
                continue;
            }

            // get total error of this line
            let err = get_line(src, dst, current, *p, linecolor, false);

            if err > best_err {
                best_err = err;
                best_peg = *p;
            }
        }

        if best_err <= 0 {
            println!("Could not find a good destination, stopping early after {} lines", i);
            break;
        }

        // draw to the best peg
        get_line(src, dst, current, best_peg, linecolor, true);

        current = best_peg;
    }
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

    let conf = StringConfig {
        ..Default::default()
    };

    // TODO parse rest of args into StringConfig

    let mut imgin = ImageReader::open(&args[1])?.decode()?.grayscale();

    if conf.invert {
        imgin.invert();
    }

    let img = DynamicImage::new_luma8(imgin.width(), imgin.height());

    let imgin = imgin.into_luma8();
    let mut img = img.into_luma8();

    gen_img(&imgin, &mut img, &conf);

    let mut img = DynamicImage::from(img);

    if conf.invert {
        img.invert();
    }

    img.save(&args[2])?;

    Ok(())
}
