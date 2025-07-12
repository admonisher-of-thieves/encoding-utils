# simple-ssimu2

```sh
Calculate SSIMULACRA2 metric - Using vszip

Usage: simple-ssimu2 [OPTIONS] --reference <REFERENCE> --distorted <DISTORTED>

Options:
  -r, --reference <REFERENCE>
          Reference video file
  -d, --distorted <DISTORTED>
          Distorted video file (encoded version)
  -S, --scenes <SCENES>
          JSON file containing scene information
  -s, --steps <STEPS>
          Frame step value (process every N-th frame) [default: 1]
  -v, --verbose
          Enable verbose output - Print all scores
  -s, --source-plugin <SOURCE_PLUGIN>
          Video Source Plugin [default: lsmash] [possible values: lsmash, bestsource]
  -s, --stats-file <STATS_FILE>
          Path to stats file (if not provided, stats will only be printed)
  -t, --trim <TRIM>
          Trim to sync video: format is "first,last,clip" Example: "6,18,distorted" or "6,18,d"
  -n, --middle-frames <N_FRAMES>
          Allows you to use a distorted video composed of n frames. Needs scenes file [default: 0]
  -d, --frames-distribution <FRAMES_DISTRIBUTION>
          How the frames are distributed when encoding [default: center] [possible values: center, evenly, start-middle-end]
  -k, --keep-files
          Keep temporary files (disables automatic cleanup)
      --color-metadata <COLOR_METADATA>
          Color params base on the svt-av1 params [default: "--color-primaries bt709 --transfer-characteristics bt709 --matrix-coefficients bt709 --color-range studio --chroma-sample-position left"]
      --crop <CROP>
          Crop (e.g. 1920:816:0:132)
      --downscale <DOWNSCALE>
          Downscale, using Box Kernel 0.5 [default: false] [possible values: true, false]
      --detelecine <DETELECINE>
          Removes telecine — A process used to convert 24fps film to 29.97fps video using a 3:2 pulldown pattern [default: false] [possible values: true, false]
  -p, --plot-file <PLOT_FILE>
          Save a plot of the SSIMU2 stats (Needs to be an .svg file)
  -t, --temp <TEMP>
          Temp folder (default: "[TEMP]_<input>.json" if no temp folder given)
  -h, --help
          Print help
  -V, --version
          Print version
```
