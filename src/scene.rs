use crate::presets::WORLD_COUNT;

/// A two-world morph never changes either endpoint halfway through a blend.
/// Rapid manual changes keep only the latest requested world and begin after
/// the current morph finishes. This keeps the GPU path small and continuous.
const MORPH_SECONDS: f32 = 5.0;

pub struct SceneTransition {
    pub from: usize,
    pub to: usize,
    pub amount: f32,
    pending: Option<usize>,
}

impl SceneTransition {
    pub fn new(scene: usize) -> Self {
        let scene = scene.min(WORLD_COUNT - 1);
        Self {
            from: scene,
            to: scene,
            amount: 1.0,
            pending: None,
        }
    }

    pub fn request(&mut self, scene: usize) {
        let scene = scene.min(WORLD_COUNT - 1);
        if scene == self.to {
            self.pending = None;
        } else if self.amount < 1.0 {
            self.pending = Some(scene);
        } else {
            self.from = self.to;
            self.to = scene;
            self.amount = 0.0;
        }
    }

    pub fn advance(&mut self, dt: f32) {
        self.advance_over(dt, MORPH_SECONDS);
    }

    pub fn advance_over(&mut self, dt: f32, seconds: f32) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        let seconds = if seconds.is_finite() {
            seconds.clamp(1.0, 30.0)
        } else {
            MORPH_SECONDS
        };
        self.amount = (self.amount + dt / seconds).min(1.0);
        if self.amount == 1.0
            && let Some(scene) = self.pending.take()
        {
            self.request(scene);
        }
    }

    pub fn blend(&self) -> f32 {
        let t = self.amount;
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    /// Read-only preparation hint; never advances or consumes the queued choice.
    pub fn pending(&self) -> Option<usize> {
        self.pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_choices_do_not_reset_an_active_morph() {
        let mut transition = SceneTransition::new(0);
        transition.request(1);
        transition.advance(MORPH_SECONDS * 0.5);
        let before = transition.blend();
        transition.request(2);
        transition.request(3);
        assert_eq!((transition.from, transition.to), (0, 1));
        assert_eq!(transition.blend(), before);
        transition.advance(MORPH_SECONDS * 0.5);
        assert_eq!((transition.from, transition.to), (1, 3));
        assert_eq!(transition.amount, 0.0);
    }

    #[test]
    fn returning_to_the_current_target_cancels_the_queue() {
        let mut transition = SceneTransition::new(0);
        transition.request(1);
        transition.request(2);
        transition.request(1);
        transition.advance(MORPH_SECONDS);
        assert_eq!(transition.to, 1);
        assert_eq!(transition.amount, 1.0);
    }

    #[test]
    fn new_worlds_morph_and_invalid_ids_are_bounded() {
        let mut transition = SceneTransition::new(usize::MAX);
        assert_eq!(
            (transition.from, transition.to),
            (WORLD_COUNT - 1, WORLD_COUNT - 1)
        );
        for scene in 0..WORLD_COUNT {
            transition.request(scene);
            transition.advance(MORPH_SECONDS);
            assert_eq!(transition.to, scene);
            assert_eq!(transition.blend(), 1.0);
        }
        transition.request(usize::MAX);
        assert_eq!(transition.to, WORLD_COUNT - 1);
        assert_eq!(transition.amount, 1.0);
    }
}
