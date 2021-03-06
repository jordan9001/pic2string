use image::{DynamicImage, GrayImage, Luma, Pixel};
use image::io::Reader as ImageReader;
use clap::Parser;
use std::path::PathBuf;
use rand::{thread_rng, Rng};

#[derive(Parser)]
struct StringConfig {
    #[clap(long, value_parser, default_value_t=72)]
    pegs: usize,
    #[clap(long, value_parser, default_value_t=0xC00)]
    passes: usize,
    #[clap(long, value_parser, default_value_t=0x42)]
    pass_val: u8,
    #[clap(long, value_parser, default_value_t=false)]
    invert: bool,
    #[clap(long, value_parser, default_value_t=false)]
    noextra: bool,
    #[clap(long, value_parser, default_value_t=1)]
    depth: usize,
    #[clap(long, value_parser, default_value_t=0)]
    randpos: usize,
    #[clap(short, long, value_parser)]
    infile: PathBuf,
    #[clap(short, long, value_parser)]
    outfile: PathBuf,
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

fn best_lines(src: &GrayImage, dst: &mut GrayImage, pegs: &Vec<Peg>, current: Peg, linecolor: u8, depth: usize, maxdepth: usize, constraints: [i32;4], tryextra: bool) -> (i64, Vec<Peg>) {

    // bruteforce best upto max depth
    if depth >= maxdepth {
        return (0, vec![]);
    }

    let [left, top, right, bottom] = constraints;

    // for each peg
    let mut best_err = 0;
    let mut best_pegs: Vec<Peg> = vec![];

    // try every combo of pegs
    for p in pegs {
        if  current.x == left && p.x == left ||
            current.x == right && p.x == right ||
            current.y == top && p.y == top ||
            current.y == bottom && p.y == bottom
        {
            continue;
        }

        // get total error of this line
        let err = get_line(src, dst, current, *p, linecolor, false);

        // add that with a recursed error
        //TODO heuristic here to say "that err is good enough, no need to recurse"?
        //TODO multithread here if sufficient maxdepth and at depth 0
        let (rerr, mut pegpath) = best_lines(src, dst, pegs, *p, linecolor, depth+1, maxdepth, constraints, false);

        let err = err + rerr;

        if err > best_err {
            best_err = err;
            best_pegs = vec![*p];
            best_pegs.append(&mut pegpath);
        }
    }

    if tryextra && best_err == 0 {
        // recurse a bit farther if we would have ended early
        return best_lines(src, dst, pegs, current, linecolor, 0, maxdepth+1, constraints, false);
    }

    return (best_err, best_pegs);
}

fn gen_img(src: &GrayImage, dst: &mut GrayImage, options: &StringConfig) {
    // for given number of passes, go from current peg and find what line most brings crossed pixels closer to the image's value

    // TODO also generate a coordinate path

    // set up the pegs
    let numpegs = (options.pegs * 4) - 1;
    let mut pegs = vec![Peg{x:0, y:0}; numpegs];

    //TODO other shapes, like circle or triangle
    // for now we just rectangle
    let w: usize = src.width() as usize;
    let h: usize = src.height() as usize;

    let bottom: i32 = (h - 1) as i32;
    let right: i32 = (w - 1) as i32;

    let linecolor = options.pass_val;
    let mut rng = thread_rng();

    let xseg = w / options.pegs;
    for i in 0..options.pegs {
        let xpos = (xseg * i) as i32;
        pegs[i] = Peg{x: xpos, y: 0};
        pegs[i + options.pegs] = Peg{x: xpos, y: bottom};
    }

    let ysides_off = (options.pegs * 2) - 1;
    let yseg = h / options.pegs;
    for i in 0..options.pegs {
        let ypos = (yseg * i) as i32;
        if ypos != 0 {
            // don't make 0,0 twice
            pegs[i + ysides_off] = Peg{x: 0, y: ypos};
        }
        pegs[i + ysides_off + options.pegs] = Peg{x: right, y: ypos};
    }

    let mut current = pegs[0];
    let perpercent = if options.passes >= 100 {
        options.passes / 100
    } else {
        1
    };
    let mut tillprint = perpercent;
    for i in 0..options.passes {
        if tillprint >= perpercent {
            tillprint = 0;
            println!("line {} / {}", i, options.passes);
        }
        tillprint += 1;

        let (best_err, pegpath) = best_lines(src, dst, &pegs, current, linecolor, 0, options.depth, [0, 0, right, bottom], !options.noextra);

        if best_err <= 0 {
            println!("Could not find a good destination, stopping early after {} lines", i);
            break;
        }

        // drawlines to the best peg and update current
        for p in pegpath {
            let mut start = current;
            let mut end = p;

            if options.randpos != 0 {
                let halfrange: i32 = (options.randpos / 2) as i32;
                let sxoff: i32= (rng.gen_range(0..options.randpos) as i32) - halfrange;
                let syoff: i32= (rng.gen_range(0..options.randpos) as i32) - halfrange;
                let exoff: i32= (rng.gen_range(0..options.randpos) as i32) - halfrange;
                let eyoff: i32= (rng.gen_range(0..options.randpos) as i32) - halfrange;

                start.x += sxoff;
                if start.x < 0 {
                    start.x = 0;
                } else if start.x > right {
                    start.x = right;
                }

                start.y += syoff;
                if start.y < 0 {
                    start.y = 0;
                } else if start.y > bottom {
                    start.y = bottom;
                }

                end.x += exoff;
                if end.x < 0 {
                    end.x = 0;
                } else if end.x > right {
                    end.x = right;
                }

                end.y += eyoff;
                if end.y < 0 {
                    end.y = 0;
                } else if end.y > bottom {
                    end.y = bottom;
                }
            }

            get_line(src, dst, start, end, linecolor, true);
            current = p;
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conf = StringConfig::parse();

    let mut imgin = ImageReader::open(&conf.infile)?.decode()?.grayscale();

    if !conf.invert {
        imgin.invert();
    }

    let img = DynamicImage::new_luma8(imgin.width(), imgin.height());

    let imgin = imgin.into_luma8();
    let mut img = img.into_luma8();

    gen_img(&imgin, &mut img, &conf);

    let mut img = DynamicImage::from(img);

    if !conf.invert {
        img.invert();
    }

    img.save(&conf.outfile)?;

    Ok(())
}
