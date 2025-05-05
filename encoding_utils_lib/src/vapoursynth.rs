use std::path::Path;
use std::{ffi::CString, str::FromStr};

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

pub fn lsmash_invoke(core: &Core, path: &Path) -> Result<VideoNode> {
    let lsmash = lsmash(core)?;
    let mut args = Map::default();
    args.set(
        KeyStr::from_cstr(&"source".to_cstring()),
        Value::Utf8(path.to_str().unwrap()),
        Replace,
    )?;

    let func = lsmash.invoke(&"LWLibavSource".to_cstring(), args);

    // Check for errors before getting the video node
    if let Some(err) = func.get_error() {
        return Err(eyre::eyre!(
            "lsmash LWLibavSource failed: {}",
            err.to_string_lossy()
        ));
    }

    Ok(func.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?)
}

pub fn bestsource_invoke(core: &Core, path: &Path) -> Result<VideoNode> {
    let bs = bestsource(core)?;
    let mut args = Map::default();
    args.set(
        KeyStr::from_cstr(&"source".to_cstring()),
        Value::Utf8(path.to_str().unwrap()),
        Replace,
    )?;

    let func = bs.invoke(&"VideoSource".to_cstring(), args);

    // Check for errors before getting the video node
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
    args.set(
        KeyStr::from_cstr(&"mode".to_cstring()),
        Value::Int(0),
        Replace,
    )?;

    let func = vszip.invoke(&"Metrics".to_cstring(), args);

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
    let resize = resize(core)?;
    let ref_info = reference.info();
    let dist_info = distorted.info();

    // Early return if resolutions already match
    if ref_info.width == dist_info.width {
        return Ok(reference.clone());
    }

    // Validate target dimensions
    if dist_info.width <= 0 || dist_info.height <= 0 {
        return Err(eyre::eyre!(
            "Invalid target dimensions: {}x{}",
            dist_info.width,
            dist_info.height
        ));
    }
    // Calculate proportional height based on width difference
    let target_width = dist_info.width as i64;
    let target_height = (ref_info.height as i64 * target_width) / ref_info.width as i64;

    let mut args = Map::default();
    args.set(
        KeyStr::from_cstr(&"clip".to_cstring()),
        Value::VideoNode(reference.to_owned()),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"width".to_cstring()),
        Value::Int(target_width),
        Replace,
    )?;
    args.set(
        KeyStr::from_cstr(&"height".to_cstring()),
        Value::Int(target_height),
        Replace,
    )?;

    // Choose the appropriate resize function
    let func = if dist_info.width > ref_info.width {
        // Upscaling - use Lanczos
        args.set(
            KeyStr::from_cstr(&"filter_param_a".to_cstring()),
            Value::Float(3.0), // Lanczos taps
            Replace,
        )?;
        resize.invoke(&"Lanczos".to_cstring(), args)
    } else {
        // Downscaling - use Bicubic
        args.set(
            KeyStr::from_cstr(&"filter_param_a".to_cstring()),
            Value::Float(0.0), // b parameter
            Replace,
        )?;
        args.set(
            KeyStr::from_cstr(&"filter_param_b".to_cstring()),
            Value::Float(0.5), // c parameter
            Replace,
        )?;
        resize.invoke(&"Bicubic".to_cstring(), args)
    };

    if let Some(err) = func.get_error() {
        return Err(eyre::eyre!(
            "Resize failed ({} → {}): {}",
            format!("{}x{}", ref_info.width, ref_info.height),
            format!("{}x{}", dist_info.width, dist_info.height),
            err.to_string_lossy()
        ));
    }

    Ok(func.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?)
}

pub fn auto_synchronize_clips(
    core: &Core,
    reference: &VideoNode,
    distorted: &VideoNode,
) -> Result<(VideoNode, VideoNode)> {
    let std = std(core)?;
    let ref_info = reference.info();
    let dist_info = distorted.info();

    match ref_info.num_frames.cmp(&dist_info.num_frames) {
        std::cmp::Ordering::Equal => {
            // No synchronization needed
            Ok((reference.clone(), distorted.clone()))
        }
        std::cmp::Ordering::Greater => {
            // Reference is longer - trim it to match distorted
            println!("Frame mismatch: Reference is longer - trimming it to match distorted");
            let frames_to_trim = ref_info.num_frames - dist_info.num_frames;
            let mut args = Map::default();
            args.set(
                KeyStr::from_cstr(&"clip".to_cstring()),
                Value::VideoNode(reference.to_owned()),
                Replace,
            )?;
            args.set(
                KeyStr::from_cstr(&"first".to_cstring()),
                Value::Int(frames_to_trim as i64),
                Replace,
            )?;

            let func = std.invoke(&"Trim".to_cstring(), args);
            if let Some(err) = func.get_error() {
                return Err(eyre::eyre!(
                    "Failed to trim reference clip by {} frames: {}",
                    frames_to_trim,
                    err.to_string_lossy()
                ));
            }

            let reference = func.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?;

            Ok((reference, distorted.clone()))
        }
        std::cmp::Ordering::Less => {
            // Distorted is longer - trim it to match reference
            println!("Frame mismatch: Distorted is longer - trimming it to match reference");
            let frames_to_trim = dist_info.num_frames - ref_info.num_frames;
            let mut args = Map::default();
            args.set(
                KeyStr::from_cstr(&"clip".to_cstring()),
                Value::VideoNode(distorted.to_owned()),
                Replace,
            )?;
            args.set(
                KeyStr::from_cstr(&"first".to_cstring()),
                Value::Int(frames_to_trim as i64),
                Replace,
            )?;

            let func = std.invoke(&"Trim".to_cstring(), args);
            if let Some(err) = func.get_error() {
                return Err(eyre::eyre!(
                    "Failed to trim distorted clip by {} frames: {}",
                    frames_to_trim,
                    err.to_string_lossy()
                ));
            }

            let distorted = func.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?;

            Ok((reference.clone(), distorted))
        }
    }
}
