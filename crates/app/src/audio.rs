//! Timeline audio playback and waveform caching for the editor.

use animara_document::{Document, Id, Timeline};
use animara_media::AudioOut;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default)]
pub struct AudioPlayback {
    out: AudioOut,
    waveforms: HashMap<(Id, usize), Option<Arc<Vec<f32>>>>,
    pub available: Option<bool>,
}

impl AudioPlayback {
    /// Peak envelope at 100 samples/second for a library sound (cached).
    pub fn waveform(&mut self, doc: &Document, id: Id) -> Option<Arc<Vec<f32>>> {
        let asset = doc.library.sound(id)?;
        let key = (id, Arc::as_ptr(&asset.data) as usize);
        self.waveforms
            .entry(key)
            .or_insert_with(|| animara_media::decode_sound(asset).ok().map(|p| Arc::new(animara_media::peaks_per_centisecond(&p))))
            .clone()
    }

    /// Play the timeline's sounds from `frame` to the end.
    pub fn play_timeline(&mut self, doc: &Document, tl: &Timeline, frame: u32) -> bool {
        if !tl.layers.iter().any(|l| l.frames.iter().any(|k| k.sound.is_some())) {
            self.stop();
            return true;
        }
        let end = tl.length().saturating_sub(1).max(frame);
        let pcm = animara_media::mixdown_timeline(doc, tl, frame, end, 44100);
        let ok = self.out.play(&pcm);
        self.available = Some(ok);
        ok
    }

    pub fn preview_sound(&mut self, doc: &Document, id: Id) {
        if let Some(s) = doc.library.sound(id)
            && let Ok(pcm) = animara_media::decode_sound(s)
        {
            let ok = self.out.play(&pcm);
            self.available = Some(ok);
        }
    }

    pub fn stop(&mut self) {
        self.out.stop();
    }
}
