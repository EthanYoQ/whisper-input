//! Show/focus/close transitions and delayed blur jobs, using a monotonic clock.
pub fn tracks_focus_loss(label: &str) -> bool {
    matches!(label, "main" | "capsule" | "qa" | "selection-polish")
}

#[derive(Clone, Copy, Debug)]
pub struct BlurJob {
    generation: u64,
    pub due: u64,
}

pub struct FocusPolicy {
    generation: u64,
    grace_until: u64,
    armed: bool,
}
impl FocusPolicy {
    pub const fn new() -> Self {
        Self {
            generation: 0,
            grace_until: 0,
            armed: false,
        }
    }
    pub fn show(&mut self, now: u64) {
        self.generation += 1;
        self.armed = false;
        self.grace_until = now.saturating_add(750);
    }
    pub fn focused(&mut self) {
        self.generation += 1;
        self.armed = true;
    }
    pub fn hide(&mut self) {
        self.generation += 1;
        self.armed = false;
        self.grace_until = 0;
    }
    pub fn suppressed(&self, now: u64) -> bool {
        now < self.grace_until
    }
    pub fn lost_focus(&mut self, now: u64) -> Option<BlurJob> {
        if !self.armed {
            return None;
        }
        self.generation += 1;
        Some(BlurJob {
            generation: self.generation,
            due: now.saturating_add(75).max(self.grace_until),
        })
    }
    pub fn current(&self, job: BlurJob, now: u64) -> bool {
        self.armed && job.generation == self.generation && now >= job.due
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auxiliary_window_blur_rechecks_main_after_same_process_handoff() {
        let mut policy = FocusPolicy::new();
        policy.show(0);
        policy.focused();
        let main_blur = policy.lost_focus(1000).unwrap();
        assert!(policy.current(main_blur, 1075));
        // The native foreground check skips minimizing while selection owns focus.
        for label in ["selection-polish", "qa", "capsule"] {
            assert!(tracks_focus_loss(label));
            let external_blur = policy.lost_focus(2000).unwrap();
            assert!(!policy.current(main_blur, 2075));
            assert!(policy.current(external_blur, 2075));
        }
        assert!(!tracks_focus_loss("unrelated"));
    }
    #[test]
    fn focus_failure_does_not_undo_explicit_show() {
        let mut policy = FocusPolicy::new();
        policy.show(0);
        assert!(policy.lost_focus(1000).is_none());
        policy.focused();
        assert!(policy.lost_focus(1000).is_some());
    }
    #[test]
    fn loss_during_grace_is_delayed_not_discarded() {
        for (lost, due) in [(0, 750), (300, 750), (749, 824), (750, 825), (1000, 1075)] {
            let mut policy = FocusPolicy::new();
            policy.show(0);
            policy.focused();
            let job = policy.lost_focus(lost).unwrap();
            assert_eq!(job.due, due);
            assert!(!policy.current(job, due - 1));
            assert!(policy.current(job, due));
        }
    }
    #[test]
    fn show_refocus_close_and_new_loss_cancel_old_jobs() {
        let mut policy = FocusPolicy::new();
        policy.show(0);
        policy.focused();
        let first = policy.lost_focus(300).unwrap();
        policy.show(400);
        assert!(!policy.current(first, 2000));
        policy.focused();
        let second = policy.lost_focus(500).unwrap();
        policy.focused();
        assert!(!policy.current(second, 2000));
        let third = policy.lost_focus(600).unwrap();
        let fourth = policy.lost_focus(650).unwrap();
        assert!(!policy.current(third, 2000));
        assert!(policy.current(fourth, 2000));
        policy.hide();
        assert!(!policy.current(fourth, 2000));
    }
}
