// use bytesize::ByteSize;
// use walkdir::WalkDir;

// fn find_large_files(dir: &str, min_size: u64) -> Vec<(String, ByteSize)> {
//     WalkDir::new(dir)
//         .into_iter()
//         .filter_map(|entry| {
//             let entry = entry.ok()?;
//             let path = entry.path();

//             // Skip directories
//             if !path.is_file() || path.extension()? != "ivf" {
//                 return None;
//             }

//             // Get file size
//             let metadata = path.metadata().ok()?;
//             let size = metadata.len();

//             // Check if file meets the size threshold
//             if size >= min_size {
//                 let file_name = path.file_name()?.to_str()?.to_string();
//                 Some((file_name, ByteSize::b(size)))
//             } else {
//                 None
//             }
//         })
//         .collect()
// }

// fn main() {
//     let min_size = ByteSize::mib(1).as_u64();
//     let large_files = find_large_files(".", min_size);

//     println!("Files larger than {}:", ByteSize::mib(1));
//     for (file, size) in large_files {
//         println!("- {}: {}", file, size); // MiB, GiB format
//     }
// }
