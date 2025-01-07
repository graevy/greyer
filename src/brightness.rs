// need to debug this llm slop

use ffmpeg_the_third as ffmpeg;

pub struct BrightnessAnalyzer {
    context: ffmpeg::format::context::Input,
    decoder: ffmpeg::decoder::Video,
    scaler: ffmpeg::software::scaling::Context,
    video_stream_index: usize,
    time_base: ffmpeg::Rational,
}

impl BrightnessAnalyzer {
    pub fn new(file_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        ffmpeg::init()?;
        let context = ffmpeg::format::input(file_path)?;
        let video_stream = context
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or("No video stream found")?;
        let video_stream_index = video_stream.index();
        let time_base = video_stream.time_base();

        let decoder = video_stream.codec().decoder().video()?;
        let scaler = ffmpeg::software::scaling::Context::get(
            decoder.format(),
            decoder.width(),
            decoder.height(),
            ffmpeg::format::Pixel::GRAY8, // Extract only the luma (brightness) channel
            decoder.width(),
            decoder.height(),
            ffmpeg::software::scaling::Flags::BILINEAR,
        )?;

        Ok(Self {
            context,
            decoder,
            scaler,
            video_stream_index,
            time_base,
        })
    }

    // Seek to a specific timestamp and decode the corresponding frame's brightness
    pub fn query_brightness(&mut self, timestamp: f64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let seek_target = (timestamp * f64::from(self.time_base.denominator()) / f64::from(self.time_base.numerator())) as i64;

        // Seek to the target timestamp
        self.context.seek(seek_target, ffmpeg::format::context::SeekFlags::ANY)?;

        // Decode the frame at the target timestamp
        for (stream, packet) in self.context.packets() {
            if stream.index() == self.video_stream_index {
                self.decoder.send_packet(&packet)?;

                while let Ok(frame) = self.decoder.receive_frame() {
                    let mut gray_frame = ffmpeg::frame::Video::empty();
                    self.scaler.run(&frame, &mut gray_frame)?;

                    // Return the luma channel (brightness) data
                    return Ok(gray_frame.data(0).to_vec());
                }
            }
        }

        Err("No frame decoded at the specified timestamp".into())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut analyzer = BrightnessAnalyzer::new("input.mp4")?;

    // Query brightness at different timestamps
    let timestamps = vec![1.0, 2.5, 5.0]; // In seconds
    for ts in timestamps {
        let brightness_data = analyzer.query_brightness(ts)?;
        println!("Brightness data at {}s: {:?}", ts, brightness_data);
    }

    Ok(())
}