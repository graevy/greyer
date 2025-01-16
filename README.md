this script:
- takes a video file and a .srt file
- takes the start timestamp from each subtitle entry in the srt file
- uses ffmpeg to extract the perceived average brightness of the frame at each entry
- adds or amends existing color tags in a new .srt file to darken subtitles when the brightness is low

only good use-case is HDR on OLEDs

`cargo build`
`cargo run -- -s input_srt_file -v input_video_file -o output_srt_file`