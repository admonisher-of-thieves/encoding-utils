use eyre::Result;
use eyre::eyre;
use indicatif::{ProgressBar, ProgressStyle};
use iter_chunks::IterChunks;
use ndarray::{Array4, ArrayView2, ArrayView4, Axis, ShapeBuilder, s};
// use rayon::iter::ParallelBridge;
// use rayon::iter::ParallelIterator;
use vapoursynth4_rs::{
    frame::VideoFrame,
    node::{Node, VideoNode},
};

#[derive(Debug)]
pub struct VideoConfig {
    pub src: VideoNode,
    pub total_frames: usize,
    pub frame_shape: FrameShape,
    pub batch: u32,
}

#[derive(Debug)]
pub struct FrameShape {
    pub height: i64,
    pub width: i64,
    pub channels: i64,
}

impl FrameShape {
    /// Converts the frame shape to a tuple of usize dimensions
    /// in (height, width, channels) order
    pub fn as_tuple(&self) -> (usize, usize, usize) {
        (
            self.height as usize,
            self.width as usize,
            self.channels as usize,
        )
    }
}

// Conversion from tuple to FrameShape
impl From<(i64, i64, i64)> for FrameShape {
    fn from(tuple: (i64, i64, i64)) -> Self {
        FrameShape {
            height: tuple.0,
            width: tuple.1,
            channels: tuple.2,
        }
    }
}

// Conversion from FrameShape to tuple
impl From<FrameShape> for (i64, i64, i64) {
    fn from(shape: FrameShape) -> Self {
        (shape.height, shape.width, shape.channels)
    }
}

impl VideoConfig {
    pub fn get_frames(&self) -> Result<Vec<VideoFrame>> {
        let pb = self.create_progress_bar("Extracting frames");

        let mut frames: Vec<(usize, VideoFrame)> = (0..self.total_frames)
            // .par_bridge()
            .map(|n| {
                let frame = self
                    .src
                    .get_frame(n.try_into().unwrap())
                    .map_err(|e| eyre!("Failed to load frame {}: {}", n, e.to_string_lossy()))?;
                pb.inc(1);
                Ok((n, frame))
            })
            .collect::<Result<Vec<_>>>()?;

        pb.finish_with_message("Frame extraction complete");
        println!("\n");

        // Sort by original index to maintain frame order
        frames.sort_by_key(|(i, _)| *i);

        // Discard the indices and return only the frames
        Ok(frames.into_iter().map(|(_, frame)| frame).collect())
    }

    pub fn frame_batches(&self) -> impl Iterator<Item = Result<Vec<VideoFrame>>> + '_ {
        // Create the frame iterator
        let frame_iter = (0..self.total_frames).map(move |n| {
            self.src
                .get_frame(n.try_into().unwrap())
                .map_err(|e| eyre!("Failed to load frame {}: {}", n, e.to_string_lossy()))
        });

        // Create chunks iterator
        let mut chunks = frame_iter.chunks(self.batch as usize);

        // Convert to proper Iterator using iter::from_fn
        std::iter::from_fn(move || chunks.next().map(|chunk| chunk.collect()))
    }

    /// Main processing pipeline
    pub fn process_frames(&self) -> Result<Array4<f32>> {
        let (height, width, channels) = self.validate_dimensions()?;

        let all_frames = self.modify_frames(height, width, channels)?;

        let frames_f32 = self.concatenate_and_convert(all_frames)?;
        self.create_padded_frames(frames_f32, height, width, channels)
    }

    /// Creates progress bar with consistent styling
    pub fn create_progress_bar(&self, message: &'static str) -> ProgressBar {
        let pb = ProgressBar::new(self.total_frames as u64);
        pb.set_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] {prefix} {wide_bar} {pos}/{len} {msg}",
            )
            .unwrap(),
        );
        pb.set_message(message);
        pb
    }

    /// Validates frame dimensions
    pub fn validate_dimensions(&self) -> Result<(usize, usize, usize)> {
        let (h, w, c) = self.frame_shape.as_tuple();
        if c != 3 {
            return Err(eyre!("Expected 3 channels (RGB), got {}", c));
        }
        Ok((h, w, c))
    }

    /// Extracts frames in batches
    pub fn modify_frames(
        &self,
        height: usize,
        width: usize,
        channels: usize,
    ) -> Result<Vec<Array4<u8>>> {
        let mut batch_frames = Vec::new();
        let all_frames = self.get_frames()?;

        for batch in all_frames.chunks(self.batch.try_into().unwrap()) {
            let batch_size = batch.len();
            let mut batch_arr = Array4::<u8>::zeros((batch_size, height, width, channels));

            self.process_batch(batch, &mut batch_arr, height, width)?;
            batch_frames.push(batch_arr);
        }

        Ok(batch_frames)
    }

    /// Processes a single batch of frames
    pub fn process_batch(
        &self,
        frames: &[VideoFrame],
        batch_arr: &mut Array4<u8>,
        height: usize,
        width: usize,
    ) -> Result<()> {
        for (i, frame) in frames.iter().enumerate() {
            for c in 0..3 {
                let plane_ptr = frame.plane(c);
                let stride = frame.stride(c) as usize;

                unsafe {
                    let plane_view =
                        ArrayView2::from_shape_ptr((height, width).strides((stride, 1)), plane_ptr);
                    batch_arr.slice_mut(s![i, .., .., c]).assign(&plane_view);
                }
            }
        }
        Ok(())
    }

    /// Concatenates and converts frames to f32
    pub fn concatenate_and_convert(&self, all_frames: Vec<Array4<u8>>) -> Result<Array4<f32>> {
        let views: Vec<_> = all_frames.iter().map(|a| a.view()).collect();
        let concatenated = ndarray::concatenate(Axis(0), &views)
            .map_err(|e| eyre!("Concatenation failed: {}", e))?;

        Ok(concatenated.mapv(|x| x as f32))
    }

    /// Creates padded frames
    pub fn create_padded_frames(
        &self,
        frames_f32: Array4<f32>,
        height: usize,
        width: usize,
        channels: usize,
    ) -> Result<Array4<f32>> {
        let pad_start = self.create_padding(
            frames_f32.slice(s![0..1, .., .., ..]),
            25,
            height,
            width,
            channels,
        )?;

        let pad_size = 25 + (50 - (self.total_frames % 50).min(50));
        let pad_end = self.create_padding(
            frames_f32.slice(s![-1.., .., .., ..]),
            pad_size,
            height,
            width,
            channels,
        )?;

        ndarray::concatenate(
            Axis(0),
            &[pad_start.view(), frames_f32.view(), pad_end.view()],
        )
        .map_err(|e| eyre!("Final concatenation failed: {}", e))
    }

    /// Creates padded section
    pub fn create_padding(
        &self,
        frame: ArrayView4<f32>,
        repeat: usize,
        height: usize,
        width: usize,
        channels: usize,
    ) -> Result<Array4<f32>> {
        frame
            .broadcast((repeat, height, width, channels))
            .map(|view| view.to_owned())
            .ok_or_else(|| eyre!("Broadcast failed for padding"))
    }
}
