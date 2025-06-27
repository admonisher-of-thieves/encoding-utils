use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf, absolute};
use std::{ffi::CString, str::FromStr};

use clap::ValueEnum;
use eyre::{OptionExt, Result, eyre};
use vapoursynth4_rs::api::Api;
use vapoursynth4_rs::ffi::VSMapAppendMode::{Append, Replace};
use vapoursynth4_rs::{
    core::Core,
    map::{KeyStr, Map, Value},
    node::VideoNode,
    plugin::Plugin,
};

use crate::vpy_files::ColorMetadata;

pub trait ToCString {
    fn to_cstring(self) -> CString;
}

impl ToCString for &str {
    fn to_cstring(self) -> CString {
        CString::from_str(self).expect("String contains null bytes")
    }
}

pub fn print_vs_plugins() {
    let api = Api::default();
    let core = Core::builder().api(api).disable_library_unloading().build();
    for plugin in core.plugins() {
        println!("{}", plugin.id().to_str().unwrap())
    }
}

/// Chunking plugin
#[derive(Debug, Clone, ValueEnum, Copy)]
pub enum SourcePlugin {
    Lsmash,
    Bestsource,
}

impl SourcePlugin {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourcePlugin::Lsmash => "lsmash",
            SourcePlugin::Bestsource => "bestsource",
        }
    }
}

pub fn lsmash(core: &Core) -> Result<Plugin> {
    core.get_plugin_by_id(&"systems.innocent.lsmas".to_cstring())
        .ok_or_eyre("Plugin [systems.innocent.lsmas] was not found")
}

pub fn bestsource(core: &Core) -> Result<Plugin> {
    core.get_plugin_by_id(&"com.vapoursynth.bestsource".to_cstring())
        .ok_or_eyre("Plugin [systems.innocent.lsmas] was not found")
}

pub fn vszip(core: &Core) -> Result<Plugin> {
    core.get_plugin_by_id(&"com.julek.vszip".to_cstring())
        .ok_or_eyre("Plugin [com.julek.vszip] was not found")
}

pub fn vs_std(core: &Core) -> Result<Plugin> {
    core.get_plugin_by_id(&"com.vapoursynth.std".to_cstring())
        .ok_or_eyre("Plugin [com.vapoursynth.std] was not found")
}

pub fn resize(core: &Core) -> Result<Plugin> {
    core.get_plugin_by_id(&"com.vapoursynth.resize".to_cstring())
        .ok_or_eyre("Plugin [com.vapoursynth.resize] was not found")
}

pub fn fmtconv(core: &Core) -> Result<Plugin> {
    core.get_plugin_by_id(&"fmtconv".to_cstring())
        .ok_or_eyre("Plugin [fmtconv] was not found")
}

pub fn vivtc(core: &Core) -> Result<Plugin> {
    core.get_plugin_by_id(&"vivtc".to_cstring())
        .ok_or_eyre("Plugin [vivtc] was not found")
}

pub fn lsmash_invoke(core: &Core, path: &Path, temp_dir: &Path) -> Result<VideoNode> {
    let lsmash = lsmash(core)?;
    let mut args = Map::default();

    let path = absolute(path)?;
    let temp_dir = absolute(temp_dir)?;

    // Set source path
    args.set(
        KeyStr::from_cstr(&"source".to_cstring()),
        Value::Utf8(path.to_str().unwrap()),
        Replace,
    )?;

    let cache_path = temp_dir.join(
        path.file_name()
            .ok_or_eyre("Input path has no filename")?
            .to_str()
            .ok_or_eyre("Filename not UTF-8")?,
    );
    let cache_path = add_extension("lwi", cache_path);

    args.set(
        KeyStr::from_cstr(&"cachefile".to_cstring()),
        Value::Utf8(cache_path.to_str().unwrap()),
        Replace,
    )?;

    let func = lsmash.invoke(&"LWLibavSource".to_cstring(), args);
    if let Some(err) = func.get_error() {
        return Err(eyre::eyre!(
            "lsmash LWLibavSource failed: {}",
            err.to_string_lossy()
        ));
    }

    Ok(func.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?)
}

pub fn bestsource_invoke(core: &Core, path: &Path, temp_dir: &Path) -> Result<VideoNode> {
    let bs = bestsource(core)?;
    let mut args = Map::default();

    // Set source path
    args.set(
        KeyStr::from_cstr(&"source".to_cstring()),
        Value::Utf8(path.to_str().unwrap()),
        Replace,
    )?;

    let cache_path = temp_dir.join(
        path.file_name()
            .ok_or_eyre("Input path has no filename")?
            .to_str()
            .ok_or_eyre("Filename not UTF-8")?,
    );
    // let cache_path = add_extension("bsindex", cache_path);

    args.set(
        KeyStr::from_cstr(&"cachepath".to_cstring()),
        Value::Utf8(cache_path.to_str().unwrap()),
        Replace,
    )?;

    args.set(
        KeyStr::from_cstr(&"cachemode".to_cstring()),
        Value::Int(4),
        Replace,
    )?;

    let func = bs.invoke(&"VideoSource".to_cstring(), args);

    if let Some(err) = func.get_error() {
        return Err(eyre::eyre!(
            "Bestsource VideoSource failed: {}",
            err.to_string_lossy()
        ));
    }

    Ok(func.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?)
}
pub fn vszip_metrics(
    core: &Core,
    reference: &VideoNode,
    distorted: &VideoNode,
) -> Result<VideoNode> {
    // Check frame counts first
    let ref_info = reference.info();
    let dist_info = distorted.info();

    if ref_info.num_frames != dist_info.num_frames {
        return Err(eyre::eyre!(
            "Frame count mismatch: reference has {}, encode has {}",
            ref_info.num_frames,
            dist_info.num_frames
        ));
    }

    let vszip = vszip(core)?;
    let mut args = Map::default();
    args.set(
        KeyStr::from_cstr(&"reference".to_cstring()),
        Value::VideoNode(reference.to_owned()),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"distorted".to_cstring()),
        Value::VideoNode(distorted.to_owned()),
        Replace,
    )?;

    let func = vszip.invoke(&"SSIMULACRA2".to_cstring(), args);

    // Check for errors before getting the video node
    if let Some(err) = func.get_error() {
        return Err(eyre::eyre!(
            "Vszip SSIMULACRA2 failed: {}",
            err.to_string_lossy()
        ));
    }

    Ok(func.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?)
}

pub fn set_color_metadata(core: &Core, clip: &VideoNode, color_params: &str) -> Result<VideoNode> {
    let color_metadata = ColorMetadata::from_params(color_params);
    let resize = resize(core)?;
    let mut args = Map::default();

    args.set(
        KeyStr::from_cstr(&"clip".to_cstring()),
        Value::VideoNode(clip.to_owned()),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"matrix_in".to_cstring()),
        Value::Int(color_metadata.matrix.into()),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"transfer_in".to_cstring()),
        Value::Int(color_metadata.transfer.into()),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"primaries_in".to_cstring()),
        Value::Int(color_metadata.primaries.into()),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"range_in".to_cstring()),
        Value::Int(color_metadata.range.into()),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"chromaloc_in".to_cstring()),
        Value::Int(color_metadata.chromaloc.into()),
        Replace,
    )?;

    let func = resize.invoke(&"Bicubic".to_cstring(), args);

    // Check for errors before getting the video node
    if let Some(err) = func.get_error() {
        return Err(eyre::eyre!(
            "Resize Bicubic failed: {}",
            err.to_string_lossy()
        ));
    }

    Ok(func.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?)
}

pub fn select_frames(core: &Core, clip: &VideoNode, frames: &[u32]) -> Result<VideoNode> {
    if frames.is_empty() {
        return Err(eyre::eyre!("No frames specified for selection"));
    }

    let std = vs_std(core)?;
    let mut splice_args = Map::default();

    for (i, &frame) in frames.iter().enumerate() {
        let mut trim_args = Map::default();
        trim_args.set(
            KeyStr::from_cstr(&"clip".to_cstring()),
            Value::VideoNode(clip.to_owned()),
            Replace,
        )?;
        trim_args.set(
            KeyStr::from_cstr(&"first".to_cstring()),
            Value::Int(frame.into()),
            Replace,
        )?;
        trim_args.set(
            KeyStr::from_cstr(&"last".to_cstring()),
            Value::Int(frame.into()),
            Replace,
        )?;

        let trimmed = std.invoke(&"Trim".to_cstring(), trim_args);
        let trimmed_clip = trimmed.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?;

        splice_args.set(
            KeyStr::from_cstr(&"clips".to_cstring()),
            Value::VideoNode(trimmed_clip),
            if i == 0 { Replace } else { Append },
        )?;
    }

    let spliced = std.invoke(&"Splice".to_cstring(), splice_args);

    // Check for errors before getting the video node
    if let Some(err) = spliced.get_error() {
        return Err(eyre::eyre!("STD Splice failed: {}", err.to_string_lossy()));
    }

    Ok(spliced.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?)
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
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 4 {
            return Err(eyre!("Crop string must be in format width:height:left:top"));
        }

        Ok(CropParams {
            width: parts[0].parse()?,
            height: parts[1].parse()?,
            left: parts[2].parse()?,
            top: parts[3].parse()?,
        })
    }
}

pub fn crop_reference_to_match(
    core: &Core,
    reference: &VideoNode,
    crop: &str,
) -> Result<VideoNode> {
    let crop_params = CropParams::from_str(crop)?;
    let ref_info = reference.info();

    let std = vs_std(core)?;
    let mut args = Map::default();

    args.set(
        KeyStr::from_cstr(&"clip".to_cstring()),
        Value::VideoNode(reference.to_owned()),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"width".to_cstring()),
        Value::Int(crop_params.width),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"height".to_cstring()),
        Value::Int(crop_params.height),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"left".to_cstring()),
        Value::Int(crop_params.left),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"top".to_cstring()),
        Value::Int(crop_params.top),
        Replace,
    )?;

    let func = std.invoke(&"CropAbs".to_cstring(), args);
    if let Some(err) = func.get_error() {
        return Err(eyre::eyre!(
            "Failed to crop reference. Crop: {}. Video: {}x{}. Error: {}",
            crop,
            ref_info.width,
            ref_info.height,
            err.to_string_lossy()
        ));
    }

    Ok(func.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?)
}

pub fn downscale_resolution(core: &Core, reference: &VideoNode) -> Result<VideoNode> {
    use vapoursynth4_rs::{
        ffi::VSMapAppendMode::Replace,
        map::{KeyStr, Map, Value},
    };

    // Get plugin handles
    let fmtconv_plugin = fmtconv(core)?;
    let std_plugin = vs_std(core)?;

    let ref_info = reference.info();

    // Check if height/2 is odd and crop if needed
    let mut working_clip = reference.clone();
    if (ref_info.height / 2) % 2 != 0 {
        let mut crop_args = Map::default();
        crop_args.set(
            KeyStr::from_cstr(&"clip".to_cstring()),
            Value::VideoNode(working_clip.to_owned()),
            Replace,
        )?;
        crop_args.set(
            KeyStr::from_cstr(&"top".to_cstring()),
            Value::Int(1),
            Replace,
        )?;
        crop_args.set(
            KeyStr::from_cstr(&"bottom".to_cstring()),
            Value::Int(1),
            Replace,
        )?;

        let cropped = std_plugin.invoke(&"Crop".to_cstring(), crop_args);
        if let Some(err) = cropped.get_error() {
            return Err(eyre::eyre!("Crop failed: {}", err.to_string_lossy()));
        }
        working_clip = cropped.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?;
    }

    // Box downscale (scale = 0.5)
    let mut fmt_args = Map::default();
    fmt_args.set(
        KeyStr::from_cstr(&"clip".to_cstring()),
        Value::VideoNode(working_clip.to_owned()),
        Replace,
    )?;
    fmt_args.set(
        KeyStr::from_cstr(&"kernel".to_cstring()),
        Value::Utf8("box"),
        Replace,
    )?;
    fmt_args.set(
        KeyStr::from_cstr(&"scale".to_cstring()),
        Value::Float(0.5),
        Replace,
    )?;

    let resampled = fmtconv_plugin.invoke(&"resample".to_cstring(), fmt_args);
    if let Some(err) = resampled.get_error() {
        return Err(eyre::eyre!(
            "Box resample failed: {}",
            err.to_string_lossy()
        ));
    }

    let box_clip = resampled.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?;

    Ok(box_clip)
}

#[derive(Debug, Clone)]
pub enum ClipTarget {
    Reference,
    Distorted,
}

#[derive(Debug, Clone)]
pub struct Trim {
    pub first: usize,
    pub last: usize,
    pub clip_target: ClipTarget,
}

impl FromStr for Trim {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 3 {
            return Err("Expected format: first,last,clip".into());
        }

        let first = parts[0]
            .parse::<usize>()
            .map_err(|_| "Invalid first value")?;
        let last = parts[1]
            .parse::<usize>()
            .map_err(|_| "Invalid last value")?;

        let clip_target = match parts[2].to_lowercase().as_str() {
            "r" | "reference" => ClipTarget::Reference,
            "d" | "distorted" => ClipTarget::Distorted,
            other => return Err(format!("Invalid clip target: '{}'", other)),
        };

        Ok(Trim {
            first,
            last,
            clip_target,
        })
    }
}

pub fn synchronize_clips(
    core: &Core,
    reference: &VideoNode,
    distorted: &VideoNode,
    clip: &Trim,
) -> Result<(VideoNode, VideoNode)> {
    let std = vs_std(core)?;

    let mut args = Map::default();
    let (target_clip, _, is_reference) = match clip.clip_target {
        ClipTarget::Reference => (reference, distorted, true),
        ClipTarget::Distorted => (distorted, reference, false),
    };

    args.set(
        KeyStr::from_cstr(&"clip".to_cstring()),
        Value::VideoNode(target_clip.to_owned()),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"first".to_cstring()),
        Value::Int(clip.first as i64),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"last".to_cstring()),
        Value::Int(clip.last as i64),
        Replace,
    )?;

    let func = std.invoke(&"Trim".to_cstring(), args);
    if let Some(err) = func.get_error() {
        return Err(eyre::eyre!(
            "Failed to trim selected clip ({}–{}): {}",
            clip.first,
            clip.last,
            err.to_string_lossy()
        ));
    }

    let trimmed = func.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?;

    if is_reference {
        Ok((trimmed, distorted.clone()))
    } else {
        Ok((reference.clone(), trimmed))
    }
}

pub fn get_dimensions(
    input: &Path,
    importer_plugin: &SourcePlugin,
    temp_dir: &Path,
) -> Result<Dimensions> {
    let api = Api::default();
    let core = Core::builder().api(api).build();
    // Load reference and distorted
    let reference = match importer_plugin {
        SourcePlugin::Lsmash => lsmash_invoke(&core, input, temp_dir)?,
        SourcePlugin::Bestsource => bestsource_invoke(&core, input, temp_dir)?,
    };

    let info = reference.info();
    Ok(Dimensions {
        width: info.width,
        height: info.height,
    })
}

pub fn get_number_of_frames(
    input: &Path,
    importer_plugin: &SourcePlugin,
    temp_dir: &Path,
) -> Result<i32> {
    let api = Api::default();
    let core = Core::builder().api(api).build();
    // Load reference and distorted
    let reference = match importer_plugin {
        SourcePlugin::Lsmash => lsmash_invoke(&core, input, temp_dir)?,
        SourcePlugin::Bestsource => bestsource_invoke(&core, input, temp_dir)?,
    };

    let info = reference.info();
    Ok(info.num_frames)
}

#[derive(Debug)]
pub struct Dimensions {
    pub width: i32,
    pub height: i32,
}

pub fn add_extension(ext: impl AsRef<OsStr>, path: PathBuf) -> PathBuf {
    let mut os_string: OsString = path.into();
    os_string.push(".");
    os_string.push(ext.as_ref());
    os_string.into()
}

pub fn inverse_telecine(core: &Core, input: &VideoNode) -> Result<VideoNode> {
    // Load vivtc plugin
    let vivtc = vivtc(core)?;

    // --- VFM: Field Matching ---
    let mut vfm_args = Map::default();
    vfm_args.set(
        KeyStr::from_cstr(&"clip".to_cstring()),
        Value::VideoNode(input.clone()),
        Replace,
    )?;
    vfm_args.set(
        KeyStr::from_cstr(&"order".to_cstring()),
        Value::Int(1), // Top field first
        Replace,
    )?;
    vfm_args.set(
        KeyStr::from_cstr(&"mode".to_cstring()),
        Value::Int(1), // Full field matching
        Replace,
    )?;

    let vfm_out = vivtc.invoke(&"VFM".to_cstring(), vfm_args);
    if let Some(err) = vfm_out.get_error() {
        return Err(eyre::eyre!("VFM failed: {}", err.to_string_lossy()));
    }
    let vfm_clip = vfm_out.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?;

    // --- VDecimate: Remove duplicates ---
    let mut vdecimate_args = Map::default();
    vdecimate_args.set(
        KeyStr::from_cstr(&"clip".to_cstring()),
        Value::VideoNode(vfm_clip.clone()),
        Replace,
    )?;

    let vdecimate_out = vivtc.invoke(&"VDecimate".to_cstring(), vdecimate_args);
    if let Some(err) = vdecimate_out.get_error() {
        return Err(eyre::eyre!("VDecimate failed: {}", err.to_string_lossy()));
    }

    let decimated_clip =
        vdecimate_out.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?;
    Ok(decimated_clip)
}
