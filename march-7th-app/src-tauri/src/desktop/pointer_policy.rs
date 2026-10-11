#[derive(Default)]
pub struct PointerPolicy {
    sequence: u64,
    owner: Option<String>,
    automatic: bool,
    applied: bool,
}

pub struct PointerRequest<'a> {
    pub sequence: u64,
    pub owner: &'a str,
    pub allowed: bool,
    pub ignore: bool,
    pub button_down: bool,
    pub manual_ignore: bool,
}

impl PointerPolicy {
    pub fn request(
        &mut self,
        request: PointerRequest<'_>,
        apply: impl FnOnce(bool) -> Result<(), String>,
    ) -> Result<bool, String> {
        let PointerRequest {
            sequence,
            owner,
            allowed,
            ignore,
            button_down,
            manual_ignore,
        } = request;
        if sequence <= self.sequence {
            return Ok(self.applied);
        }
        self.sequence = sequence;
        self.owner = allowed.then(|| owner.to_owned());
        self.automatic = allowed && ignore;
        self.refresh(allowed, button_down, manual_ignore, apply)
    }

    pub fn owned_by(&self, owner: &str) -> bool {
        self.owner.as_deref() == Some(owner)
    }

    pub fn refresh(
        &mut self,
        allowed: bool,
        button_down: bool,
        manual_ignore: bool,
        apply: impl FnOnce(bool) -> Result<(), String>,
    ) -> Result<bool, String> {
        if !allowed {
            self.owner = None;
            self.automatic = false;
        }
        // Preserve the receiving window during a held button: a pet drag must
        // stay interactive, and dragging a window underneath must stay ignored.
        let automatic = if allowed && button_down {
            self.applied
        } else {
            self.automatic
        };
        let next = manual_ignore || automatic;
        if next != self.applied {
            apply(next)?;
            self.applied = next;
        }
        Ok(self.applied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(_: bool) -> Result<(), String> {
        Ok(())
    }

    fn request(
        sequence: u64,
        owner: &str,
        allowed: bool,
        ignore: bool,
        button_down: bool,
        manual_ignore: bool,
    ) -> PointerRequest<'_> {
        PointerRequest {
            sequence,
            owner,
            allowed,
            ignore,
            button_down,
            manual_ignore,
        }
    }

    #[test]
    fn blank_is_ignored_but_visible_character_is_interactive() {
        let mut policy = PointerPolicy::default();
        assert!(policy
            .request(request(1, "capable", true, true, false, false), apply)
            .unwrap());
        assert!(!policy
            .request(request(2, "capable", true, false, false, false), apply)
            .unwrap());
    }

    #[test]
    fn pressing_or_dragging_keeps_native_input_until_button_release() {
        let mut policy = PointerPolicy::default();
        assert!(!policy
            .request(request(1, "capable", true, true, true, false), apply)
            .unwrap());
        assert!(policy
            .request(request(2, "capable", true, true, false, false), apply)
            .unwrap());
    }

    #[test]
    fn manual_click_through_always_has_priority() {
        let mut policy = PointerPolicy::default();
        assert!(policy
            .request(request(1, "capable", true, false, true, true), apply)
            .unwrap());
        assert!(policy
            .request(request(2, "legacy", false, false, false, true), apply)
            .unwrap());
    }

    #[test]
    fn switching_to_legacy_restores_input_and_rejects_late_old_requests() {
        let mut policy = PointerPolicy::default();
        assert!(policy
            .request(request(1, "capable", true, true, false, false), apply)
            .unwrap());
        assert!(!policy
            .request(request(3, "legacy", false, false, false, false), apply)
            .unwrap());
        assert!(!policy
            .request(request(2, "capable", true, true, false, false), apply)
            .unwrap());
    }

    #[test]
    fn a_failed_native_change_keeps_the_previous_effective_state() {
        let mut policy = PointerPolicy::default();
        assert!(policy
            .request(request(1, "capable", true, true, false, false), |_| Err(
                "failed".into()
            ))
            .is_err());
        assert!(!policy.applied);
        assert!(!policy
            .request(request(2, "capable", true, false, false, false), apply)
            .unwrap());
    }

    #[test]
    fn a_drag_in_the_underlying_app_does_not_get_stolen_when_crossing_the_pet() {
        let mut policy = PointerPolicy::default();
        assert!(policy
            .request(request(1, "capable", true, true, false, false), apply)
            .unwrap());
        assert!(policy
            .request(request(2, "capable", true, false, true, false), apply)
            .unwrap());
        assert!(!policy
            .request(request(3, "capable", true, false, false, false), apply)
            .unwrap());
    }

    #[test]
    fn switching_characters_clears_only_automatic_passthrough() {
        let mut policy = PointerPolicy::default();
        policy
            .request(request(1, "capable", true, true, false, false), apply)
            .unwrap();
        assert!(!policy.refresh(false, true, false, apply).unwrap());
        assert!(!policy.owned_by("capable"));
        assert!(policy.refresh(false, false, true, apply).unwrap());
    }

    #[test]
    fn an_old_character_request_cannot_enable_a_non_capable_current_selection() {
        let mut policy = PointerPolicy::default();
        assert!(!policy
            .request(request(1, "old-capable", false, true, false, false), apply)
            .unwrap());
        assert!(policy
            .request(request(2, "old-capable", false, true, false, true), apply)
            .unwrap());
    }
}
