use anyhow::{anyhow, Result};

/// The current state/chapter of the tutorial
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TutorialState {
    /// Setup - building required binaries
    Setup,
    /// Welcome screen - introduction to eHash
    Welcome,
    /// Pool Operator chapter - setting up the Pool with eHash minting
    PoolOperator,
    /// Proxy Operator chapter - creating wallet and starting TProxy
    ProxyOperator,
    /// Pioneer chapter - mining with eHash pubkey
    Pioneer,
    /// Tutorial complete
    Complete,
}

impl TutorialState {
    /// Get the display name for this state
    pub fn display_name(&self) -> &str {
        match self {
            TutorialState::Setup => "Setup",
            TutorialState::Welcome => "Welcome",
            TutorialState::PoolOperator => "Pool Operator",
            TutorialState::ProxyOperator => "Proxy Operator",
            TutorialState::Pioneer => "Pioneer",
            TutorialState::Complete => "Complete",
        }
    }

    /// Get a brief description of this chapter
    pub fn description(&self) -> &str {
        match self {
            TutorialState::Setup => "Build required binaries",
            TutorialState::Welcome => "Learn about eHash and the tutorial structure",
            TutorialState::PoolOperator => "Set up a mining pool with eHash minting",
            TutorialState::ProxyOperator => "Configure a translation proxy with eHash support",
            TutorialState::Pioneer => "Mine and earn eHash tokens",
            TutorialState::Complete => "You've completed the eHash tutorial!",
        }
    }

    /// Get the next state
    pub fn next(&self) -> Option<TutorialState> {
        match self {
            TutorialState::Setup => Some(TutorialState::Welcome),
            TutorialState::Welcome => Some(TutorialState::PoolOperator),
            TutorialState::PoolOperator => Some(TutorialState::ProxyOperator),
            TutorialState::ProxyOperator => Some(TutorialState::Pioneer),
            TutorialState::Pioneer => Some(TutorialState::Complete),
            TutorialState::Complete => None,
        }
    }

    /// Get the previous state
    pub fn previous(&self) -> Option<TutorialState> {
        match self {
            TutorialState::Setup => None,
            TutorialState::Welcome => Some(TutorialState::Setup),
            TutorialState::PoolOperator => Some(TutorialState::Welcome),
            TutorialState::ProxyOperator => Some(TutorialState::PoolOperator),
            TutorialState::Pioneer => Some(TutorialState::ProxyOperator),
            TutorialState::Complete => Some(TutorialState::Pioneer),
        }
    }

    /// Get the chapter number (1-based)
    pub fn chapter_number(&self) -> usize {
        match self {
            TutorialState::Setup => 0,
            TutorialState::Welcome => 1,
            TutorialState::PoolOperator => 2,
            TutorialState::ProxyOperator => 3,
            TutorialState::Pioneer => 4,
            TutorialState::Complete => 5,
        }
    }

    /// Get total number of chapters
    pub fn total_chapters() -> usize {
        5
    }
}

impl Default for TutorialState {
    fn default() -> Self {
        TutorialState::Setup
    }
}

/// Represents a state transition triggered by a command or event
#[derive(Debug, Clone)]
pub enum StateTransition {
    /// Move to the next chapter
    Next,
    /// Move to the previous chapter
    Back,
    /// Jump to a specific state
    JumpTo(TutorialState),
    /// Stay in current state
    Stay,
}

/// Manages the tutorial state machine and transitions
pub struct TutorialStateMachine {
    current_state: TutorialState,
    history: Vec<TutorialState>,
}

impl TutorialStateMachine {
    pub fn new() -> Self {
        Self {
            current_state: TutorialState::default(),
            history: vec![TutorialState::default()],
        }
    }

    /// Get the current state
    pub fn current_state(&self) -> &TutorialState {
        &self.current_state
    }

    /// Apply a state transition
    pub fn transition(&mut self, transition: StateTransition) -> Result<()> {
        match transition {
            StateTransition::Next => {
                if let Some(next_state) = self.current_state.next() {
                    self.current_state = next_state.clone();
                    self.history.push(next_state);
                    Ok(())
                } else {
                    Err(anyhow!("Already at the last chapter"))
                }
            }
            StateTransition::Back => {
                if let Some(prev_state) = self.current_state.previous() {
                    self.current_state = prev_state.clone();
                    self.history.push(prev_state);
                    Ok(())
                } else {
                    Err(anyhow!("Already at the first chapter"))
                }
            }
            StateTransition::JumpTo(state) => {
                self.current_state = state.clone();
                self.history.push(state);
                Ok(())
            }
            StateTransition::Stay => Ok(()),
        }
    }

    /// Handle a navigation command and return the appropriate transition
    pub fn handle_command(&self, command: &str) -> StateTransition {
        match command.trim() {
            "next" => StateTransition::Next,
            "back" => StateTransition::Back,
            _ => StateTransition::Stay,
        }
    }

    /// Check if we can go to the next chapter
    pub fn can_go_next(&self) -> bool {
        self.current_state.next().is_some()
    }

    /// Check if we can go to the previous chapter
    pub fn can_go_back(&self) -> bool {
        self.current_state.previous().is_some()
    }

    /// Get progress information
    pub fn progress(&self) -> (usize, usize) {
        (
            self.current_state.chapter_number(),
            TutorialState::total_chapters(),
        )
    }
}

impl Default for TutorialStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        let mut machine = TutorialStateMachine::new();

        assert_eq!(machine.current_state(), &TutorialState::Welcome);

        // Test forward navigation
        machine.transition(StateTransition::Next).unwrap();
        assert_eq!(machine.current_state(), &TutorialState::PoolOperator);

        machine.transition(StateTransition::Next).unwrap();
        assert_eq!(machine.current_state(), &TutorialState::ProxyOperator);

        // Test backward navigation
        machine.transition(StateTransition::Back).unwrap();
        assert_eq!(machine.current_state(), &TutorialState::PoolOperator);

        // Test jump
        machine
            .transition(StateTransition::JumpTo(TutorialState::Complete))
            .unwrap();
        assert_eq!(machine.current_state(), &TutorialState::Complete);
    }

    #[test]
    fn test_navigation_boundaries() {
        let mut machine = TutorialStateMachine::new();

        // Can't go back from Welcome
        assert!(!machine.can_go_back());
        assert!(machine.transition(StateTransition::Back).is_err());

        // Navigate to end
        while machine.can_go_next() {
            machine.transition(StateTransition::Next).unwrap();
        }

        // Can't go forward from Complete
        assert!(!machine.can_go_next());
        assert!(machine.transition(StateTransition::Next).is_err());
    }

    #[test]
    fn test_handle_command() {
        let machine = TutorialStateMachine::new();

        match machine.handle_command("next") {
            StateTransition::Next => (),
            _ => panic!("Expected Next transition"),
        }

        match machine.handle_command("back") {
            StateTransition::Back => (),
            _ => panic!("Expected Back transition"),
        }

        match machine.handle_command("something") {
            StateTransition::Stay => (),
            _ => panic!("Expected Stay transition"),
        }
    }
}
