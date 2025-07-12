use clap::{ArgAction, Parser};
use eyre::{Context, OptionExt, Result, eyre};
use encoding_utils_lib::{main_loop::run_loop, scenes::{FramesDistribution, SceneDetectionMethod}, vapoursynth::SourcePlugin};

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
        default_value = "--verbose --workers 2 --concat mkvmerge --chunk-method bestsource --encoder svt-av1 --no-defaults"
    )]
    av1an_params: String,

    /// SVT-AV1 encoder parameters
    #[arg(
    long,
        default_value = "--preset 2 --tune 2 --keyint -1 --film-grain 0 --scm 0 --hbd-mds 1 --tile-columns 1 --enable-qm 1 --qm-min 8 --luminance-qp-bias 20  --kf-tf-strength 0 --psy-rd 1 --spy-rd 2 --complex-hvs 1 --input-depth 10 --color-primaries bt709 --transfer-characteristics bt709 --matrix-coefficients bt709 --color-range studio --chroma-sample-position left"
    )]
    encoder_params: String,

    /// Target SSIMULACRA2 score (0-100)
    #[arg(short = 'q', long, default_value_t = 81.0)]
    target_quality: f64,

    /// Target CRF value(s) (70-1). Can be:
    /// - Single value (35)
    /// - Comma-separated list (35,27,21)
    /// - Range (36..21)
    /// - Stepped range (36..21:3)
    #[arg(
        short = 'c',
        long,
        default_value = "35,30,27,24,21",
    )]
    crf: String,

    /// Number of frames to encode for scene. Higher value increase the confidence than all the frames in the scene will be above your quality target at cost of encoding time
    #[arg(short = 'n', long = "n-frames", default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..))]
    n_frames: u32,

    /// Workers to use when encoding
    #[arg(short = 'w', long, default_value_t = 2, value_parser = clap::value_parser!(u32).range(1..))]
    workers: u32,

    /// How the frames are distributed when encoding
    #[arg(value_enum, short = 'd', long = "frames-distribution", default_value_t = FramesDistribution::Center)]
    frames_distribution: FramesDistribution,

    /// Velocity tuning preset (-1~13)
    #[arg(short = 'p', long, default_value_t = 4, value_parser = clap::value_parser!(i32).range(-1..=13))]
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
    #[arg(short, long = "source-metric-plugin", default_value = "lsmash")]
    source_metric_plugin: SourcePlugin,
    
    /// Video Source Plugin for encoding
    #[arg(short, long = "source-encoding-plugin", default_value = "lsmash")]
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
    #[arg(short, long, action = ArgAction::SetTrue, default_value_t = false)]
    verbose: bool,

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

    /// Minimum number of frames for a scenecut. Only supported with transnetv2 scene method.
    #[arg(long = "min-scene-len-sec", default_value_t = 1, value_parser = clap::value_parser!(u32).range(0..))]
    min_scene_len_sec: u32,

    /// Minimum number of frames for a scenecut. 
    #[arg(long = "min-scene-len", value_parser = clap::value_parser!(u32).range(0..))]
    min_scene_len: Option<u32>,

    /// Threshold to detect scene cut
    #[arg(long = "threshold", default_value_t = 0.5)]
    threshold: f32,

    /// Threshold to fade detection
    #[arg(long = "fade-threshold", default_value_t = 0.05)]
    fade_threshold: f32,

    /// Minimum fade length in frames
    #[arg(long = "min-fade-len", default_value_t = 8)]
    min_fade_len: u32,

    /// Merge fades separated by this many frames or less
    #[arg(long = "merge-gap-between-fades", default_value_t = 4)]
    merge_gap_between_fades: u32,
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

    run_loop(
        &input_path,
        &scene_boosted,
        &args.av1an_params,
        &args.encoder_params,
        &crf_values,
        args.target_quality,
        args.velocity_preset,
        args.n_frames,
        args.frames_distribution,
        args.scene_detection_method,
        args.filter_frames,
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
        &temp_folder,
        args.extra_split_sec.into(),
        args.extra_split.map(|x| x.into()),
        args.min_scene_len_sec.into(),
        args.min_scene_len.map(|x| x.into()),
        args.threshold,
          args.fade_threshold,
        args.min_fade_len.try_into().unwrap(),
        args.merge_gap_between_fades.try_into().unwrap()
    )?;

    Ok(())
}


/// Enhanced CRF parser that enforces strictly descending values
/// Supported formats:
/// - Single values (35) → [35]
/// - Comma-separated lists (35,27,21) → [35, 27, 21]
/// - Backward ranges (36..21) → [36, 35, ..., 21]
/// - Stepped backward ranges (36..21:3) → [36, 33, 30, ..., 21]
pub fn crf_parser(s: &str) -> Result<Vec<u8>> {
    // Parse the raw values first
    let values = parse_raw_crf_values(s)?;
    
    // Validate descending order
    validate_descending(&values).wrap_err_with(|| {
        format!("CRF values must be in strictly descending order (got {values:?})")
    })?;
    
    Ok(values)
}

/// Core parsing logic
fn parse_raw_crf_values(s: &str) -> Result<Vec<u8>> {
    const CRF_RANGE: std::ops::RangeInclusive<u8> = 1..=70;
    
    let validate_crf = |value: u8| {
        if !CRF_RANGE.contains(&value) {
            Err(eyre!("CRF must be between {}-{} (got {})", 
                CRF_RANGE.start(), CRF_RANGE.end(), value))
        } else {
            Ok(value)
        }
    };

    // Handle stepped ranges (36..21:3)
    if let Some((range_part, step)) = s.split_once(':') {
        if let Some((start, end)) = range_part.split_once("..") {
            let (start, end, step) = (
                start.parse().wrap_err_with(|| format!("Invalid range start: '{start}'"))?,
                end.parse().wrap_err_with(|| format!("Invalid range end: '{end}'"))?,
                step.parse().wrap_err_with(|| format!("Invalid step value: '{step}'"))?,
            );
            
            if start < end {
                return Err(eyre!(
                    "Backward range requires start >= end (got {start}..{end})"
                ));
            }
            if step == 0 {
                return Err(eyre!("Step value must be positive"));
            }

            let mut values = Vec::new();
            let mut current = start;
            while current >= end {
                values.push(validate_crf(current)?);
                current = current.saturating_sub(step);
            }
            return Ok(values);
        }
    }

    // Handle simple ranges (36..21)
    if let Some((start, end)) = s.split_once("..") {
        let (start, end) = (
            start.parse().wrap_err_with(|| format!("Invalid range start: '{start}'"))?,
            end.parse().wrap_err_with(|| format!("Invalid range end: '{end}'"))?,
        );

        if start < end {
            return Err(eyre!(
                "Backward range requires start >= end (got {start}..{end})"
            ));
        }

        return (end..=start)
            .rev()
            .map(validate_crf)
            .collect();
    }

    // Handle comma-separated or single value
    s.split(',')
        .map(|part| {
            part.trim()
                .parse()
                .wrap_err_with(|| format!("Invalid CRF value: '{}'", part.trim()))
                .and_then(validate_crf)
        })
        .collect()
}

/// Validate strict descending order
fn validate_descending(values: &[u8]) -> Result<()> {
    if values.windows(2).any(|pair| pair[0] <= pair[1]) {
        Err(eyre!("Sequence contains non-descending values"))
    } else {
        Ok(())
    }
}
#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Args::command().debug_assert();
}