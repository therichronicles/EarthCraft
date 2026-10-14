//! Remember sources independently of preferences and cancel stale completions.
use std::{collections::VecDeque, path::PathBuf};

#[derive(Default)]
pub(in crate::app) struct KmlSession {
    pub(in crate::app) paths: Vec<PathBuf>,
    pending: VecDeque<bool>,
}

impl KmlSession {
    pub(in crate::app) fn submitted(&mut self) {
        self.pending.push_back(true);
    }

    pub(in crate::app) fn completed(&mut self) -> bool {
        self.pending.pop_front().unwrap_or(false)
    }

    pub(in crate::app) fn remember(&mut self, path: PathBuf) {
        if !self.paths.contains(&path) {
            self.paths.push(path);
        }
    }

    pub(in crate::app) fn clear(&mut self) {
        self.paths.clear();
        for pending in &mut self.pending {
            *pending = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_discards_pending_imports_and_saves_but_accepts_later_requests() {
        let mut session = KmlSession::default();
        session.remember("first.kml".into());
        session.remember("first.kml".into());
        assert_eq!(session.paths.len(), 1);
        session.submitted();
        session.submitted();
        session.clear();
        session.submitted();
        assert!(session.paths.is_empty());
        assert!(!session.completed());
        assert!(!session.completed());
        assert!(session.completed());
    }
}
