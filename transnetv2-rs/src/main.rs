use clap::{ArgAction, Parser};
use encoding_utils_lib::vapoursynth::SourcePlugin;
use eyre::OptionExt;
use transnetv2_rs::transnet::run_transnetv2;
use std::{fs, path::{absolute, PathBuf}};

/// Video processing tool with scene detection
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the video file
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    input: PathBuf,

    /// Path to the scenes JSON output file (default: "[SCENES]_<input>.json" if no path given)
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    output: Option<PathBuf>,

    /// Path to custom ONNX model (default: uses embedded TransNetV2 model)
    #[arg(long, value_parser = clap::value_parser!(PathBuf))]
    model: Option<PathBuf>,

    // Maximum scene length in seconds. 
    /// If both `--extra-split` (frames) and `--extra-split-sec` are provided, frames take priority.
    #[arg(long = "extra-split-sec", default_value_t = 5, value_parser = clap::value_parser!(u32).range(0..))]
    extra_split_sec: u32,


    /// Maximum scene length. 
    /// When a scenecut is found whose distance to the previous scenecut is greater than the value specified by this option, one or more extra splits (scenecuts) are added. Set this option to 0 to disable adding extra splits.
    #[arg(long = "extra-split", value_parser = clap::value_parser!(u32).range(0..))]
    extra_split: Option<u32>,

    /// Minimum number of frames for a scenecut.
    #[arg(long = "min-scene-len-sec", default_value_t = 1, value_parser = clap::value_parser!(u32).range(0..))]
    min_scene_len_sec: u32,

    /// Minimum number of frames for a scenecut.
    #[arg(long = "min-scene-len", value_parser = clap::value_parser!(u32).range(0..))]
    min_scene_len: Option<u32>,

    /// Threshold to detect scene cut
    #[arg(long = "threshold", default_value_t = 0.5)]
    threshold: f32,

    /// Skip GPU acceleration
    #[arg(long, action = ArgAction::SetTrue, default_value_t = true)]
    cpu: bool,

    /// Temp folder (default: "[Temp]_<input>" if no temp folder given)
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    temp: Option<PathBuf>,

    /// Video Source Plugin for obtaining the scene file
    #[arg(short, long = "source-plugin", default_value = "lsmash")]
    source_plugin: SourcePlugin,

    // Enable verbose output
    #[arg(short, long, action = ArgAction::SetTrue, default_value_t = false)]
    verbose: bool,

    /// Crop string (e.g. 1920:816:0:132)
    #[arg(short, long)]
    crop: Option<String>,

    /// Downscale, using Box Kernel 0.5
    #[arg(
        long, 
        default_value_t = false,
        action = ArgAction::Set,
        value_parser = clap::value_parser!(bool)
    )]
    downscale: bool,

    /// Removes telecine — a process used to convert 24fps film to 29.97fps video using a 3:2 pulldown pattern.
    #[arg(
        long, 
        default_value_t = false,
        action = ArgAction::Set,
        value_parser = clap::value_parser!(bool)
    )]
    detelecine: bool,

    /// Color params base on the svt-av1 params
    #[arg(
    long,
        default_value = "--color-primaries bt709 --transfer-characteristics bt709 --matrix-coefficients bt709 --color-range studio --chroma-sample-position left"
    )]
    color_metadata: String,

    /// Keep temporary files (disables automatic cleanup)
    #[arg(
        short = 'k', 
        long = "keep-files",
        action = ArgAction::SetTrue,
        default_value_t = false,
    )]
    keep_files: bool,
}

fn main() -> eyre::Result<()> {
    let args = Args::parse();
    let input_path = absolute(&args.input)?;

    let scenes = match args.output {
        Some(path) => path,
        None => {
            let output_name = format!(
                "[SCENES]_{}.json",
                args.input
                    .file_stem()
                    .ok_or_eyre("No file name")?
                    .to_str()
                    .ok_or_eyre("Invalid UTF-8 in input path")?
            );
            input_path.with_file_name(output_name)
        }
    };

    let temp_folder = match args.temp {
        Some(temp) => temp, 
        None => { 
            let temp_folder = args.input.with_file_name(format!(
                "[TEMP]_{}",
                args.input
                    .file_stem()
                    .ok_or_eyre("No file name")?
                    .to_str()
                    .ok_or_eyre("Invalid UTF-8 in input path")?
            ));
            args.input.with_file_name(temp_folder)
        }
    };
    fs::create_dir_all(&temp_folder)?;

    run_transnetv2(&input_path, &scenes, args.model.as_deref(),
     args.cpu, args.source_plugin, &temp_folder, args.verbose, &args.color_metadata, args.crop.as_deref(), args.downscale, args.detelecine, args.extra_split_sec.into(), args.extra_split.map(|x| x.into()),  args.min_scene_len_sec.into(), args.min_scene_len, args.threshold)?;

     if !args.keep_files && fs::exists(&temp_folder)? {
     fs::remove_dir_all(&temp_folder)?;
    }

    Ok(())
}
