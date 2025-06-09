use clap::{ArgAction, Parser};
use eyre::{Context, OptionExt, Result, eyre};
use encoding_utils_lib::{main_loop::{run_loop, FramesDistribution}, vapoursynth::SourcePlugin};

use std::{fs, path::{absolute, PathBuf}};

/// Scene-based boost that dynamically adjusts CRF.
/// It creates a scene-file with zone overrides
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input video file
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    input: PathBuf,

    /// Output scene file (default: "[BOOST]_<input>.json" if no output given)
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    output: Option<PathBuf>,

    /// Temp folder (default: "[Temp]_[FRAME-BOOST]_<input>" if no temp folder given)
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    temp: Option<PathBuf>,

    /// AV1an encoding parameters
    #[arg(
        long,
        default_value = "--verbose --workers 2 --concat mkvmerge --chunk-method bestsource --encoder svt-av1 --split-method av-scenechange --sc-method standard --extra-split 120 --min-scene-len 24"
    )]
    av1an_params: String,

    /// SVT-AV1 encoder parameters
    #[arg(
    long,
        default_value = "--preset 4 --tune 2 --keyint -1 --hbd-mds 1 --input-depth 10 --color-primaries bt709 --transfer-characteristics bt709 --matrix-coefficients bt709 --color-range studio --chroma-sample-position left"
    )]
    encoder_params: String,

    /// Target SSIMULACRA2 score (0-100)
    #[arg(short = 'q', long, default_value_t = 75.0)]
    target_quality: f64,

    /// Target CRF value(s) (1-70). Can be:
    /// - Single value (35)
    /// - Comma-separated list (21,27,35)
    /// - Range (21..36)
    /// - Stepped range (21..36:3)
    #[arg(
        short = 'c',
        long,
        default_value = "18,21,24,27,30,33,35",
    )]
    crf: String,

    /// Number of frames to encode for scene. Higher value increase the confidence than all the frames in the scene will be above your quality target at cost of encoding time
    #[arg(short = 'n', long = "n-frames", default_value_t = 3, value_parser = clap::value_parser!(u32).range(1..))]
    n_frames: u32,

    /// How the frames are distributed when encoding
    #[arg(value_enum, short = 'd', long = "frames-distribution", default_value_t = FramesDistribution::Center)]
    frames_distribution: FramesDistribution,

    /// Velocity tuning preset (-1~13)
    #[arg(short = 'p', long, default_value_t = 4, value_parser = clap::value_parser!(i32).range(-1..=13))]
    velocity_preset: i32,

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
        short = 'F', 
        long = "no-force",
        action = ArgAction::SetTrue,
        default_value_t = false,
    )]
    no_force: bool,

    /// Video Source Plugin for metrics and encoding frames
    #[arg(short, long = "source-plugin", default_value = "lsmash")]
    source_plugin: SourcePlugin,

    /// Path to save the updated crf data
    #[arg(short, long = "crf-data-file")]
    crf_data_file: Option<PathBuf>,

    /// Crop string (e.g. 1920:816:0:132)
    #[arg(short, long)]
    crop: Option<String>,

    /// Downscale, using Box Kernel 0.5
    #[arg(
        short, 
        long, 
        default_value_t = false,
        action = ArgAction::Set,
        value_parser = clap::value_parser!(bool)
    )]
    downscale: bool,

    // Enable verbose output
    #[arg(short, long, action = ArgAction::SetTrue, default_value_t = false)]
    verbose: bool,

    /// Avoid encoding frames that have already reached the quality score
    #[arg(
        short = 'f', 
        long = "filter-frames",
        action = ArgAction::SetTrue,
        default_value_t = true,
    )]
    filter_frames: bool,
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
        if !args.no_force {
            fs::remove_file(&scene_boosted)?;
            println!("\nRemoved existing scene file: {}", scene_boosted.display());
        } else {
            eyre::bail!(
                "Scene file {} already exists. Remove -F(--no-force) to overwrite",
                scene_boosted.display()
            );
        }
    }

    let temp_folder = match args.temp {
        Some(temp) => temp, 
        None => { 
            let temp_folder = args.input.with_file_name(format!(
                "[Temp]_[FRAME-BOOST]_{}",
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
        args.filter_frames,
        &args.source_plugin,
        args.crf_data_file.as_deref(),
        args.crop.as_deref(),
        args.downscale,
        !args.keep_files,
        args.verbose,
        &temp_folder
    )?;

    Ok(())
}



/// Enhanced CRF parser supporting:
/// - Single values (35)
/// - Comma-separated lists (21,27,35)
/// - Simple ranges (21..36)
/// - Stepped ranges (21..36:3)
fn crf_parser(s: &str) -> Result<Vec<u8>> {
    const CRF_RANGE: std::ops::RangeInclusive<u8> = 1..=70;
    
    // Handle stepped ranges (e.g., "21..36:3")
    if let Some((range_part, step)) = s.split_once(':') {
        if let Some((start, end)) = range_part.split_once("..") {
            let start = start.parse()
                .wrap_err_with(|| format!("Invalid CRF range start: '{}'", start))?;
            let end = end.parse()
                .wrap_err_with(|| format!("Invalid CRF range end: '{}'", end))?;
            let step = step.parse()
                .wrap_err_with(|| format!("Invalid step value: '{}'", step))?;
            
            if start > end {
                return Err(eyre!("Range start must be <= end (got {start}..{end})"));
            }
            if step == 0 {
                return Err(eyre!("Step value must be > 0"));
            }
            if !CRF_RANGE.contains(&start) || !CRF_RANGE.contains(&end) {
                return Err(eyre!("CRF must be between {}-{} (got {start}..{end})",
                    CRF_RANGE.start(), CRF_RANGE.end()));
            }

            let mut values = Vec::new();
            let mut current = start;
            while current <= end {
                values.push(current);
                current = match current.checked_add(step) {
                    Some(v) => v,
                    None => break, // Prevent overflow
                };
            }
            return Ok(values);
        }
    }

    // Handle simple ranges (e.g., "21..36")
    if let Some((start, end)) = s.split_once("..") {
        let start = start.parse()
            .wrap_err_with(|| format!("Invalid CRF range start: '{}'", start))?;
        let end = end.parse()
            .wrap_err_with(|| format!("Invalid CRF range end: '{}'", end))?;
        
        if start > end {
            return Err(eyre!("Range start must be <= end (got {start}..{end})"));
        }
        if !CRF_RANGE.contains(&start) || !CRF_RANGE.contains(&end) {
            return Err(eyre!("CRF must be between {}-{} (got {start}..{end})",
                CRF_RANGE.start(), CRF_RANGE.end()));
        }
        
        return Ok((start..=end).collect());
    }

    // Handle comma-separated values
    s.split(',')
        .map(|part| {
            let value = part.trim().parse()
                .wrap_err_with(|| format!("Invalid CRF value: '{}'", part))?;
            
            if CRF_RANGE.contains(&value) {
                Ok(value)
            } else {
                Err(eyre!("CRF must be between {}-{} (got {value})",
                    CRF_RANGE.start(), CRF_RANGE.end()))
            }
        })
        .collect()
}

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Args::command().debug_assert();
}