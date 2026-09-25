use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SectionFile {
    pub section: Vec<Section>,
}

#[derive(Debug, Deserialize)]
pub struct Section {
    pub name: String,
    pub frames: Option<FrameRange>,
    #[serde(default = "default_crop")]
    pub crop: Vec<Crop>,
    #[serde(default)]
    pub ocr: OCR,
    pub ocr_script: Option<String>,
    pub languages: Option<String>,
    #[serde(default)]
    pub position: Position,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FrameRange {
    pub start: Option<i32>,
    pub end: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct Crop {
    #[serde(default)]
    pub top: u32,
    #[serde(default)]
    pub bottom: u32,
    #[serde(default)]
    pub left: u32,
    #[serde(default)]
    pub right: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Position {
    Top,
    #[default]
    Bottom,
}

#[derive(Debug, Deserialize, Default)]
pub enum OCR {
    AppleVision,
    AppleLiveText,
    #[default]
    RapidOCR,
}

fn default_crop() -> Vec<Crop> {
    vec![Crop {
        top: 0,
        bottom: 0,
        left: 0,
        right: 0,
    }]
}

impl Section {
    pub fn resolved_frame_range(&self, total_frames: i32) -> FrameRange {
        FrameRange {
            start: Some(self.frames.as_ref().and_then(|f| f.start).unwrap_or(0)),
            end: Some(
                self.frames
                    .as_ref()
                    .and_then(|f| f.end)
                    .unwrap_or(total_frames),
            ),
        }
    }
}
