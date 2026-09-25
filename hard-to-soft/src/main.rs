use clap::{ArgAction, Parser};
use encoding_utils_lib::
    vapoursynth::{get_number_of_frames, SourcePlugin}
;
use eyre::{OptionExt, Result};
use hard_to_soft::{crop_extract::extract_frames, sections::SectionFile};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use vapoursynth4_rs::core::Core;
use std::{fs::{self, create_dir_all}, path::PathBuf};

/// Calculate SSIMULACRA2 metric - Using vszip
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input video file
    #[arg(short, long)]
    input: PathBuf,

    /// Toml file containing the sections defining the extraction of hardsubs
    #[arg(short = 's', long)]
    sections: PathBuf,

    /// Enable verbose output
    #[arg(short = 'v', long = "verbose", action = ArgAction::SetTrue, default_value_t = false)]
    verbose: bool,

    /// Video Source Plugin
    #[arg(long = "source-plugin", default_value = "lsmash")]
    source_plugin: SourcePlugin,

    /// Keep temporary files (disables automatic cleanup)
    #[arg(
        short = 'k', 
        long = "keep-files",
        action = ArgAction::SetTrue,
        default_value_t = false,

    )]
    keep_files: bool,

    /// Temp folder (default: "[TEMP]_<input>.json" if no temp folder given)
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    temp: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let temp_folder = match args.temp {
        Some(temp) => temp, 
        None => { 
            let output_name = format!(
                "[TEMP]_{}",
                args.input
                    .file_stem()
                    .ok_or_eyre("No file name")?
                    .to_str()
                    .ok_or_eyre("Invalid UTF-8 in input path")?
            );
            args.input.with_file_name(output_name)
        }
    };

    let toml_content = fs::read_to_string(&args.sections)?;
    let section_file: SectionFile = toml::from_str(&toml_content)?;

    if temp_folder.exists() && !args.keep_files {
        fs::remove_dir_all(&temp_folder)?;
    }

    let to_override = !temp_folder.exists();

    create_dir_all(&temp_folder)?;

    let core = Core::builder().build();

    let total_frames = get_number_of_frames(&core,&args.input, &args.source_plugin, &temp_folder)?;

    section_file.section
    .par_iter()
    .try_for_each(|section| {
        extract_frames(
            &args.input,
            args.source_plugin,
            section,
            total_frames,
            to_override,
            &temp_folder,
        )
    })?;


    println!("READY");

    if !args.keep_files && fs::exists(&temp_folder)? {
        fs::remove_dir_all(&temp_folder)?;
    }

    Ok(())
}
