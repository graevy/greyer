this script:
- takes a video file and a .srt file
- takes the start timestamp from each subtitle entry in the srt file
- uses ffmpeg to extract the perceived average brightness of the frame at each entry
- averages the brightness with the color midpoint #7f7f7f, making subtitles more grey (TODO nonlinear interp)
- adds or amends existing color tags in a new .srt file with the new color
  
only good use-case is HDR on OLEDs

`cargo run -- -s input_srt_file -v input_video_file [-o output_srt_file] [-c correction_coefficient] [--fast]`
