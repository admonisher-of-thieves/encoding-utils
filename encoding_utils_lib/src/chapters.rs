use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, fs, path::Path};
use vapoursynth4_rs::node::VideoNode;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Chapters {
    #[serde(rename = "EditionEntry")]
    pub edition_entry: EditionEntry,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct EditionEntry {
    #[serde(default)]
    #[serde(rename = "EditionFlagHidden")]
    pub flag_hidden: Option<u8>,
    #[serde(default)]
    #[serde(rename = "EditionFlagDefault")]
    pub flag_default: Option<u8>,
    #[serde(default)]
    #[serde(rename = "EditionFlagOrdered")]
    pub flag_ordered: Option<u8>,
    #[serde(rename = "EditionUID")]
    pub uid: String,
    #[serde(rename = "ChapterAtom")]
    pub chapters: Vec<ChapterAtom>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct ChapterAtom {
    #[serde(rename = "ChapterUID")]
    pub uid: String,
    #[serde(rename = "ChapterTimeStart")]
    pub time_start: String,
    #[serde(rename = "ChapterFlagHidden")]
    pub flag_hidden: u8,
    #[serde(rename = "ChapterFlagEnabled")]
    pub flag_enabled: u8,
    #[serde(rename = "ChapterDisplay")]
    pub display: ChapterDisplay,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct ChapterDisplay {
    #[serde(rename = "ChapterString")]
    pub string: String,
    #[serde(rename = "ChapterLanguage")]
    pub language: String,
    #[serde(rename = "ChapLanguageIETF")]
    pub language_ietf: String,
}

// Custom display implementation for better formatting
impl fmt::Display for Chapters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Chapters:")?;
        writeln!(f, "  Edition UID: {}", self.edition_entry.uid)?;
        writeln!(
            f,
            "  Hidden: {}, Default: {}, Ordered: {}",
            self.edition_entry.flag_hidden.unwrap_or(0),
            self.edition_entry.flag_default.unwrap_or(0),
            self.edition_entry.flag_ordered.unwrap_or(0)
        )?;
        writeln!(f, "  Chapters:")?;

        for (i, chapter) in self.edition_entry.chapters.iter().enumerate() {
            writeln!(f, "    Chapter {}:", i + 1)?;
            writeln!(f, "      UID: {}", chapter.uid)?;
            writeln!(f, "      Start Time: {}", chapter.time_start)?;
            writeln!(
                f,
                "      Hidden: {}, Enabled: {}",
                chapter.flag_hidden, chapter.flag_enabled
            )?;
            writeln!(f, "      Title: {}", chapter.display.string)?;
            writeln!(
                f,
                "      Language: {} ({})",
                chapter.display.language, chapter.display.language_ietf
            )?;
        }
        Ok(())
    }
}

impl Chapters {
    pub fn parse(path: &Path) -> eyre::Result<Chapters> {
        let xml_data = fs::read_to_string(path)?;
        let chapters: Chapters = quick_xml::de::from_str(&xml_data)?;
        Ok(chapters)
    }

    pub fn write<'a>(&self, path: &'a Path) -> eyre::Result<&'a Path> {
        let xml = quick_xml::se::to_string(&self)?;
        fs::write(path, xml)?;
        Ok(path)
    }
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct ZoneChapters {
    pub chapters: Vec<ZoneChapter>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct ZoneChapter {
    pub name: String,
    pub start: u32,
    pub end: u32,
    pub crf: f64,
}

impl ZoneChapters {
    /// Basic conversion from Chapters to ZoneChapters without CRF values
    pub fn from_chapters(video: &VideoNode, chapters: Chapters) -> Self {
        let info = video.info();
        let fps_num = info.fps_num as f64;
        let fps_den = info.fps_den as f64;
        let fps = fps_num / fps_den;

        let mut zone_chapters = Vec::new();
        let chapter_atoms = chapters.edition_entry.chapters;

        for i in 0..chapter_atoms.len() {
            let current_chapter = &chapter_atoms[i];

            // Convert time string to frame number for start
            let start_frame = Self::time_to_frame(&current_chapter.time_start, fps);

            // Convert time string to frame number for end
            let end_frame = if i < chapter_atoms.len() - 1 {
                Self::time_to_frame(&chapter_atoms[i + 1].time_start, fps)
            } else {
                // For the last chapter, use the total frames
                info.num_frames.try_into().unwrap()
            };

            zone_chapters.push(ZoneChapter {
                name: current_chapter.display.string.clone(),
                start: start_frame,
                end: end_frame,
                crf: 0.0, // Default CRF value
            });
        }

        ZoneChapters {
            chapters: zone_chapters,
        }
    }

    /// Adds CRF values to existing ZoneChapters based on the CRF string
    pub fn with_crfs(&mut self, crfs: String) {
        if crfs.is_empty() {
            return;
        }

        // Parse CRF values from the string in format "Chapter:CRF,Chapter:CRF"
        let mut crf_map = HashMap::new();
        for pair in crfs.split(',') {
            let parts: Vec<&str> = pair.split(':').collect();
            if parts.len() == 2 {
                let chapter_name = parts[0].trim();
                if let Ok(crf_value) = parts[1].trim().parse::<f64>() {
                    crf_map.insert(chapter_name.to_string(), crf_value);
                }
            }
        }

        // Apply CRF values to matching chapters
        for zone_chapter in &mut self.chapters {
            if let Some(crf) = crf_map.get(&zone_chapter.name) {
                zone_chapter.crf = *crf;
            }
        }
    }

    /// Converts a time string in format "HH:MM:SS.FFFFFFFFF" to frame number
    fn time_to_frame(time_str: &str, fps: f64) -> u32 {
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() != 3 {
            return 0;
        }

        let hours: f64 = parts[0].parse().unwrap_or(0.0);
        let minutes: f64 = parts[1].parse().unwrap_or(0.0);
        let seconds_parts: Vec<&str> = parts[2].split('.').collect();

        let seconds: f64 = seconds_parts[0].parse().unwrap_or(0.0);
        let nanoseconds: f64 = if seconds_parts.len() > 1 {
            // Handle up to 9 decimal places (nanoseconds)
            let frac_str = seconds_parts[1];
            let padded_frac = if frac_str.len() > 9 {
                &frac_str[..9]
            } else {
                frac_str
            };
            padded_frac.parse::<f64>().unwrap_or(0.0) / 1_000_000_000.0
        } else {
            0.0
        };

        let total_seconds = hours * 3600.0 + minutes * 60.0 + seconds + nanoseconds;
        (total_seconds * fps).round() as u32
    }
}
