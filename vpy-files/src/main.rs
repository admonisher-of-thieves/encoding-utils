use std::path::PathBuf;

use clap::Parser;
use encoding_utils_lib::{vapoursynth::{crop_reference_to_match, SourcePlugin}, vpy_files::create_filter_vpy_file};
use eyre::{OptionExt, Result};

/// Tool to create VapourSynth filter script
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Input video file
    #[arg(short, long)]
    input: PathBuf,

    /// Output file (default: "[VPY] <input>.vpy" if no input given)
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    output: Option<PathBuf>,

    /// Crop string (e.g. 1920:816:0:132)
    #[arg(short, long)]
    crop: Option<String>,

    /// Scale expression (e.g. zscale=1920:-1:filter=lanczos)
    #[arg(short, long)]
    scale: Option<String>,

    /// Importer plugin (lsmash or bestsource)
    #[arg(short = 'P', long, value_enum, default_value_t = SourcePlugin::Lsmash)]
    importer: SourcePlugin,

    /// Overwrite output file if it exists
    #[arg(short = 'O', long, default_value_t = false)]
    overwrite: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let vpy_name = format!(
        "[{}{}VPY] {}.vpy",
        if args.crop.is_some() { "CROP " } else { "" },
        if args.scale.is_some() { "SCALE " } else { "" },
        args.input
            .file_stem()
            .ok_or_eyre("No file name")?
            .to_str()
            .ok_or_eyre("Invalid UTF-8 in input path")?,
    );

    let vpy_file = match args.output {
        Some(output) => output,
        None => args.input.with_file_name(&vpy_name),
    };

    create_filter_vpy_file(
        &args.input,
        &vpy_file,
        args.crop.as_deref(),
        args.scale.as_deref(),
        args.importer,
        args.overwrite,
    )?;

    println!("{}", vpy_name);
    Ok(())
}
