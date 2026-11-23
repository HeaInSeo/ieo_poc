//! Policy decisions for one review lineage. Callers must persist transitions under
//! the assignment lock and revalidate external evidence before acting on them.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewPhase {
    Development,
    Pending,
    Correction,
    Approved,
    Blocked,
    TimedOut,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewVerdict {
    Approve,
    Reject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewError {
    WrongPhase,
    WrongCandidate,
    StaleGeneration,
    ReviewLimit,
    InvalidGeneration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewActivity {
    Unclaimed,
    Claimed,
    InProgress,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewTimeout {
    Waiting,
    Warning(ReviewActivity),
    ReplacementCandidate(ReviewActivity),
    Blocked(ReviewActivity),
}

#[derive(Debug, Eq, PartialEq)]
pub struct ReviewLifecycle {
    phase: ReviewPhase,
    candidate: Option<String>,
    generation: Option<u64>,
    reviews: u8,
    corrections: u8,
    unauthorized_head_change: bool,
    expired: bool,
}

impl Default for ReviewLifecycle {
    fn default() -> Self {
        Self {
            phase: ReviewPhase::Development,
            candidate: None,
            generation: None,
            reviews: 0,
            corrections: 0,
            unauthorized_head_change: false,
            expired: false,
        }
    }
}

impl ReviewLifecycle {
    const REVIEW_LIMIT: u8 = 4;
    const CORRECTION_LIMIT: u8 = 3;

    pub fn phase(&self) -> ReviewPhase {
        self.phase
    }

    pub fn developer_push_allowed(&self) -> bool {
        matches!(
            self.phase,
            ReviewPhase::Development | ReviewPhase::Correction
        )
    }

    pub(crate) fn review_delivery_allowed(&self, head: &str, generation: u64) -> bool {
        self.phase == ReviewPhase::Pending
            && self.candidate.as_deref() == Some(head)
            && self.generation == Some(generation)
    }

    pub fn unauthorized_head_change(&self) -> bool {
        self.unauthorized_head_change
    }

    pub fn submit(&mut self, head: &str, generation: u64) -> Result<(), ReviewError> {
        if !self.developer_push_allowed() {
            return Err(ReviewError::WrongPhase);
        }
        if self.candidate.as_deref() == Some(head) {
            return Err(ReviewError::WrongCandidate);
        }
        self.start_review(head, generation)
    }

    /// Invalidate the old review even when the new candidate cannot be assigned.
    pub fn observe_head_change(&mut self, head: &str, generation: u64) -> Result<(), ReviewError> {
        self.invalidate_head(head)?;
        self.reassign_invalidated(head, generation)
    }

    pub(crate) fn invalidate_head(&mut self, head: &str) -> Result<(), ReviewError> {
        if !self.expired && !matches!(self.phase, ReviewPhase::Pending | ReviewPhase::Approved) {
            return Err(ReviewError::WrongPhase);
        }
        if self.candidate.as_deref() == Some(head) {
            return Err(ReviewError::WrongCandidate);
        }
        self.unauthorized_head_change = true;
        self.expired = false;
        self.phase = ReviewPhase::Blocked;
        Ok(())
    }

    pub(crate) fn reassign_invalidated(
        &mut self,
        head: &str,
        generation: u64,
    ) -> Result<(), ReviewError> {
        if self.phase != ReviewPhase::Blocked || !self.unauthorized_head_change {
            return Err(ReviewError::WrongPhase);
        }
        if self.candidate.as_deref() == Some(head) {
            return Err(ReviewError::WrongCandidate);
        }
        self.start_review(head, generation)
    }

    pub(crate) fn review_round(&self) -> u64 {
        self.reviews.into()
    }

    pub(crate) fn expire(&mut self, head: &str, can_replace: bool) -> Result<(), ReviewError> {
        if self.phase != ReviewPhase::Pending {
            return Err(ReviewError::WrongPhase);
        }
        if self.candidate.as_deref() != Some(head) {
            return Err(ReviewError::WrongCandidate);
        }
        self.expired = true;
        self.phase = if can_replace {
            ReviewPhase::TimedOut
        } else {
            ReviewPhase::Blocked
        };
        Ok(())
    }

    pub(crate) fn replace_expired(&mut self, head: &str, round: u64) -> Result<(), ReviewError> {
        if self.phase != ReviewPhase::TimedOut {
            return Err(ReviewError::WrongPhase);
        }
        if self.candidate.as_deref() != Some(head) {
            return Err(ReviewError::WrongCandidate);
        }
        self.start_review(head, round)
    }

    pub fn resolve(
        &mut self,
        head: &str,
        generation: u64,
        verdict: ReviewVerdict,
    ) -> Result<(), ReviewError> {
        if self.generation != Some(generation) {
            return Err(ReviewError::StaleGeneration);
        }
        if self.candidate.as_deref() != Some(head) {
            return Err(ReviewError::WrongCandidate);
        }
        if self.phase != ReviewPhase::Pending {
            return Err(ReviewError::WrongPhase);
        }
        self.phase = match verdict {
            ReviewVerdict::Approve => ReviewPhase::Approved,
            ReviewVerdict::Reject if self.reviews == Self::REVIEW_LIMIT => ReviewPhase::Blocked,
            ReviewVerdict::Reject if self.corrections == Self::CORRECTION_LIMIT => {
                ReviewPhase::Blocked
            }
            ReviewVerdict::Reject => {
                self.corrections += 1;
                ReviewPhase::Correction
            }
        };
        Ok(())
    }

    /// Classify the elapsed time without scheduling or granting write authority.
    /// Policy thresholds are supplied by the caller; a replacement still needs
    /// the prior assignment fenced and the decision committed before dispatch.
    pub fn timeout(
        &self,
        elapsed_seconds: u64,
        warning_seconds: u64,
        limit_seconds: u64,
        activity: ReviewActivity,
        replacement_already_used: bool,
    ) -> Option<ReviewTimeout> {
        if self.phase != ReviewPhase::Pending || warning_seconds >= limit_seconds {
            return None;
        }
        Some(if elapsed_seconds >= limit_seconds {
            if replacement_already_used || self.reviews >= Self::REVIEW_LIMIT {
                ReviewTimeout::Blocked(activity)
            } else {
                ReviewTimeout::ReplacementCandidate(activity)
            }
        } else if elapsed_seconds >= warning_seconds {
            ReviewTimeout::Warning(activity)
        } else {
            ReviewTimeout::Waiting
        })
    }

    /// A fresh approval alone cannot clear a recorded unauthorized push.
    pub fn approved_head(&self) -> Option<&str> {
        if self.phase == ReviewPhase::Approved && !self.unauthorized_head_change {
            self.candidate.as_deref()
        } else {
            None
        }
    }

    fn start_review(&mut self, head: &str, generation: u64) -> Result<(), ReviewError> {
        if self.reviews == Self::REVIEW_LIMIT {
            self.phase = ReviewPhase::Blocked;
            return Err(ReviewError::ReviewLimit);
        }
        if self.generation.is_some_and(|prior| generation <= prior) {
            return Err(ReviewError::InvalidGeneration);
        }
        self.reviews += 1;
        self.expired = false;
        self.candidate = Some(head.to_owned());
        self.generation = Some(generation);
        self.phase = ReviewPhase::Pending;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correction_reopens_push_then_rechecks_the_new_candidate() {
        let mut state = ReviewLifecycle::default();
        state.submit("a", 0).unwrap();
        assert!(!state.developer_push_allowed());
        state.resolve("a", 0, ReviewVerdict::Reject).unwrap();
        assert!(state.developer_push_allowed());
        state.submit("b", 1).unwrap();
        assert_eq!(
            state.resolve("a", 0, ReviewVerdict::Approve),
            Err(ReviewError::StaleGeneration)
        );
        state.resolve("b", 1, ReviewVerdict::Approve).unwrap();
        assert_eq!(state.approved_head(), Some("b"));
    }

    #[test]
    fn unauthorized_head_change_invalidates_approval_even_without_reassignment() {
        let mut state = ReviewLifecycle::default();
        state.submit("a", 0).unwrap();
        state.resolve("a", 0, ReviewVerdict::Approve).unwrap();
        assert_eq!(
            state.observe_head_change("b", 0),
            Err(ReviewError::InvalidGeneration)
        );
        assert_eq!(state.approved_head(), None);
        assert!(state.unauthorized_head_change());
    }

    #[test]
    fn new_approval_does_not_erase_an_unauthorized_push() {
        let mut state = ReviewLifecycle::default();
        state.submit("a", 0).unwrap();
        state.observe_head_change("b", 1).unwrap();
        state.resolve("b", 1, ReviewVerdict::Approve).unwrap();
        assert_eq!(state.approved_head(), None);
    }

    #[test]
    fn timeout_classifies_boundaries_without_changing_review_state() {
        let mut state = ReviewLifecycle::default();
        state.submit("a", 0).unwrap();
        for activity in [
            ReviewActivity::Unclaimed,
            ReviewActivity::Claimed,
            ReviewActivity::InProgress,
        ] {
            assert_eq!(
                state.timeout(1799, 1800, 3600, activity, false),
                Some(ReviewTimeout::Waiting)
            );
            assert_eq!(
                state.timeout(1800, 1800, 3600, activity, false),
                Some(ReviewTimeout::Warning(activity))
            );
            assert_eq!(
                state.timeout(3599, 1800, 3600, activity, false),
                Some(ReviewTimeout::Warning(activity))
            );
            assert_eq!(
                state.timeout(3600, 1800, 3600, activity, false),
                Some(ReviewTimeout::ReplacementCandidate(activity))
            );
            assert_eq!(
                state.timeout(3600, 1800, 3600, activity, true),
                Some(ReviewTimeout::Blocked(activity))
            );
        }
        assert_eq!(state.phase(), ReviewPhase::Pending);
        assert_eq!(state.approved_head(), None);
    }

    #[test]
    fn fourth_rejection_blocks_further_correction() {
        let mut state = ReviewLifecycle::default();
        for generation in 0..4 {
            let candidate = generation.to_string();
            state.submit(&candidate, generation).unwrap();
            state
                .resolve(&candidate, generation, ReviewVerdict::Reject)
                .unwrap();
        }
        assert_eq!(state.phase(), ReviewPhase::Blocked);
        assert!(!state.developer_push_allowed());
        assert_eq!(state.submit("extra", 4), Err(ReviewError::WrongPhase));
    }
}
