use std::path::Path;
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
#[derive(Debug, Clone, ValueEnum)]
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

pub fn std(core: &Core) -> Result<Plugin> {
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

pub fn lsmash_invoke(core: &Core, path: &Path, temp_dir: &Path) -> Result<VideoNode> {
    let lsmash = lsmash(core)?;
    let mut args = Map::default();

    // Set source path
    args.set(
        KeyStr::from_cstr(&"source".to_cstring()),
        Value::Utf8(path.to_str().unwrap()),
        Replace,
    )?;

    let cache_path = temp_dir
        .join(
            path.file_name()
                .ok_or_eyre("Input path has no filename")?
                .to_str()
                .ok_or_eyre("Filename not UTF-8")?,
        )
        .with_extension("lwi");

    println!("{:?}", cache_path);
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

    let cache_path = temp_dir
        .join(
            path.file_name()
                .ok_or_eyre("Input path has no filename")?
                .to_str()
                .ok_or_eyre("Filename not UTF-8")?,
        )
        .with_extension("bsi");
    args.set(
        KeyStr::from_cstr(&"cachefile".to_cstring()),
        Value::Utf8(cache_path.to_str().unwrap()),
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
            "Vszip Metrics failed: {}",
            err.to_string_lossy()
        ));
    }

    Ok(func.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?)
}

pub fn resize_bicubic(core: &Core, clip: &VideoNode) -> Result<VideoNode> {
    let resize = resize(core)?;
    let mut args = Map::default();

    args.set(
        KeyStr::from_cstr(&"clip".to_cstring()),
        Value::VideoNode(clip.to_owned()),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"format".to_cstring()),
        Value::Int(555745280),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"matrix_in_s".to_cstring()),
        Value::Utf8("709"),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"transfer_in_s".to_cstring()),
        Value::Utf8("709"),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"primaries_in_s".to_cstring()),
        Value::Utf8("709"),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"range_in_s".to_cstring()),
        Value::Utf8("limited"),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"matrix_s".to_cstring()),
        Value::Utf8("rgb"),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"transfer_s".to_cstring()),
        Value::Utf8("709"),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"primaries_s".to_cstring()),
        Value::Utf8("709"),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"range_s".to_cstring()),
        Value::Utf8("full"),
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

    let std = std(core)?;
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
    distorted: &VideoNode,
) -> Result<VideoNode> {
    let dist_info = distorted.info();
    let ref_info = reference.info();

    // Early return if dimensions already match
    if ref_info.width == dist_info.width && ref_info.height == dist_info.height {
        return Ok(reference.clone());
    }

    // Calculate centered crop position
    let left = (ref_info.width as i64 - dist_info.width as i64) / 2;
    let top = (ref_info.height as i64 - dist_info.height as i64) / 2;

    // Validate crop area
    if left < 0 || top < 0 {
        return Err(eyre::eyre!(
            "Distorted dimensions {}x{} are larger than reference {}x{}",
            dist_info.width,
            dist_info.height,
            ref_info.width,
            ref_info.height
        ));
    }

    let std = std(core)?;
    let mut args = Map::default();

    args.set(
        KeyStr::from_cstr(&"clip".to_cstring()),
        Value::VideoNode(reference.to_owned()),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"width".to_cstring()),
        Value::Int(dist_info.width as i64),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"height".to_cstring()),
        Value::Int(dist_info.height as i64),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"left".to_cstring()),
        Value::Int(left),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"top".to_cstring()),
        Value::Int(top),
        Replace,
    )?;

    let func = std.invoke(&"CropAbs".to_cstring(), args);
    if let Some(err) = func.get_error() {
        return Err(eyre::eyre!(
            "Failed to crop reference to {}x{}: {}",
            dist_info.width,
            dist_info.height,
            err.to_string_lossy()
        ));
    }

    Ok(func.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?)
}

pub fn match_distorted_resolution(
    core: &Core,
    reference: &VideoNode,
    distorted: &VideoNode,
) -> Result<VideoNode> {
    use vapoursynth4_rs::{
        ffi::VSMapAppendMode::Replace,
        map::{KeyStr, Map, Value},
    };

    // Get plugin handles
    let fmtconv_plugin = fmtconv(core)?;

    let ref_info = reference.info();
    let dist_info = distorted.info();

    if ref_info.width == dist_info.width && ref_info.height == dist_info.height {
        return Ok(reference.clone());
    }

    // Throw an error if distorted is larger than reference
    if dist_info.width > ref_info.width || dist_info.height > ref_info.height {
        return Err(eyre::eyre!(
            "Distorted resolution ({:?}x{:?}) is larger than reference ({:?}x{:?})",
            dist_info.width,
            dist_info.height,
            ref_info.width,
            ref_info.height
        ));
    }

    // Step 2: Box downscale (scale = 0.5)
    let mut fmt_args = Map::default();
    fmt_args.set(
        KeyStr::from_cstr(&"clip".to_cstring()),
        Value::VideoNode(reference.to_owned()),
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
    let std = std(core)?;

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

#[derive(Debug)]
pub struct Dimensions {
    pub width: i32,
    pub height: i32,
}
