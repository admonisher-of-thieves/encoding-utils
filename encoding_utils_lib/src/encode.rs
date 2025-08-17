use std::{
    fs::{self, create_dir_all},
    path::Path,
    process::{Command, Stdio},
};

use eyre::{OptionExt, Result};

pub fn encode_frames<'a>(
    input: &'a Path,
    scenes_with_zones: &'a Path,
    encode_path: &'a Path,
    av1an_params: &str,
    encoder_params: &str,
    clean: bool,
    temp_folder: &'a Path,
) -> Result<&'a Path> {
    if clean && encode_path.exists() {
        fs::remove_file(encode_path)?;
    }
    let mut temp_folder = temp_folder.to_owned();
    temp_folder.push(
        input
            .file_stem()
            .ok_or_eyre("No file name")?
            .to_str()
            .ok_or_eyre("Invalid UTF-8 in input path")?,
    );

    create_dir_all(&temp_folder)?;

    let temp_folder = temp_folder
        .to_str()
        .ok_or_eyre("Invalid UTF-8 in scenes path")?;

    let vpy_str = input.to_str().ok_or_eyre("Invalid UTF-8 in input path")?;
    let encode_str = encode_path
        .to_str()
        .ok_or_eyre("Invalid UTF-8 in encoder path")?;
    let scenes_str = scenes_with_zones
        .to_str()
        .ok_or_eyre("Invalid UTF-8 in scenes path")?;

    let av1an_params: Vec<&str> = av1an_params.split_whitespace().collect();
    let mut construct_params: Vec<&str> = Vec::from([
        "--video-params",
        encoder_params,
        "-y",
        "--scenes",
        scenes_str,
        "--temp",
        temp_folder,
    ]);

    if !clean {
        construct_params.push("--keep");
    }

    let mut args = Vec::from(["-i", vpy_str, "-o", encode_str]);
    args.extend(av1an_params);
    args.extend(construct_params);

    println!("{:?}", args.join(" "));
    println!();

    Command::new("av1an")
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()?;

    Ok(encode_path)
}

pub fn resume_encode<'a>(
    input: &'a Path,
    scenes_with_zones: &'a Path,
    encode_path: &'a Path,
    av1an_params: &str,
    encoder_params: &str,
    clean: bool,
    temp_folder: &'a Path,
) -> Result<&'a Path> {
    if clean && encode_path.exists() {
        fs::remove_file(encode_path)?;
    }

    let temp_folder = temp_folder
        .to_str()
        .ok_or_eyre("Invalid UTF-8 in scenes path")?;

    let vpy_str = input.to_str().ok_or_eyre("Invalid UTF-8 in input path")?;
    let encode_str = encode_path
        .to_str()
        .ok_or_eyre("Invalid UTF-8 in encoder path")?;
    let scenes_str = scenes_with_zones
        .to_str()
        .ok_or_eyre("Invalid UTF-8 in scenes path")?;

    let av1an_params: Vec<&str> = av1an_params.split_whitespace().collect();
    let mut construct_params: Vec<&str> = Vec::from([
        "--video-params",
        encoder_params,
        "-y",
        "--scenes",
        scenes_str,
        "--temp",
        temp_folder,
        "--resume",
    ]);

    if !clean {
        construct_params.push("--keep");
    }

    let mut args = Vec::from(["-i", vpy_str, "-o", encode_str]);
    args.extend(av1an_params);
    args.extend(construct_params);

    println!("{:?}", args.join(" "));
    println!();

    Command::new("av1an")
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()?;

    Ok(encode_path)
}
