use std::{fs, path::Path};

use eyre::{OptionExt, Result};

use crate::scenes::SceneList;

pub fn create_vpy_file<'a>(
    input: &'a Path,
    vpy_file: &'a Path,
    scene_list: &'a SceneList,
    override_file: bool,
) -> Result<&'a Path> {
    if override_file && vpy_file.exists() {
        fs::remove_file(vpy_file)?;
    }

    let input_str = input.to_str().ok_or_eyre("Invalid UTF-8 in input path")?;

    let frames = scene_list.middle_frames();

    // Build the frames list as a single string
    let frames_str: String = frames
        .iter()
        .map(|frame| frame.to_string())
        .collect::<Vec<String>>()
        .join(", ");

    // Use string formatting to build the vpy script efficiently
    let vpy_script = format!(
        "import vapoursynth as vs\n\
        core = vs.core\n\n\
        src = core.bs.VideoSource(\"{}\")\n\n\
        frames = [{}]\n\n\
        selected_frames = [src[frame] for frame in frames]\n\n\
        output = core.std.Splice(selected_frames)\n\
        output.set_output()\n",
        input_str, frames_str
    );

    fs::write(vpy_file, vpy_script)?;

    Ok(vpy_file)
}
