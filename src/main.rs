use srtlib::{Subtitle, Subtitles};
use palette::{FromColor, Srgb, Lch, Darken};
use regex::Regex;
use ffmpeg_sidecar::event::OutputVideoFrame;

const SRT_INPUT: &str = "input.srt";
const VIDEO_INPUT: &str = "input.mkv";
const OUTPUT: &str = "output.srt";
const COLOR_DEFAULT: &str = "#ffffff";

const COLOR_TAG_START: &str = "<font color=\"";
const COLOR_TAG_END: &str = "</font>";

// rgb hex color code
const HEX_COLOR_REGEX: &str = r"#[0-9a-fA-F]{6}";



fn add_color(sub: &mut Subtitle, brightness: f32) -> &mut Subtitle {
    let subtext = &mut sub.text;
    
    // you could encounter existing color tags within the .srt
    // they will override the outer tag we're about to place, so they need to be transformed too
    let color_regex = Regex::new(HEX_COLOR_REGEX).unwrap();
    for m in color_regex.find_iter(subtext.clone().as_str()) {
        let match_str = m.as_str();
        *subtext = subtext.replace(match_str, &transform_color(String::from(match_str), brightness));
    }
    // add "<font color=newcolor>" before the subtitle text and </font> after
    *subtext = format!("{}{}{}{}{}", COLOR_TAG_START, transform_color(String::from(COLOR_DEFAULT) , brightness), "\">", subtext, COLOR_TAG_END);
    return sub;
}

fn transform_color(color: String, brightness: f32) -> String {
    let r = u8::from_str_radix(&color[1..3], 16).expect("Err at r") as f32;
    let g = u8::from_str_radix(&color[3..5], 16).expect("Err at g") as f32;
    let b = u8::from_str_radix(&color[5..], 16).expect("Err at b") as f32;

    let lch = Lch::from_color(Srgb::new(r,g,b));
    let darkened = lch.darken(brightness);
    let new_rgb: Srgb<u8> = Srgb::from_color(darkened).into();

    format!("#{:02x}{:02x}{:02x}", new_rgb.red, new_rgb.green, new_rgb.blue)
}

// https://ffmpeg.org/ffmpeg-utils.html#time-duration-syntax
fn ffmpeg_timestamp(stamp: (u8, u8, u8, u16)) -> String {
    format!("S+{}ms", ((stamp.0 as u64 * 3600000) + (stamp.1 as u64 * 60000) + (stamp.2 as u64 * 1000) + stamp.3 as u64).to_string())
}

fn average_brightness(frame: &OutputVideoFrame) -> f32 {
    let data = &frame.data;
    let sum: f32 = data.iter().map(|x| *x as f32).sum();

    (sum / data.len() as f32).clamp(0.0, 1.0)
}

// pretty weak right now. it spawns a different ffmpeg process for each frame seek.
// maybe an ffmpeg video stream iterator that i can next() for a single frame, and seek in between?
fn main() {
    let mut video = ffmpeg_sidecar::command::FfmpegCommand::new();
    video
        .input(VIDEO_INPUT)
        // .no_audio() // not sure if relevant
        .format("rawvideo")
        // converts to grayscale. for yuv color space, strips u&v;
        // for rgb, applies r * 0.299 + g * 0.587 + b * 0.114 perceived brightness formula.
        // either way, we end up with the desired brightness value per-pixel
        .pix_fmt("gray")
        .frames(1);

    let subs = &mut Subtitles::parse_from_file(SRT_INPUT, None).unwrap();
    for s in subs.into_iter() {
        let timestamp = ffmpeg_timestamp(s.start_time.get());
        let frame_cmd = video.seek(&timestamp);
        // --- the command is now built and a process can be spawned ---
        let mut frame_iter = frame_cmd
            .spawn()
            .expect("Err on spawn")
            .iter()
            .expect("Err on iter")
            .filter_frames();
        let frame = &frame_iter
            .find(|_| true)
            .expect(&format!("Err on decode. timestamp: {timestamp}"));
        let brightness = average_brightness(&frame);
        add_color(s, brightness);
    }
    subs.write_to_file(OUTPUT, None).unwrap();
}
