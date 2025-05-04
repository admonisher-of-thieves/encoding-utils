use std::path::Path;
use std::{ffi::CString, str::FromStr};

use eyre::{OptionExt, Result};
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

    // func.get_error().ok_or_eyre("Error lsmash invoke")?;

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

    // println!("{}",);

    // func.get_error().ok_or_eyre("Error bestsource invoke")?;

    Ok(func.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?)
}

pub fn vszip_metrics(
    core: &Core,
    reference: &VideoNode,
    distorted: &VideoNode,
) -> Result<VideoNode> {
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

    // func.get_error().ok_or_eyre("Error vszip invoke")?;

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
    Ok(spliced.get_video_node(KeyStr::from_cstr(&"clip".to_cstring()), 0)?)
}
