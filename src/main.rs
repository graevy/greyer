use srtlib::{Subtitle, Subtitles, Timestamp};
use palette::Srgb;
use regex::Regex;
// use time::Time;

mod brightness;

const SRT_INPUT: &str = "input.srt";
const VIDEO_INPUT: &str = "input.mp4";
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

// placeholder. palette lch maybe
fn transform_color(color: String, brightness: f32) -> String {
    return color;
}

fn main() {
    let video = brightness::BrightnessAnalyzer::new(VIDEO_INPUT)?;
    let subs = &mut Subtitles::parse_from_file(SRT_INPUT, None).unwrap();
    for s in subs.into_iter() {
        // oh god
        let timetuple = s.start_time.get();
        let timef64: f64 = f64::from(timetuple.0 * 3600) + f64::from(timetuple.1 * 60) + f64::from(timetuple.2) + f64::from(timetuple.3) / 1000.0;
        let brightness = video.query_brightness(timef64)?;
        add_color(s, brightness);
    }
    subs.write_to_file(OUTPUT, None).unwrap();
}
