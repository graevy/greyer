use srtlib::{Subtitle, Subtitles, Timestamp};
use regex::Regex;
use std::process::Command;
use clap::Parser as clapParser;
use tl;

const COLOR_DEFAULT: &str = "#7f7f7f";
const COLOR_MIDPOINT: f32 = 127.0;

// rgb hex color code
const HEX_COLOR_REGEX: &str = r"#[0-9a-fA-F]{6}";

const DEFAULT_COEFFICIENT: &str = "0.25";


#[derive(clapParser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short='s', long="input_srt", help="input .srt file to color-correct")]
    input_sub: String,

    #[arg(short='v', long="input_video", help="input video container to grab frame brightness from")]
    input_vid: String,

    #[arg(short='o', long="output_srt", help="output .srt file, color-corrected", default_value="out.srt")]
    output_sub: String,

    #[arg(short='c', long="correction_coefficient",
    help="0 to 1 inclusive float -- 1 sets color to COLOR_MIDPOINT, 0 leaves as-is, 0.5 averages",
    default_value=DEFAULT_COEFFICIENT
    )]
    coefficient: f32,

    #[arg(short='f', long="fast",
    help="use nearest-iframe brightness instead of decoding each srt timestamp",
    default_value="false"
    )]
    fast: bool,
}


// this is just for the case where color is already hardcoded into the srt file
// my preference is to respect those existing hardcodes, color-correct them, and skip adding
// this isn't complete, i think there's an edge case of a font tag that doesn't include color data
fn replace_existing_sub_colors(sub: &mut Subtitle, correction: i32) {
    let subtext = &mut sub.text;

    let color_regex = Regex::new(HEX_COLOR_REGEX).expect("Err at regex, somehow?");
    for m in color_regex.find_iter(subtext.clone().as_str()) {
        let match_str = m.as_str();
        *subtext = subtext.replace(match_str, &correct_rgb_hex(match_str, correction));
    }
}

fn add_color_to_sub(sub: &mut Subtitle, correction: i32) {
    let subtext = &mut sub.text;

    // add "<font color=newcolor>" before the subtitle text and </font> after
    *subtext = format!(
        "{}{}{}{}{}",
        "<font color=\"",
        correct_rgb_hex(COLOR_DEFAULT, correction),
        "\">", subtext, "</font>"
    );
}

fn correct_rgb_hex(color: &str, correction: i32) -> String {
    let mut r = i32::from_str_radix(&color[1..3], 16).expect("Err at r");
    let mut g = i32::from_str_radix(&color[3..5], 16).expect("Err at g");
    let mut b = i32::from_str_radix(&color[5..], 16).expect("Err at b");

    r = (r + correction).clamp(0, 255);
    g = (g + correction).clamp(0, 255);
    b = (b + correction).clamp(0, 255);


    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

fn get_correction(color: f32, args: &Args) -> i32 {
    ((color - COLOR_MIDPOINT) * &args.coefficient) as i32
}

// https://ffmpeg.org/ffmpeg-utils.html#time-duration-syntax
fn ffmpeg_timestamp(stamp: (u8, u8, u8, u16)) -> String {
    format!("{}", (
        (stamp.0 as f32 * 3600.0) + 
        (stamp.1 as f32 * 60.0) + 
        (stamp.2 as f32 * 1.0) + 
        stamp.3 as f32 / 1000.0
    ).to_string())
}

// grabs brightness at closest iframe to timestamp
// i suspect the next version of this should use `ffprobe read_intervals`;
// read_intervals can accept essentially a .csv of timestamps
fn get_frame_yavg_fast(timestamp: &String, input_vid: &String) -> f32 {
    // ffprobe solves the problem of getting a frame's brightness very carefully, with this monstrosity:
    //     "ffprobe -v quiet -hide_banner -f lavfi movie={}:seek_point={},signalstats,trim=end_frame=1 \
    //     -show_entries frame_tags=lavfi.signalstats.YAVG -of default=noprint_wrappers=1:nokey=1"

    // -v quiet suppresses the log
    // -hide_banner suppresses printing file metadata at the start of the command
    // -f lavfi specifies that we're reading a "filtergraph" instead of a file,
    //      and to use ffmpeg's filter lib for parsing, libavfilter
    // "movie=input.mkv:seek_point=123,signalstats,trim=end_frame=1" is the filtergraph.
    //      signalstats will give us per-frame metadata, and we're only reading one frame
    //      trim=duration=1234ms is also valid trim syntax, if we want to get the average over the entire srt entry
    // -show_entries frame_tags=lavfi.signalstats.YAVG
    //      filters all signalstats output to just YAVG, the average luma channel brightness of a frame
    // -of default=noprint_wrappers=1:nokey=1
    //      the output format (of) is default, but don't print the tag wrappers e.g. [FRAME][/FRAME],
    //      or the keys, so omit "TAG:lavfi.signalstats.YAVG=" preceeding the YAVG

    let mut cmd = Command::new("ffprobe");
    cmd
        .args(&[("-v"),
        ("quiet"),
        ("-hide_banner"),
        ("-f"),
        ("lavfi"),
        (format!("movie={}:seek_point={},signalstats,trim=end_frame=1", input_vid, timestamp).as_str()),
        ("-show_entries"),
        ("frame_tags=lavfi.signalstats.YAVG"),
        ("-of"),
        ("default=noprint_wrappers=1:nokey=1"),
        ]);

    // command output should be a utf-8 byte vec, ending in a newline
    let res = match cmd.output() {
        Ok(out) => {
            println!("{}", out.status);
            if !out.status.success() {
                eprintln!("Err on termination: {}", out.status);
                String::new()
            } else {
                String::from_utf8(out.stdout)
                    .expect("Err on ffprobe stdout decode")
            }
        },
        Err(e) => {
            eprintln!("Err on ffprobe execution: {}", e);
            String::new()
        }
    };

    match res.trim_end().parse::<f32>() {
        Ok(brightness) => brightness,
        Err(_) => {
            println!("res: {} not f32", res);
            COLOR_MIDPOINT
        }
    }
}

// uses accurate_seek, meaning it decodes video from preceeding iframe to supplied timestamp
// next version probably involves delving into libavcodec/libavfilter to manipulate streams directly
fn get_frame_yavg_slow(timestamp: &String, input_vid: &String) -> f32 {
    // final command should look like
    // ffmpeg -y -hide_banner -accurate_seek -ss 1337 -i input.mkv -filter_complex signalstats,metadata=print:key=lavfi.signalstats.YAVG,trim=end_frame=0 -an -f null -
    let mut cmd = Command::new("ffmpeg");
    cmd
        .args(&[
            ("-y"),                     // overwrite existing output
            ("-hide_banner"),           // reduce printed metadata
            ("-accurate_seek"),         // decode video until reaching timestamp
            ("-ss"),                    // seek to timestamp
            (timestamp.as_str()),
            ("-i"),                     // use this video as input
            (input_vid.as_str()),
            ("-filter_complex"),        // filtergraph does not have one input/output of same type, so is "complex"
            ("signalstats,metadata=print:key=lavfi.signalstats.YAVG,trim=end_frame=0"),
            ("-an"),                    // drop the audio output stream
            ("-f"),                     // use null output format i.e. do not encode the input stream to an output stream
            ("null"),
            ("-"),                      // dump to stderr. metadata=print pushes everything to stderr, for whatever reason
        ]);

    let res = match cmd.output() {
        Ok(out) => {
            if !out.status.success() {
                eprintln!("Err on termination: {}", out.status);
                String::new()
            } else {
                String::from_utf8(out.stderr)
                    .expect("Err on ffmpeg stdout decode")
            }
        },
        Err(e) => {
            eprintln!("Err on ffmpeg execution: {}", e);
            String::new()
        }
    };

    let re = Regex::new(r"lavfi.signalstats.YAVG=(\d+\.\d+)").unwrap();
    let yavg = if let Some(caps) = re.captures(&res) {
        String::from(&caps[1])
    } else {
        eprintln!("YAVG value not found in output. Defaulting to {}", COLOR_MIDPOINT);
        return COLOR_MIDPOINT
    };

    match yavg.trim_end().parse::<f32>() {
        Ok(brightness) => brightness,
        Err(_) => {
            println!("res: {} not valid f32", res);
            COLOR_MIDPOINT
        }
    }
}

// only needs to be its own function because of the watermark headache
// watermark confirms that you're using the correct subs. also advertising is cute
fn get_subtitles_from_file(file: &String) -> Subtitles {
    let mut file_subs_vec = Subtitles::parse_from_file(file, None)
        .expect(format!("Err parsing {}", file).as_str()).to_vec();

    if file_subs_vec.len() == 0 {
        panic!("No subtitles found");
    }

    let sub_start_time = file_subs_vec[0].start_time;
    let watermark_subtitle = Subtitle::new(
        0, Timestamp::new(0, 0, 0, 0),
        sub_start_time,
        String::from("Srt color-corrected with https://github.com/graevy/greyer")
    );
    file_subs_vec.insert(0, watermark_subtitle);
    Subtitles::new_from_vec(file_subs_vec)
}

fn main() {
    let args = Args::parse();

    let subs = &mut get_subtitles_from_file(&args.input_sub);
    let len = subs.len() - 1;

    let yavg_function = if args.fast {
        get_frame_yavg_fast
    } else { 
        get_frame_yavg_slow
    };

    let start = std::time::Instant::now();
    let mut prev: std::time::Duration;
    for (idx, mut sub) in subs.into_iter().enumerate() {
        let timestamp = ffmpeg_timestamp(sub.start_time.get());
        println!("Fetching YAVG {} / {} @ t:{}s", idx, len, timestamp);

        prev = start.elapsed();
        let yavg = yavg_function(&timestamp, &args.input_vid);

        let correction = get_correction(yavg, &args);
        println!("YAVG={}, correction={} (in {:.3?})\r\n", yavg, correction, start.elapsed() - prev);

        let subtext = sub.text.clone();
        let parser = tl::parse(subtext.as_str(), tl::ParserOptions::default()).expect("Err on parse");
        let mut existing_font_tags = false;
        for node in parser.nodes() {
            if let Some(tag) = node.as_tag() {
                if tag.name() == "font" {
                    existing_font_tags = true;
                }
            }  
        }

        sub.num += 1; // because watermark was added

        if existing_font_tags {
            replace_existing_sub_colors(&mut sub, correction)
        }
        else {
            add_color_to_sub(&mut sub, correction);
        }
    }
    subs.write_to_file(&args.output_sub, None)
        .expect(format!("Err writing to {}", &args.output_sub).as_str());
}
