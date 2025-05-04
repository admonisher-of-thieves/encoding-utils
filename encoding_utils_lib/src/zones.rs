use std::{fs, path::Path};

use eyre::Result;

use crate::scenes::SceneList;

pub fn create_zone_file<'a>(
    zone_file: &'a Path,
    scene_list: &'a SceneList,
    crf: u32,
    override_file: bool,
) -> Result<&'a Path> {
    if override_file && zone_file.exists() {
        fs::remove_file(zone_file)?;
    }

    println!("Creating zone file:\n");

    // Convert scores to strings and join with newlines
    let content = scene_list
        .scenes
        .iter()
        .map(|scene| {
            format!(
                "{} {} svt-av1 --crf {}",
                scene.start_frame, scene.end_frame, crf
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(zone_file, content)?;

    println!("Zone file created\n");
    Ok(zone_file)
}

pub fn create_temp_zone_file<'a>(
    zone_file: &'a Path,
    scene_list: &'a SceneList,
    crf: u32,
    override_file: bool,
) -> Result<&'a Path> {
    if override_file && zone_file.exists() {
        fs::remove_file(zone_file)?;
    }

    println!("Creating zone file:\n");

    let scene_len = scene_list.scenes.len();

    // Convert scores to strings and join with newlines
    let content = (0..scene_len)
        .map(|i| {
            let start_frame = i;
            let end_frame = if i == scene_len - 1 {
                -1
            } else {
                (i + 1) as i32
            };
            format!("{} {} svt-av1 --crf {}", start_frame, end_frame, crf)
        })
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(zone_file, content)?;

    println!("Zone file created\n");
    Ok(zone_file)
}

// pub fn create_temp_zone_file<'a>(
//     zone_file: &'a Path,
//     scene_list: &'a SceneList,
//     crf: u32,
//     override_file: bool,
// ) -> Result<&'a Path> {
//     if override_file && zone_file.exists() {
//         fs::remove_file(zone_file)?;
//     }

//     println!("Creating zone file:\n");

//     let scenes = scene_list.middle_frames();

//     // Convert scores to strings and join with newlines
//     let content = (scenes)
//         .iter()
//         .enumerate()
//         .map(|(i, &scene)| {
//             let start_frame = scene;
//             let end_frame = if i == scenes.len() - 1 {
//                 -1
//             } else {
//                 (i + 1) as i32
//             };
//             format!("{} {} svt-av1 --crf {}", start_frame, end_frame, crf)
//         })
//         .collect::<Vec<_>>()
//         .join("\n");

//     fs::write(zone_file, content)?;

//     println!("Zone file created\n");
//     Ok(zone_file)
// }
