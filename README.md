this script:
- takes a video file and a .srt file
- takes the start timestamp from each subtitle entry in the srt file
- uses ffmpeg to extract the perceived average brightness of the frame at each entry
- adds or amends existing color tags in a new .srt file to darken subtitles when the brightness is low

it's only useful for HDR on OLEDs

still experimenting with it
