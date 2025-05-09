use crate::{
    main_loop::update_extra_split_and_min_scene_len,
    math::{Score, ScoreList},
    scenes::{Scene, SceneList, ZoneOverrides},
};

#[derive(Debug)]
pub struct ChunkList {
    pub chunks: Vec<Chunk>,
    pub frames: u32,
}

impl ChunkList {
    pub fn to_scene_list_with_zones(&self, av1an_params: &str, encoder_params: &str) -> SceneList {
        let av1an_params = update_extra_split_and_min_scene_len(av1an_params, Some(0), None);
        let scenes: Vec<Scene> = self
            .chunks
            .iter()
            .map(|chunk| {
                let zone_overrides = ZoneOverrides::from(&av1an_params, encoder_params, chunk.crf);
                Scene {
                    start_frame: chunk.scene.start_frame,
                    end_frame: chunk.scene.end_frame,
                    zone_overrides: Some(zone_overrides),
                }
            })
            .collect();

        SceneList {
            scenes,
            frames: self.frames,
        }
    }

    pub fn to_scene_list_with_zones_filtered(
        &self,
        av1an_params: &str,
        encoder_params: &str,
        ssimu2_score: f64,
    ) -> SceneList {
        let av1an_params = update_extra_split_and_min_scene_len(av1an_params, Some(0), Some(1));
        let scenes: Vec<Scene> = self
            .chunks
            .iter()
            .filter(|chunk| chunk.score.value < ssimu2_score)
            .map(|chunk| {
                let zone_overrides = ZoneOverrides::from(&av1an_params, encoder_params, chunk.crf);
                Scene {
                    start_frame: chunk.scene.start_frame,
                    end_frame: chunk.scene.end_frame,
                    zone_overrides: Some(zone_overrides),
                }
            })
            .collect();
        let scenes_len = scenes.len();

        SceneList {
            scenes,
            frames: scenes_len as u32,
        }
    }

    pub fn to_score_list(&self) -> ScoreList {
        ScoreList {
            scores: self
                .chunks
                .clone()
                .into_iter()
                .map(|chunk| chunk.score)
                .collect(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Chunk {
    pub crf: u8,
    pub score: Score,
    pub scene: Scene,
}
