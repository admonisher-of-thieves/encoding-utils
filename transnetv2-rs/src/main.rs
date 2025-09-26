use clap::{ArgAction, Parser};
use encoding_utils_lib::{transnetv2::transnet::run_transnetv2, vapoursynth::SourcePlugin};
use eyre::OptionExt;
use vapoursynth4_rs::core::Core;
use std::{fs, path::{absolute, PathBuf}};

/// Scene detection using TransnetV2
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
    #[arg(long = "extra-split-sec", default_value_t = 10, value_parser = clap::value_parser!(u32).range(0..))]
    extra_split_sec: u32,

    /// Maximum scene length. 
    /// When a scenecut is found whose distance to the previous scenecut is greater than the value specified by this option, one or more extra splits (scenecuts) are added. Set this option to 0 to disable adding extra splits.
    #[arg(long = "extra-split", value_parser = clap::value_parser!(u32).range(0..))]
    extra_split: Option<u32>,

    // Scene length cut in seconds for fades. Add a fade split (if the fade exists) when the scene is bigger than this value.
    /// If both `--extra-splits-fades` (frames) and `--extra-split-sec-fades` are provided, frames take priority.
    #[arg(long = "extra-split-sec-fades", default_value_t = 10, value_parser = clap::value_parser!(u32).range(0..))]
    extra_split_sec_fades: u32,

    /// Scene length cut in seconds for fades. Add a fade split (if the fade exists) when the scene is bigger than this value.
    #[arg(long = "extra-split-fades", value_parser = clap::value_parser!(u32).range(0..))]
    extra_split_fades: Option<u32>,

    /// Minimum number of frames for a scenecut.
    #[arg(long = "min-scene-len-sec", default_value_t = 1, value_parser = clap::value_parser!(u32).range(0..))]
    min_scene_len_sec: u32,

    /// Minimum number of frames for a scenecut.
    #[arg(long = "min-scene-len", value_parser = clap::value_parser!(u32).range(0..))]
    min_scene_len: Option<u32>,

    /// Threshold to detect scene cut
    #[arg(long = "threshold", default_value_t = 0.4)]
    threshold: f32,

    /// Combine hardcut scenes and fade scenes
    #[arg(
        long = "enable-fade",
        action = ArgAction::SetTrue,
        default_value_t = true,
    )]
    enable_fade_detection: bool,

    /// Threshold to fade detection
    #[arg(long = "fade-threshold", default_value_t = 0.05)]
    fade_threshold: f32,

    /// Minimum fade length in frames
    #[arg(long = "min-fade-len", default_value_t = 5,  value_parser = clap::value_parser!(u32).range(0..))]
    min_fade_len: u32,

    /// Merge fades separated by this many frames or less
    #[arg(long = "merge-gap-between-fades", default_value_t = 4, value_parser = clap::value_parser!(u32).range(0..))]
    merge_gap_between_fades: u32,

    /// Skip GPU acceleration
    #[arg(long, action = ArgAction::SetTrue, default_value_t = false)]
    cpu: bool,

    /// Temp folder (default: "[Temp]_<input>" if no temp folder given)
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    temp: Option<PathBuf>,

    /// Video Source Plugin for obtaining the scene file
    #[arg(short, long = "source-plugin", default_value = "ffms2")]
    source_plugin: SourcePlugin,

    // Enable verbose output
    #[arg(short, long, action = ArgAction::SetTrue, default_value_t = false)]
    verbose: bool,

    /// Crop string (e.g. 1920:816:0:132)
    #[arg(short, long)]
    crop: Option<String>,

    /// Trim source file. Format Start:End. Examples: 1261:5623, 0:2432, 2352:-1. 
    #[arg(short, long)]
    trim: Option<String>,

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

    /// Get [PREDICTIONS]_{input}.csv file
    #[arg(
        long = "scene-predictions",
        action = ArgAction::SetTrue,
        default_value_t = false,
    )]
    scene_predictions: bool,

    /// Get [HARDCUTS-SCENE]_{input}.json file
    #[arg(
        long = "hardcut-scenes",
        action = ArgAction::SetTrue,
        default_value_t = false,
    )]
    hardcut_scenes: bool,
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

    let indexes_folder = temp_folder.join("indexes");
    fs::create_dir_all(&indexes_folder)?;

    let core = Core::builder().build();

    let (scene_list, hardcut_list) = run_transnetv2(
        &core,
        &input_path,
        args.model.as_deref(),
        args.cpu,
        args.source_plugin,
        &indexes_folder,
         args.verbose,
        &args.color_metadata,
        args.crop.as_deref(),
        args.trim.as_deref(),
        args.detelecine,
        args.extra_split_sec.into(),
        args.extra_split.map(|x| x.into()),
        args.extra_split_sec_fades.into(),
        args.extra_split_fades.map(|x| x.into()),
        args.min_scene_len_sec.into(),
        args.min_scene_len.map(|x| x.into()),
        args.threshold,
        args.fade_threshold,
        args.min_fade_len.into(),
        args.merge_gap_between_fades.into(),
        args.enable_fade_detection,
        args.scene_predictions
    )?;

    scene_list.write_scene_list_to_file( &scenes)?;

    if args.hardcut_scenes {
        let output_name = format!(
            "[HARDCUT-SCENES]_{}.json",
            args.input
                .file_stem()
                .ok_or_eyre("No file name")?
                .to_str()
                .ok_or_eyre("Invalid UTF-8 in input path")?
        );
        let hardcut_path = input_path.with_file_name(output_name);
        hardcut_list.write_scene_list_to_file(&hardcut_path)?;
    }


     if !args.keep_files && fs::exists(&temp_folder)? {
     fs::remove_dir_all(&temp_folder)?;
    }

    Ok(())
}
