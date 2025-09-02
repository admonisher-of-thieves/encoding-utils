use clap::{ArgAction, Parser};
use eyre::{OptionExt, Result};
use encoding_utils_lib::{crf::crf_parser, frame_loop::run_frame_loop, scenes::{FramesDistribution, SceneDetectionMethod}, vapoursynth::SourcePlugin};

use std::{fs, path::{absolute, PathBuf}};

/// Scene-based boost that dynamically adjusts CRF.
/// It creates a scene-file with zone overrides
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input video file, you can also pass a .vpy script
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    input: PathBuf,

    /// Output scene file (default: "[BOOST]_<input>.json" if no output given)
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    output: Option<PathBuf>,

    /// Temp folder (default: "[Temp]_<input>" if no temp folder given)
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    temp: Option<PathBuf>,

    /// AV1an encoding parameters
    #[arg(
        long,
        default_value = "--verbose --resume --workers 1 --concat mkvmerge --chunk-method bestsource --chunk-order sequential --encoder svt-av1 --no-defaults --split-method none --extra-split-sec 0 --min-scene-len 0"
    )]
    av1an_params: String,

    /// SVT-AV1 encoder parameters
    #[arg(
    long,
        default_value = "--preset 2 --tune 1 --keyint 0 --film-grain 0 --scm 0 --scd 0 --hbd-mds 1 --psy-rd 1.0 --complex-hvs 1 --spy-rd 2 --enable-qm 1 --qm-min 8 --qm-max 15 --chroma-qm-min 8 --chroma-qm-max 15 --luminance-qp-bias 20 --enable-tf 1 --tf-strength 2 --alt-tf-decay 1 --kf-tf-strength 0 --filtering-noise-detection 2 --enable-cdef 1 --enable-restoration 1 --enable-dlf 2 --enable-variance-boost 1 --variance-boost-strength 2 --variance-octile 6 --qp-scale-compress-strength 4.0 --low-q-taper 1 --noise-norm-strength 1 --adaptive-film-grain 0 --film-grain-denoise 0 --rc 0 --aq-mode 2 --sharpness 1 --sharp-tx 1 --tile-columns 1 --input-depth 10 --color-primaries bt709 --transfer-characteristics bt709 --matrix-coefficients bt709 --color-range studio --chroma-sample-position left"
    )]
    encoder_params: String,

    /// Target SSIMULACRA2 score (0-100)
    #[arg(short = 'q', long, default_value_t = 77.0)]
    target_quality: f64,

    /// Min SSIMULACRA2 score (0-100). All scores are going to be above the min-q when selecting a crf value.
    #[arg(long = "min-q", default_value_t = 74.0)]
    min_target_quality: f64,

    /// Percentile (0-100). 20 means that 80 percent of all values in a scene will be above target-quality when selecting a crf value.
    #[arg(short = 'p', long, default_value_t = 50)]
    target_percentile: u8,

    /// Target CRF value(s) (1.0-70.0). Can be:
    /// - Single value (35 or 35.5)
    /// - Comma-separated list (35,27.2,21)
    /// - Backward range (36..21 or 36.0..21.0)
    /// - Stepped backward range (36..21:1.5 or 36.0..21.0:1.5)
    #[arg(
        short = 'c',
        long,
        default_value = "35,30,27,24,21,18",
    )]
    crf: String,
    /// Number of frames to encode for scene. Higher value increase the confidence than all the frames in the scene will be above your quality target at cost of encoding time
    #[arg(short = 'n', long = "n-frames", value_parser = clap::value_parser!(u32).range(1..))]
    n_frames: Option<u32>,

    /// Number of seconds to encode for scene. Higher value increase the confidence than all the frames in the scene will be above your quality target at cost of encoding time
    #[arg(short = 's', long = "s-frames", default_value_t = 0.5)]
    s_frames: f64,

    /// XML Chapters file. Used for zoning.
    #[arg(long, value_parser = clap::value_parser!(PathBuf))]
    chapters: Option<PathBuf>,

    /// Zoning by chapters. {Chapter}:{CRF} (e.g. Opening:21,Ending:35,Episode:24)
    #[arg(short = 'z', long = "chapters-zoning", default_value = "")]
   chapters_zoning: String,

    /// Workers to use when encoding
    #[arg(short = 'w', long, default_value_t = 4, value_parser = clap::value_parser!(u32).range(1..))]
    workers: u32,

    /// How the frames are distributed when encoding
    #[arg(value_enum, short = 'd', long = "frames-distribution", default_value_t = FramesDistribution::Evenly)]
    frames_distribution: FramesDistribution,

    /// Velocity tuning preset (-1~13)
    #[arg(short = 'v', long, default_value_t = 7, value_parser = clap::value_parser!(i32).range(-1..=13))]
    velocity_preset: i32,

    /// Which method to use to calculate scenes
    #[arg(value_enum, short = 'd', long = "scene-detection-method", default_value_t = SceneDetectionMethod::TransnetV2)]
    scene_detection_method: SceneDetectionMethod,

    /// Keep temporary files (disables automatic cleanup)
    #[arg(
        short = 'k', 
        long = "keep-files",
        action = ArgAction::SetTrue,
        default_value_t = false,
    )]
    keep_files: bool,

    /// Disable overwrite protection (remove the scene file)
    #[arg(
        short = 'f', 
        long = "force",
        action = ArgAction::SetTrue,
        default_value_t = true,
    )]
    force: bool,

    /// Video Source Plugin for metrics
    #[arg(short, long = "source-metric-plugin", default_value = "ffms2")]
    source_metric_plugin: SourcePlugin,
    
    /// Video Source Plugin for encoding
    #[arg(short, long = "source-encoding-plugin", default_value = "ffms2")]
    source_encoding_plugin: SourcePlugin,

    /// Video Source Plugin for obtaining the scene file
    #[arg(short, long = "source-scene-plugin", default_value = "bestsource")]
    source_scene_plugin: SourcePlugin,

    /// Path to save the updated crf data
    #[arg(short, long = "crf-data-file")]
    crf_data_file: Option<PathBuf>,

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

    // Enable verbose output
    #[arg(long, action = ArgAction::SetTrue, default_value_t = false)]
    verbose: bool,

    // Enable verbose output
    #[arg(long = "verbose-verbose", action = ArgAction::SetTrue, default_value_t = false)]
    verbose_verbose: bool,

    // Enable verbose output
    #[arg(long = "verbose-verbose-verbose", action = ArgAction::SetTrue, default_value_t = false)]
    verbose_verbose_verbose: bool,

    /// Avoid encoding frames that have already reached the quality score
    #[arg(
        long = "filter-frames",
        action = ArgAction::SetTrue,
        default_value_t = true,
    )]
    filter_frames: bool,

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
    #[arg(long = "min-fade-len", default_value_t = 5)]
    min_fade_len: u32,

    /// Merge fades separated by this many frames or less
    #[arg(long = "merge-gap-between-fades", default_value_t = 4)]
    merge_gap_between_fades: u32,

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

fn main() -> Result<()> {
    let args = Args::parse();

    let crf_values = crf_parser(&args.crf)?;
    let input_path = absolute(&args.input)?;
    let scene_boosted = match args.output {
        Some(output) => output, 
        None => { 
            let output_name = format!(
                "[BOOST]_{}.json",
                args.input
                    .file_stem()
                    .ok_or_eyre("No file name")?
                    .to_str()
                    .ok_or_eyre("Invalid UTF-8 in input path")?
            );
            args.input.with_file_name(output_name)
        }
    };

    if scene_boosted.exists() {
        if args.force {
            fs::remove_file(&scene_boosted)?;
            println!("\nRemoved existing scene file: {}", scene_boosted.display());
        } else {
            eyre::bail!(
                "Scene file {} already exists. Remove use --force to overwrite",
                scene_boosted.display()
            );
        }
    }

    let temp_folder = match args.temp {
        Some(temp) => temp, 
        None => {
            args.input.with_file_name(format!(
                "[TEMP]_{}",
                args.input
                    .file_stem()
                    .ok_or_eyre("No file name")?
                    .to_str()
                    .ok_or_eyre("Invalid UTF-8 in input path")?
            ))
        }
    };

    fs::create_dir_all(&temp_folder)?;

    run_frame_loop(
        &input_path,
        &scene_boosted,
        &args.av1an_params,
        &args.encoder_params,
        &crf_values,
        args.target_quality,
        args.min_target_quality,
        args.velocity_preset,
        args.n_frames,
        args.s_frames,
        args.frames_distribution,
        args.scene_detection_method,
        args.filter_frames,
        args.chapters.as_deref(),
        args.chapters_zoning,
        args.workers,
        &args.source_metric_plugin,
        &args.source_encoding_plugin,
        &args.source_scene_plugin,
        args.crf_data_file.as_deref(),
        args.crop.as_deref(),
        args.downscale,
        args.detelecine,
        !args.keep_files,
        args.verbose,
        args.verbose_verbose,
        args.verbose_verbose_verbose,
        &temp_folder,
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
        args.scene_predictions,
        args.target_percentile,
        args.hardcut_scenes,
    )?;

    Ok(())
}


