use crate::{
    main_loop::update_extra_split_and_min_scene_len,
    math::Score,
    scenes::{Scene, SceneList, ZoneOverrides},
};

#[derive(Debug)]
pub struct ChunkList {
    pub chunks: Vec<Chunk>,
    pub frames: u32,
}

impl ChunkList {
    pub fn to_scene_list_with_zones(&self, av1an_params: &str, encoder_params: &str) -> SceneList {
        let scenes = self
            .chunks
            .iter()
            .map(|chunk| {
                let zone_overrides = ZoneOverrides::from(av1an_params, encoder_params, chunk.crf);
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
        verbose: bool,
    ) -> SceneList {
        let av1an_params = update_extra_split_and_min_scene_len(av1an_params, 0, 1);
        let scenes = self
            .chunks
            .iter()
            .filter(|chunk| chunk.score.value < ssimu2_score)
            .map(|chunk| {
                if verbose {
                    println!("{:?}", chunk);
                }
                let zone_overrides = ZoneOverrides::from(&av1an_params, encoder_params, chunk.crf);
                Scene {
                    start_frame: chunk.scene.start_frame,
                    end_frame: chunk.scene.end_frame,
                    zone_overrides: Some(zone_overrides),
                }
            })
            .collect();

        if verbose {
            println!();
        }
        SceneList {
            scenes,
            frames: self.frames, // Note: may no longer match filtered scene count
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Chunk {
    pub crf: u32,
    pub score: Score,
    pub scene: Scene,
}
