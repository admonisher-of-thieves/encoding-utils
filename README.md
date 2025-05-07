# Encoding utils

## frame-boost

```sh
Scene-based boost that dynamically adjusts CRF. It creates a scene-file with zone overrides

Usage: frame-boost [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>
          Input video file
  -o, --output <OUTPUT>
          Output scene file (default: "[BOOST] <input>.json" if no output given)
      --av1an-params <AV1AN_PARAMS>
          AV1an encoding parameters [default: "--verbose --workers 4 --concat mkvmerge --chunk-method bestsource --encoder svt-av1 --split-method av-scenechange --sc-method standard --extra-split 120 --min-scene-len 24"]
      --encoder-params <ENCODER_PARAMS>
          SVT-AV1 encoder parameters [default: "--preset 2 --tune 2 --keyint -1 --input-depth 10 --color-primaries bt709 --transfer-characteristics bt709 --matrix-coefficients bt709 --color-range studio"]
  -q, --target-quality <TARGET_QUALITY>
          Target SSIMULACRA2 score (0-100) [default: 75]
  -c, --crf <CRF>
          Target CRF value(s) (1-70). Can be: - Single value (35) - Comma-separated list (21,27,35) - Range (21..36) - Stepped range (21..36:3) [default: 21,24,27,30,33,36]
  -p, --velocity-preset <VELOCITY_PRESET>
          Velocity tuning preset (-1~13) [default: 4]
  -k, --keep-files
          Keep temporary files (disables automatic cleanup)
  -F, --no-force
          Disable overwrite protection (remove the scene file)
  -m, --metric-importer <METRIC_IMPORTER_PLUGIN>
          Importer plugin, only for the calculates of ssimu2 [default: lsmash] [possible values: lsmash, bestsource]
  -v, --verbose
          
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
  -s, --step <STEP>
          Frame step value (process every N-th frame) [default: 1]
  -o, --only-stats
          Disable verbose output - Print only stats
  -i, --importer-plugin <IMPORTER_PLUGIN>
          Importer plugin [default: lsmash] [possible values: lsmash, bestsource]
  -o, --output <OUTPUT>
          Path to output file (if not provided, stats will only be printed)
  -t, --trim <TRIM>
          Trim to sync video: format is "first,last,clip" Example: "6,18,distorted" or "6,18,d"
  -m, --middle-frames
          Allows you to use a distorted video composed of middle frames. Needs scenes file
  -h, --help
          Print help
  -V, --version
          Print version
```

## vpy-files

```sh
Tool to create VapourSynth filter script

Usage: vpy-files [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>        Input video file
  -o, --output <OUTPUT>      Output file (default: "[VPY] <input>.vpy" if no input given)
  -c, --crop <CROP>          Crop string (e.g. 1920:816:0:132)
  -s, --scale <SCALE>        Scale expression (e.g. zscale=1920:-1:filter=lanczos)
  -P, --importer <IMPORTER>  Importer plugin (lsmash or bestsource) [default: lsmash] [possible values: lsmash, bestsource]
  -O, --overwrite            Overwrite output file if it exists
  -h, --help                 Print help
  -V, --version              Print version
```
