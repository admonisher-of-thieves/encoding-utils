# transnetv2-rs

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
