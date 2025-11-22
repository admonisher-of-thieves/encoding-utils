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
    pub flag_hidden: Option<u8>,
    #[serde(rename = "ChapterFlagEnabled")]
    pub flag_enabled: Option<u8>,
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
                "      Hidden: {:?}, Enabled: {:?}",
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

impl fmt::Display for ZoneChapters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\nZoneChapters ({} total):", self.chapters.len())?;

        if self.chapters.is_empty() {
            return Ok(());
        }

        // Find max widths for alignment
        let max_name = self.chapters.iter().map(|c| c.name.len()).max().unwrap();
        let max_start = self
            .chapters
            .iter()
            .map(|c| c.start.to_string().len())
            .max()
            .unwrap();
        let max_end = self
            .chapters
            .iter()
            .map(|c| c.end.to_string().len())
            .max()
            .unwrap();

        for (i, chapter) in self.chapters.iter().enumerate() {
            // Format CRF: show "NaN" if it is NaN
            let crf_str = if chapter.crf.is_nan() {
                "NaN".to_string()
            } else {
                format!("{:.2}", chapter.crf)
            };

            writeln!(
                f,
                "  {:>2}. Chapter: {:<width_name$} | frames: {:>width_start$}–{:>width_end$} | CRF: {:>5}",
                i + 1,
                chapter.name,
                chapter.start,
                chapter.end,
                crf_str,
                width_name = max_name,
                width_start = max_start,
                width_end = max_end
            )?;
        }

        Ok(())
    }
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

            if end_frame < start_frame {
                panic!(
                    "Invalid chapter '{}': end_frame ({}) is smaller than start_frame ({})",
                    current_chapter.display.string, end_frame, start_frame
                );
            }

            zone_chapters.push(ZoneChapter {
                name: current_chapter.display.string.clone(),
                start: start_frame,
                end: end_frame,
                crf: f64::NAN,
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
        assert!(
            parts.len() == 3,
            "Invalid timestamp '{}': must be in format HH:MM:SS.FFFFFFFFF",
            time_str
        );

        let hours: f64 = parts[0].parse().expect("Invalid hours field");
        let minutes: f64 = parts[1].parse().expect("Invalid minutes field");

        let seconds_parts: Vec<&str> = parts[2].split('.').collect();
        assert!(
            !seconds_parts.is_empty(),
            "Invalid seconds field in timestamp '{}'",
            time_str
        );

        let seconds: f64 = seconds_parts[0].parse().expect("Invalid seconds value");
        let nanoseconds: f64 = if seconds_parts.len() > 1 {
            let frac_str = seconds_parts[1];
            let padded_frac = if frac_str.len() > 9 {
                &frac_str[..9]
            } else {
                frac_str
            };
            padded_frac
                .parse::<f64>()
                .expect("Invalid fractional seconds")
                / 1_000_000_000.0
        } else {
            0.0
        };

        let total_seconds = hours * 3600.0 + minutes * 60.0 + seconds + nanoseconds;
        (total_seconds * fps).round() as u32
    }
}
