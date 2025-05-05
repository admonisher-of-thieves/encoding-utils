# Encoding utils

## frame-boost

```sh
Scene-based boost that dynamically adjusts CRF. It creates a scene-file with zone overrides

Usage: frame-boost [OPTIONS] --input <INPUT>

Options:
  -i, --input <INPUT>
          Input video file
      --av1an-params <AV1AN_PARAMS>
          AV1an encoding parameters [default: "--verbose --workers 4 --concat mkvmerge --chunk-method bestsource --encoder svt-av1"]
      --encoder-params <ENCODER_PARAMS>
          SVT-AV1 encoder parameters [default: "--preset 2 --crf 21~36 --tune 2 --keyint -1 --input-depth 10 --color-primaries bt709 --transfer-characteristics bt709 --matrix-coefficients bt709 --color-range studio"]
  -q, --target-quality <TARGET_QUALITY>
          Target SSIMULACRA2 score (0-100) [default: 80]
  -p, --velocity-preset <VELOCITY_PRESET>
          Velocity tuning preset (-1~13) [default: 4]
  -s, --step <STEP>
          Frame processing step (1 = every frame) [default: 3]
  -k, --keep-files
          Keep temporary files (disables automatic cleanup)
  -F, --no-force
          Disable overwrite protection (remove the scene file)
  -v, --verbose
          
  -h, --help
          Print help
  -V, --version
          Print version
```

## simple-ssimu2

```sh
Calculate SSIMULACRA2 metric

Usage: simple-ssimu2 [OPTIONS] --reference <REFERENCE> --distorted <DISTORTED>

Options:
  -r, --reference <REFERENCE>  Reference video file
  -d, --distorted <DISTORTED>  Distorted video file (encoded version)
  -S, --scenes <SCENES>        JSON file containing scene information
  -s, --step <STEP>            Frame step value (process every N-th frame) [default: 1]
  -o, --only-stats             Disabel verbose output - Print only stats
  -h, --help                   Print help
  -V, --version                Print version
```
