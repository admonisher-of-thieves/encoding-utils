use std::{
    fs,
    path::{Path, absolute},
};

use crate::{scenes::FrameSelection, vapoursynth::SourcePlugin};
use crate::{scenes::FramesDistribution, vapoursynth::add_extension};
use eyre::{OptionExt, Result, eyre};
use std::str::FromStr;

#[allow(clippy::too_many_arguments)]
pub fn create_vpy_file<'a>(
    input: &'a Path,
    vpy_file: &'a Path,
    frame_selection: Option<&FrameSelection>,
    source_plugin: &'a SourcePlugin,
    crop: Option<&str>,
    downscale: bool,
    detelecine: bool,
    encoder_params: &str,
    temp_folder: &'a Path,
    override_file: bool,
) -> Result<&'a Path> {
    if override_file && vpy_file.exists() {
        fs::remove_file(vpy_file)?;
    }

    // Parse and map color metadata parameters
    let color_metadata = ColorMetadata::from_params(encoder_params);

    let input_str = input.to_str().ok_or_eyre("Invalid UTF-8 in input path")?;

    // Configure source and cache
    let (source, cache) = {
        let cache_path = absolute(match source_plugin {
            SourcePlugin::Lsmash => add_extension(
                "lwi",
                temp_folder.join(input.file_name().ok_or_eyre("Input path has no filename")?),
            ),
            SourcePlugin::Bestsource => {
                temp_folder.join(input.file_name().ok_or_eyre("Input path has no filename")?)
            }
        })?;

        let cache_str = cache_path.to_str().ok_or_eyre("Filename not UTF-8")?;

        match source_plugin {
            SourcePlugin::Lsmash => (
                "core.lsmas.LWLibavSource",
                format!("cachefile=\"{}\"", cache_str),
            ),
            SourcePlugin::Bestsource => (
                "core.bs.VideoSource",
                format!("cachepath=\"{}\", cachemode=4", cache_str),
            ),
        }
    };

    // Build script sections
    let header = format!(
        r#"import vapoursynth as vs
core = vs.core

src = {source}("{input_str}", {cache})
"#,
        source = source,
        input_str = input_str,
        cache = cache
    );

    let color_metadata_section = format!(
        r#"src = core.resize.Bicubic(
    src,
    matrix_in={matrix},
    transfer_in={transfer},
    primaries_in={primaries},
    range_in={range},
    chromaloc_in={chromaloc}
)
"#,
        matrix = color_metadata.matrix,
        transfer = color_metadata.transfer,
        primaries = color_metadata.primaries,
        range = color_metadata.range,
        chromaloc = color_metadata.chromaloc
    );

    // Frame selection handling
    let frame_selection_section = if let Some(fs) = frame_selection {
        let frames_str = match fs.distribution {
            FramesDistribution::Center => fs.scene_list.center_expanding_frames(fs.n_frames),
            FramesDistribution::Evenly => fs.scene_list.evenly_spaced_frames(fs.n_frames),
        }
        .iter()
        .map(|f| f.to_string())
        .collect::<Vec<_>>()
        .join(", ");

        format!(
            r#"frames = [{frames_str}]
selected_frames = [src[frame] for frame in frames]
src = core.std.Splice(selected_frames)
"#,
            frames_str = frames_str
        )
    } else {
        String::new()
    };

    let detelecine_section = if detelecine {
        r#"
# IVTC for 29.97fps to 23.976fps conversion
src = core.vivtc.VFM(src, order=1, mode=1)
src = core.vivtc.VDecimate(src)
"#
    } else {
        ""
    };

    let crop = if let Some(crop_str) = crop.filter(|s| !s.is_empty()) {
        let params = CropParams::from_str(crop_str)?;
        format!(
            r#"
# Apply cropping
src = core.std.CropAbs(
    src,
    width={width},
    height={height},
    left={left},
    top={top}
)
"#,
            width = params.width,
            height = params.height,
            left = params.left,
            top = params.top
        )
    } else {
        String::new()
    };

    let downscale_section = if downscale {
        format!(
            r#"
rgb = core.resize.Bicubic(src, transfer_s="linear", format=vs.RGBS)
if (rgb.height / 2) % 2 != 0:
    rgb = core.std.Crop(rgb, top=1, bottom=1)
downscaled = core.fmtc.resample(rgb, kernel="box", scale=0.5)

src = core.resize.Bicubic(
    downscaled,
    format=vs.YUV420P10,
    matrix={matrix},
    transfer={transfer},
    primaries={primaries},
    range={range},
    chromaloc={chromaloc},
    dither_type="error_diffusion"
)
"#,
            matrix = color_metadata.matrix,
            transfer = color_metadata.transfer,
            primaries = color_metadata.primaries,
            range = color_metadata.range,
            chromaloc = color_metadata.chromaloc
        )
    } else {
        format!(
            r#"
src = core.resize.Bicubic(
    src,
    format=vs.YUV420P10,
    matrix={matrix},
    transfer={transfer},
    primaries={primaries},
    range={range},
    chromaloc={chromaloc}
)
"#,
            matrix = color_metadata.matrix,
            transfer = color_metadata.transfer,
            primaries = color_metadata.primaries,
            range = color_metadata.range,
            chromaloc = color_metadata.chromaloc
        )
    };

    let vpy_script = format!(
        "{header}\n{color_metadata}\n{detelecine}\n{frame_selection}\n{crop}\n{downscale}\nsrc.set_output()\n",
        header = header,
        color_metadata = color_metadata_section,
        detelecine = detelecine_section,
        frame_selection = frame_selection_section,
        downscale = downscale_section,
    );

    fs::write(vpy_file, vpy_script)?;
    Ok(vpy_file)
}

// Helper function to parse parameters
pub fn parse_param<'a>(params: &'a str, name: &str) -> Option<&'a str> {
    params
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1])
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
pub struct ColorMetadata {
    pub matrix: u8,
    pub transfer: u8,
    pub primaries: u8,
    pub range: u8,
    pub chromaloc: u8,
}

impl Default for ColorMetadata {
    fn default() -> Self {
        Self {
            matrix: 1,    // bt709
            transfer: 1,  // bt709
            primaries: 1, // bt709
            range: 0,     // studio
            chromaloc: 0, // left
        }
    }
}

impl ColorMetadata {
    pub fn from_params(params: &str) -> Self {
        let mut metadata = Self::default();

        if let Some(value) = parse_param(params, "--matrix-coefficients") {
            metadata.matrix = match value {
                "bt2020-ncl" => 9,
                _ => metadata.matrix,
            };
        }

        if let Some(value) = parse_param(params, "--transfer-characteristics") {
            metadata.transfer = match value {
                "smpte2084" => 16,
                _ => metadata.transfer,
            };
        }

        if let Some(value) = parse_param(params, "--color-primaries") {
            metadata.primaries = match value {
                "bt2020" => 9,
                _ => metadata.primaries,
            };
        }

        if let Some(value) = parse_param(params, "--color-range") {
            metadata.range = match value {
                "full" => 1,
                _ => metadata.range,
            };
        }

        if let Some(value) = parse_param(params, "--chroma-sample-position") {
            metadata.chromaloc = match value {
                "topleft" => 2,
                _ => metadata.chromaloc,
            };
        }

        metadata
    }
}
