use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

use eyre::{OptionExt, Result};

pub fn encode_frames<'a>(
    vpy: &'a Path,
    scenes_with_zones: &'a Path,
    encode_path: &'a Path,
    av1an_params: &str,
    encoder_params: &str,
    override_file: bool,
) -> Result<&'a Path> {
    if override_file && encode_path.exists() {
        fs::remove_file(encode_path)?;
    }

    let vpy_str = vpy.to_str().ok_or_eyre("Invalid UTF-8 in input path")?;
    let encode_str = encode_path
        .to_str()
        .ok_or_eyre("Invalid UTF-8 in encoder path")?;
    let scenes_str = scenes_with_zones
        .to_str()
        .ok_or_eyre("Invalid UTF-8 in scenes path")?;

    let av1an_params: Vec<&str> = av1an_params.split_whitespace().collect();
    let construct_params: Vec<&str> = Vec::from([
        "--video-params",
        encoder_params,
        "-y",
        "--scenes",
        scenes_str,
    ]);

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
