use std::{fs, path::Path};

use crate::{main_loop::check_chunk_method, scenes::SceneList, vapoursynth::ImporterPlugin};
use eyre::{OptionExt, Result, eyre};
use std::str::FromStr;

pub fn create_frames_vpy_file<'a>(
    input: &'a Path,
    vpy_file: &'a Path,
    scene_list: &'a SceneList,
    av1an_params: &'a str,
    crop: Option<&str>,
    downscale: bool,
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

    let importer = check_chunk_method(av1an_params).unwrap_or(ImporterPlugin::Bestsource);
    let importer = match importer {
        ImporterPlugin::Lsmash => "core.lsmas.LWLibavSource",
        ImporterPlugin::Bestsource => "core.bs.VideoSource",
    };

    // Use string formatting to build the vpy script efficiently
    let mut vpy_script = format!(
        r#"
import vapoursynth as vs

from vstools import (
    core,
    set_output,
    initialize_clip,
    Matrix,
    Primaries,
    Transfer,
)

src = {importer}("{input_str}")

src = initialize_clip(
    src,
    matrix=Matrix.BT709,
    primaries=Primaries.BT709,
    transfer=Transfer.BT709,
)

frames = [{frames_str}]
selected_frames = [src[frame] for frame in frames]
output = core.std.Splice(selected_frames)
src = output

"#,
        importer = importer,
        input_str = input_str,
        frames_str = frames_str
    );

    if let Some(crop_str) = crop {
        let crop_params = CropParams::from_str(crop_str)?;
        vpy_script += &format!(
            r#"
cropped = core.std.CropAbs\(
    src,
    width={width},
    height={height},
    left={left},
    top={top}
src = cropped

"#,
            width = crop_params.width,
            height = crop_params.height,
            left = crop_params.left,
            top = crop_params.top,
        );
    }

    if downscale {
        vpy_script += r#"
rgb = core.resize.Bicubic(src, transfer_s="linear", format=vs.RGBS)
box = core.fmtc.resample(rgb, kernel="Box", scale=0.5)
src = box

"#;
    }

    vpy_script += r#"
set_output(src)
"#;

    fs::write(vpy_file, vpy_script)?;

    Ok(vpy_file)
}

#[derive(Debug)]
pub struct CropParams {
    pub width: i64,
    pub height: i64,
    pub left: i64,
    pub top: i64,
}

impl FromStr for CropParams {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self> {
        if s.is_empty() {
            return Err(eyre!("Crop string is empty"));
        }

        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 4 {
            return Err(eyre!(
                "Crop string must be in format width:height:left:top or empty"
            ));
        }

        Ok(CropParams {
            width: parts[0].parse()?,
            height: parts[1].parse()?,
            left: parts[2].parse()?,
            top: parts[3].parse()?,
        })
    }
}

#[derive(Debug)]
pub struct ZscaleParams {
    pub width: i64,
    pub height: i64, // -1 means calculate from aspect ratio
    pub filter: String,
}

impl FromStr for ZscaleParams {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self> {
        if s.is_empty() {
            return Err(eyre!("Zscale string string is empty"));
        }

        let parts: Vec<&str> = s.split('=').collect();
        if parts.len() != 3 || !parts[0].eq_ignore_ascii_case("zscale") {
            return Err(eyre!(
                "Zscale string must be in format zscale=width:height:filter=type or empty"
            ));
        }

        let params: Vec<&str> = parts[1].split(':').collect();
        if params.is_empty() {
            return Err(eyre!("Zscale parameters must include width"));
        }

        let width = params[0].parse()?;
        let height = if params.len() > 1 && params[1] != "-1" {
            params[1].parse()?
        } else {
            -1 // Special value indicating we should calculate from aspect ratio
        };

        let filter = if params.len() > 2 {
            params[2]
                .strip_prefix("filter=")
                .unwrap_or("lanczos")
                .to_string()
        } else {
            "lanczos".to_string()
        };

        Ok(ZscaleParams {
            width,
            height,
            filter,
        })
    }
}

pub fn create_filter_vpy_file<'a>(
    input: &'a Path,
    vpy_file: &'a Path,
    crop_str: Option<&str>,
    zscale_str: Option<&str>,
    importer_plugin: ImporterPlugin,
    override_file: bool,
) -> Result<&'a Path> {
    if override_file && vpy_file.exists() {
        fs::remove_file(vpy_file)?;
    }

    let input_str = input.to_str().ok_or_eyre("Invalid UTF-8 in input path")?;
    let crop = crop_str.map(CropParams::from_str).transpose()?;
    let zscale = zscale_str.map(ZscaleParams::from_str).transpose()?;

    let mut processing_steps = Vec::new();
    let mut current_var = "src".to_string();

    match importer_plugin {
        ImporterPlugin::Lsmash => {
            processing_steps.push(format!(
                r#"# Load source using L-SMASH
{current_var} = core.lsmas.LWLibavSource("{input_path}")"#,
                input_path = input_str,
                current_var = current_var
            ));
        }
        ImporterPlugin::Bestsource => {
            processing_steps.push(format!(
                r#"# Load source using BestSource
{current_var} = core.bs.VideoSource("{input_path}")"#,
                input_path = input_str,
                current_var = current_var
            ));
        }
    }

    // Crop processing if specified
    if let Some(crop) = crop {
        processing_steps.push(format!(
            r#"
# Crop parameters: {crop_str}
{next_var}_cropped = core.std.CropAbs(
    {current_var},
    width={width},
    height={height},
    left={left},
    top={top}
)"#,
            current_var = current_var,
            next_var = current_var,
            crop_str = crop_str.unwrap_or(""),
            width = crop.width,
            height = crop.height,
            left = crop.left,
            top = crop.top
        ));
        current_var = format!("{}_cropped", current_var);
    }

    // Zscale processing if specified
    if let Some(zscale) = zscale {
        let height_expr = if zscale.height == -1 {
            format!(
                "int({current_var}.height * {width} / {current_var}.width)",
                width = zscale.width
            )
        } else {
            zscale.height.to_string()
        };

        // Determine the resize function based on the filter type
        let resize_func = match zscale.filter.to_lowercase().as_str() {
            "point" => "Point",
            "bilinear" => "Bilinear",
            "bicubic" => "Bicubic",
            "lanczos" => "Lanczos",
            "spline16" => "Spline16",
            "spline36" => "Spline36",
            "spline64" => "Spline64",
            _ => "Bicubic", // default to bicubic if unknown
        };

        // Prepare filter-specific parameters
        let filter_params = match resize_func {
            "Bicubic" => {
                let (a, b) = if zscale.filter == "bicubic" {
                    (0.0, 0.75) // Mitchell-Netravali
                } else {
                    (0.5, 0.5)
                };
                format!("filter_param_a={}, filter_param_b={}", a, b)
            }
            "Lanczos" => "filter_param_a=3".to_string(),
            _ => String::new(),
        };
        processing_steps.push(format!(
            r#"
# Resize parameters: {zscale_str}
{next_var}_scaled = core.resize.{resize_func}(
    {current_var},
    width={width},
    height={height},
    {filter_params}
    matrix_in_s="709",
    transfer_in_s="709",
    primaries_in_s="709",
    range_in_s="limited",
    matrix_s="709",
    transfer_s="709",
    primaries_s="709",
    range_s="limited"
)"#,
            current_var = current_var,
            next_var = current_var,
            zscale_str = zscale_str.unwrap_or(""),
            width = zscale.width,
            height = height_expr,
            resize_func = resize_func,
            filter_params = filter_params
        ));
        current_var = format!("{}_scaled", current_var);
    }

    // Build final script
    let vpy_script = format!(
        r#"import vapoursynth as vs
core = vs.core

{processing_steps}

{output_var}.set_output()"#,
        processing_steps = processing_steps.join("\n"),
        output_var = current_var
    );

    fs::write(vpy_file, vpy_script)?;

    Ok(vpy_file)
}
