# Encoding utils

- [frame-boost](#frame-boost)
- [simple-ssimu2](#simple-ssimu2)

## Requirements

- vapoursynth
- ffmpeg
- mkvtoolnix
- av1an

### Vapoursynth Plugins

- bestsource
- lsmas
- vszip
- fmtconv

## Installation

<https://www.rust-lang.org/tools/install>

```sh
cd frame-boost
cargo install --path .
```

```sh
cd simple-ssimu2
cargo install --path .
```

## frame-boost

```sh
Scene-based boost that dynamically adjusts CRF. It creates a scene-file with zone overrides

Usage: frame-boost [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>
          Input video file, you can also pass a .vpy script
  -o, --output <OUTPUT>
          Output scene file (default: "[BOOST]_<input>.json" if no output given)
  -t, --temp <TEMP>
          Temp folder (default: "[Temp]_<input>" if no temp folder given)
      --av1an-params <AV1AN_PARAMS>
          AV1an encoding parameters [default: "--verbose --workers 2 --concat mkvmerge --chunk-method bestsource --encoder svt-av1 --no-defaults"]
      --encoder-params <ENCODER_PARAMS>
          SVT-AV1 encoder parameters [default: "--preset 2 --tune 2 --keyint -1 --film-grain 0 --scm 0 --hbd-mds 1 --tile-columns 1 --enable-qm 1 --qm-min 8 --luminance-qp-bias 20  --kf-tf-strength 0 --psy-rd 1 --spy-rd 2 --complex-hvs 1 --input-depth 10 --color-primaries bt709 --transfer-characteristics bt709 --matrix-coefficients bt709 --color-range studio --chroma-sample-position left"]
  -q, --target-quality <TARGET_QUALITY>
          Target SSIMULACRA2 score (0-100) [default: 81]
  -c, --crf <CRF>
          Target CRF value(s) (70-1). Can be: - Single value (35) - Comma-separated list (35,27,21) - Range (36..21) - Stepped range (36..21:3) [default: 35,30,27,24,21]
  -n, --n-frames <N_FRAMES>
          Number of frames to encode for scene. Higher value increase the confidence than all the frames in the scene will be above your quality target at cost of encoding time [default: 10]
  -w, --workers <WORKERS>
          Workers to use when encoding [default: 2]
  -d, --frames-distribution <FRAMES_DISTRIBUTION>
          How the frames are distributed when encoding [default: center] [possible values: center, evenly, start-middle-end]
  -p, --velocity-preset <VELOCITY_PRESET>
          Velocity tuning preset (-1~13) [default: 4]
  -d, --scene-detection-method <SCENE_DETECTION_METHOD>
          Which method to use to calculate scenes [default: transnet-v2] [possible values: av1an, transnet-v2]
  -k, --keep-files
          Keep temporary files (disables automatic cleanup)
  -f, --force
          Disable overwrite protection (remove the scene file)
  -s, --source-metric-plugin <SOURCE_METRIC_PLUGIN>
          Video Source Plugin for metrics [default: lsmash] [possible values: lsmash, bestsource]
  -s, --source-encoding-plugin <SOURCE_ENCODING_PLUGIN>
          Video Source Plugin for encoding [default: lsmash] [possible values: lsmash, bestsource]
  -s, --source-scene-plugin <SOURCE_SCENE_PLUGIN>
          Video Source Plugin for obtaining the scene file [default: bestsource] [possible values: lsmash, bestsource]
  -c, --crf-data-file <CRF_DATA_FILE>
          Path to save the updated crf data
  -c, --crop <CROP>
          Crop string (e.g. 1920:816:0:132)
      --downscale <DOWNSCALE>
          Downscale, using Box Kernel 0.5 [default: false] [possible values: true, false]
      --detelecine <DETELECINE>
          Removes telecine — a process used to convert 24fps film to 29.97fps video using a 3:2 pulldown pattern [default: false] [possible values: true, false]
  -v, --verbose
          
      --filter-frames
          Avoid encoding frames that have already reached the quality score
      --model <MODEL>
          Path to custom ONNX model (default: uses embedded TransNetV2 model)
      --extra-split-sec <EXTRA_SPLIT_SEC>
          If both `--extra-split` (frames) and `--extra-split-sec` are provided, frames take priority [default: 10]
      --extra-split <EXTRA_SPLIT>
          Maximum scene length. When a scenecut is found whose distance to the previous scenecut is greater than the value specified by this option, one or more extra splits (scenecuts) are added. Set this option to 0 to disable adding extra splits
      --min-scene-len-sec <MIN_SCENE_LEN_SEC>
          Minimum number of frames for a scenecut. Only supported with transnetv2 scene method [default: 1]
      --min-scene-len <MIN_SCENE_LEN>
          Minimum number of frames for a scenecut
      --threshold <THRESHOLD>
          Threshold to detect scene cut [default: 0.5]
  -h, --help
          Print help
  -V, --version
          Print version

```

## simple-ssimu2

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

## transnetv2-rs

```sh
transnetv2-rs --help
Scene detection using TransnetV2

Usage: transnetv2-rs [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>
          Path to the video file
  -o, --output <OUTPUT>
          Path to the scenes JSON output file (default: "[SCENES]_<input>.json" if no path given)
      --model <MODEL>
          Path to custom ONNX model (default: uses embedded TransNetV2 model)
      --extra-split-sec <EXTRA_SPLIT_SEC>
          If both `--extra-split` (frames) and `--extra-split-sec` are provided, frames take priority [default: 5]
      --extra-split <EXTRA_SPLIT>
          Maximum scene length. When a scenecut is found whose distance to the previous scenecut is greater than the value specified by this option, one or more extra splits (scenecuts) are added. Set this option to 0 to disable adding extra splits
      --min-scene-len-sec <MIN_SCENE_LEN_SEC>
          Minimum number of frames for a scenecut [default: 1]
      --min-scene-len <MIN_SCENE_LEN>
          Minimum number of frames for a scenecut
      --threshold <THRESHOLD>
          Threshold to detect scene cut [default: 0.5]
      --cpu
          Skip GPU acceleration
  -t, --temp <TEMP>
          Temp folder (default: "[Temp]_<input>" if no temp folder given)
  -s, --source-plugin <SOURCE_PLUGIN>
          Video Source Plugin for obtaining the scene file [default: lsmash] [possible values: lsmash, bestsource]
  -v, --verbose
          
  -c, --crop <CROP>
          Crop string (e.g. 1920:816:0:132)
      --downscale <DOWNSCALE>
          Downscale, using Box Kernel 0.5 [default: false] [possible values: true, false]
      --detelecine <DETELECINE>
          Removes telecine — a process used to convert 24fps film to 29.97fps video using a 3:2 pulldown pattern [default: false] [possible values: true, false]
      --color-metadata <COLOR_METADATA>
          Color params base on the svt-av1 params [default: "--color-primaries bt709 --transfer-characteristics bt709 --matrix-coefficients bt709 --color-range studio --chroma-sample-position left"]
  -k, --keep-files
          Keep temporary files (disables automatic cleanup)
  -h, --help
          Print help
  -V, --version
          Print version
```
