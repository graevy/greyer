use srtlib::{Subtitle, Subtitles};
use regex::Regex;
use ffmpeg_sidecar::event::OutputVideoFrame;
use clap::Parser;

const COLOR_DEFAULT: &str = "#7f7f7f";
const COLOR_MIDPOINT: i32 = 127;

// rgb hex color code
const HEX_COLOR_REGEX: &str = r"#[0-9a-fA-F]{6}";

// 0 to 1 inclusive float -- 1 sets color to COLOR_MIDPOINT, 0 leaves as-is, 0.5 averages, etc
const CORRECTION_COEFFICIENT: f32 = 0.5;


#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short='s', long="input_srt", help="input .srt file to color-correct")]
    input_sub: String,

    #[arg(short='v', long="input_video", help="input video container to grab frame brightness from")]
    input_vid: String,

    #[arg(short='o', long="output_srt", help="output .srt file, color-corrected")]
    output_sub: String,
}


// this is just for the case where color is already hardcoded into the srt file
// my preference is to respect those existing hardcodes, color-correct them, and skip adding
// this isn't complete, i think there's an edge case of a font tag that doesn't include color data
fn replace_existing_sub_colors(sub: &mut Subtitle, correction: isize) -> bool {
    let subtext = &mut sub.text;
    let mut replaced = false;

    let color_regex = Regex::new(HEX_COLOR_REGEX).expect("Err at regex, somehow?");
    for m in color_regex.find_iter(subtext.clone().as_str()) {
        replaced = true;
        let match_str = m.as_str();
        *subtext = subtext.replace(match_str, &transform_color(match_str, correction));
    }
    replaced
}

fn add_color_to_sub(sub: &mut Subtitle, correction: isize) {
    let subtext = &mut sub.text;

    // add "<font color=newcolor>" before the subtitle text and </font> after
    *subtext = format!(
        "{}{}{}{}{}",
        "<font color=\"",
        transform_color(COLOR_DEFAULT, correction),
        "\">", subtext, "</font>"
    );
}

fn transform_color(color: &str, correction: isize) -> String {
    let mut r = isize::from_str_radix(&color[1..3], 16).expect("Err at r");
    let mut g = isize::from_str_radix(&color[3..5], 16).expect("Err at g");
    let mut b = isize::from_str_radix(&color[5..], 16).expect("Err at b");

    r = (r + correction).clamp(0, 255);
    g = (g + correction).clamp(0, 255);
    b = (b + correction).clamp(0, 255);


    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

// https://ffmpeg.org/ffmpeg-utils.html#time-duration-syntax
fn ffmpeg_timestamp(stamp: (u8, u8, u8, u16)) -> String {
    format!("{}ms", (
        (stamp.0 as u64 * 3600000) + 
        (stamp.1 as u64 * 60000) + 
        (stamp.2 as u64 * 1000) + 
        stamp.3 as u64
    ).to_string())
}

// of all pixels in a frame, e.g. all 2073600 pixels in a 1080p frame.
fn get_average_brightness(frame: &OutputVideoFrame) -> i32 {
    let data = &frame.data;
    let sum: i32 = data.iter().map(|x| *x as i32).sum();

    sum / data.len() as i32
}

// pretty weak right now. it spawns a different ffmpeg process for each frame seek.
// maybe an ffmpeg video stream iterator that i can next() for a single frame, and seek in between?
// would really require getting into ffmpeg, or at least the bindings, which do NOT want to build
fn main() {
    let args = Args::parse();
    let subs = &mut Subtitles::parse_from_file(&args.input_sub, None).unwrap();
    for sub in subs.into_iter() {
        let timestamp = ffmpeg_timestamp(sub.start_time.get());

        // build a command to extract the single frame we want. order matters
        let mut frame_cmd = ffmpeg_sidecar::command::FfmpegCommand::new();
        frame_cmd
            .seek(&timestamp)
            .input(&args.input_vid)
            .format("rawvideo")
            .overwrite()
            .pix_fmt("gray")
            .frames(1)
            .output("-");

        // --- the command is now built and a process can be spawned ---
        let mut frame_proc = frame_cmd
            .spawn()
            .expect("Err on spawn");

        // let proc_stdout = frame_proc.take_stdout().expect("Err on stdout");
        // let proc_stderr = frame_proc.take_stderr().expect("Err on stderr");

        let mut frame_iter = frame_proc
            .iter()
            .expect("Err on iter")
            .filter_frames();

        let frame = frame_iter
            .find(|_| true)
            .expect(&format!("Err on decode. timestamp: {timestamp}"));
        let brightness = get_average_brightness(&frame);
        let correction = ((brightness - COLOR_MIDPOINT) as f32 * CORRECTION_COEFFICIENT) as isize;
        if !replace_existing_sub_colors(sub, correction) { add_color_to_sub(sub, correction); }
    }
    subs.write_to_file(&args.output_sub, None).unwrap();
}
