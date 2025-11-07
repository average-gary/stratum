mod commands;
mod state;

use anyhow::Result;
use commands::CommandSystem;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use state::{TutorialState, TutorialStateMachine};
use std::io;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

struct App {
    should_quit: bool,
    state_machine: TutorialStateMachine,
    command_system: CommandSystem,
    input: Input,
    command_history: Vec<String>,
    history_index: Option<usize>,
    message: Option<String>,
    show_help: bool,
}

impl App {
    fn new() -> Self {
        Self {
            should_quit: false,
            state_machine: TutorialStateMachine::new(),
            command_system: CommandSystem::new(),
            input: Input::default(),
            command_history: Vec::new(),
            history_index: None,
            message: None,
            show_help: false,
        }
    }

    fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Run main loop
        let res = self.main_loop(&mut terminal);

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        // Return result
        res
    }

    fn main_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|f| self.ui(f))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn ui(&self, f: &mut ratatui::Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header
                Constraint::Min(0),     // Content
                Constraint::Length(8),  // Command input + help
                Constraint::Length(2),  // Footer
            ])
            .split(f.area());

        // Header with chapter info
        let current_state = self.state_machine.current_state();
        let (current, total) = self.state_machine.progress();
        let header_text = format!(
            "Interactive eHash Tutorial - {} ({}/{})",
            current_state.display_name(),
            current,
            total
        );
        let header = Paragraph::new(header_text)
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(header, chunks[0]);

        // Content - chapter-specific
        let content = self.get_chapter_content();
        f.render_widget(content, chunks[1]);

        // Command input area
        let input_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Input field
                Constraint::Length(5),  // Help text or message
            ])
            .split(chunks[2]);

        // Input field
        let width = input_chunks[0].width.max(3) - 3;
        let scroll = self.input.visual_scroll(width as usize);
        let input_widget = Paragraph::new(self.input.value())
            .style(Style::default().fg(Color::Yellow))
            .scroll((0, scroll as u16))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Command (Tab for completion, ↑↓ for history)")
            );
        f.render_widget(input_widget, input_chunks[0]);

        // Show cursor in input
        f.set_cursor_position((
            input_chunks[0].x + ((self.input.visual_cursor()).max(scroll) - scroll) as u16 + 1,
            input_chunks[0].y + 1,
        ));

        // Help text or message
        let help_content = if self.show_help {
            let commands = self.command_system.get_available_commands(current_state.display_name());
            Paragraph::new(commands.join("\n"))
                .style(Style::default().fg(Color::Green))
                .wrap(Wrap { trim: true })
                .block(Block::default().borders(Borders::ALL).title("Available Commands"))
        } else if let Some(ref msg) = self.message {
            Paragraph::new(msg.as_str())
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: true })
                .block(Block::default().borders(Borders::ALL).title("Message"))
        } else {
            Paragraph::new("Type 'help' to see available commands")
                .style(Style::default().fg(Color::Gray))
                .block(Block::default().borders(Borders::ALL).title("Hint"))
        };
        f.render_widget(help_content, input_chunks[1]);

        // Footer
        let footer_text = "Ctrl+C or Esc to quit | Enter to execute command | Tab for completion";
        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(footer, chunks[3]);
    }

    fn get_chapter_content(&self) -> Paragraph<'_> {
        let current_state = self.state_machine.current_state();

        let content_lines = match current_state {
            TutorialState::Welcome => vec![
                Line::from(""),
                Line::from(Span::styled("Welcome to the eHash Tutorial!", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("This interactive tutorial will guide you through setting up and using"),
                Line::from("the eHash protocol with Stratum v2 mining."),
                Line::from(""),
                Line::from(Span::styled("What you'll learn:", Style::default().fg(Color::Yellow))),
                Line::from("  • How to set up a Pool with eHash minting"),
                Line::from("  • How to configure a Translation Proxy with eHash support"),
                Line::from("  • How to mine with eHash and earn tokens"),
                Line::from(""),
                Line::from("This tutorial uses real CLI commands in a controlled environment."),
                Line::from("Type 'next' to begin, or 'help' to see available commands."),
            ],
            TutorialState::PoolOperator => vec![
                Line::from(""),
                Line::from(Span::styled("Chapter 1: Pool Operator", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Welcome, Pool Operator! Let's set up your eHash mint."),
                Line::from(""),
                Line::from("The Pool is responsible for:"),
                Line::from("  • Coordinating mining work with miners"),
                Line::from("  • Minting eHash tokens for valid shares"),
                Line::from("  • Distributing tokens based on contribution"),
                Line::from(""),
                Line::from(Span::styled("Your task:", Style::default().fg(Color::Yellow))),
                Line::from("Start the Pool with eHash minting enabled by running:"),
                Line::from(""),
                Line::from(Span::styled("  pool_sv2 --config pool-config-ehash.toml", Style::default().fg(Color::Green))),
                Line::from(""),
                Line::from("Type the command above to continue. Use Tab for completion."),
            ],
            TutorialState::ProxyOperator => vec![
                Line::from(""),
                Line::from(Span::styled("Chapter 2: Proxy Operator", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Great! Now let's set up the Translation Proxy."),
                Line::from(""),
                Line::from("The Proxy will:"),
                Line::from("  • Translate between Stratum v1 and v2 protocols"),
                Line::from("  • Lock eHash tokens to your pubkey"),
                Line::from("  • Enable v1 miners to participate in eHash"),
                Line::from(""),
                Line::from(Span::styled("Steps:", Style::default().fg(Color::Yellow))),
                Line::from("1. Create a wallet to receive eHash tokens:"),
                Line::from(Span::styled("   cdk-cli wallet create --name proxy-wallet --mint-url http://127.0.0.1:3338", Style::default().fg(Color::Green))),
                Line::from(""),
                Line::from("2. Get your wallet info and hpub:"),
                Line::from(Span::styled("   cdk-cli wallet info proxy-wallet", Style::default().fg(Color::Green))),
                Line::from(""),
                Line::from("3. Start the Translation Proxy:"),
                Line::from(Span::styled("   translator_sv2 --config tproxy-config-ehash.toml", Style::default().fg(Color::Green))),
            ],
            TutorialState::Pioneer => vec![
                Line::from(""),
                Line::from(Span::styled("Chapter 3: Pioneer (Miner)", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("Excellent! Now let's start mining and earning eHash tokens."),
                Line::from(""),
                Line::from("As a Pioneer, you will:"),
                Line::from("  • Create your own wallet"),
                Line::from("  • Mine using your unique hpub address"),
                Line::from("  • Earn eHash tokens for your work"),
                Line::from(""),
                Line::from(Span::styled("Steps:", Style::default().fg(Color::Yellow))),
                Line::from("1. Create your mining wallet:"),
                Line::from(Span::styled("   cdk-cli wallet create --name pioneer-wallet --mint-url http://127.0.0.1:3338", Style::default().fg(Color::Green))),
                Line::from(""),
                Line::from("2. Get your hpub for mining:"),
                Line::from(Span::styled("   cdk-cli wallet info pioneer-wallet", Style::default().fg(Color::Green))),
                Line::from(""),
                Line::from("3. Start mining (replace <hpub> with your actual hpub):"),
                Line::from(Span::styled("   mining_device --pool-address 127.0.0.1:34255 --user-identity <hpub>", Style::default().fg(Color::Green))),
                Line::from(""),
                Line::from("4. Check your balance:"),
                Line::from(Span::styled("   cdk-cli wallet balance pioneer-wallet", Style::default().fg(Color::Green))),
            ],
            TutorialState::Complete => vec![
                Line::from(""),
                Line::from(Span::styled("Congratulations!", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("You've completed the eHash tutorial!"),
                Line::from(""),
                Line::from(Span::styled("What you've learned:", Style::default().fg(Color::Yellow))),
                Line::from("  ✓ Setting up a Pool with eHash minting"),
                Line::from("  ✓ Configuring a Translation Proxy"),
                Line::from("  ✓ Creating Cashu wallets"),
                Line::from("  ✓ Mining with eHash pubkeys"),
                Line::from("  ✓ Managing eHash tokens"),
                Line::from(""),
                Line::from("You now have hands-on experience with the complete eHash ecosystem!"),
                Line::from(""),
                Line::from("Type 'back' to review previous chapters, or press Ctrl+C to exit."),
            ],
        };

        Paragraph::new(content_lines)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Tutorial Content"))
    }

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.should_quit = true;
                    }
                    KeyCode::Esc => {
                        self.should_quit = true;
                    }
                    KeyCode::Enter => {
                        self.execute_command();
                    }
                    KeyCode::Tab => {
                        self.handle_tab_completion();
                    }
                    KeyCode::Up => {
                        self.navigate_history_up();
                    }
                    KeyCode::Down => {
                        self.navigate_history_down();
                    }
                    _ => {
                        // Reset help display when typing
                        self.show_help = false;
                        self.history_index = None;
                        // Handle normal input
                        self.input.handle_event(&Event::Key(key));
                    }
                }
            }
        }
        Ok(())
    }

    fn execute_command(&mut self) {
        let command = self.input.value().trim().to_string();

        if command.is_empty() {
            return;
        }

        // Add to history
        if !self.command_history.contains(&command) {
            self.command_history.push(command.clone());
        }

        // Clear input
        self.input.reset();
        self.history_index = None;

        // Handle special commands
        if command == "help" {
            self.show_help = true;
            self.message = None;
            return;
        }

        self.show_help = false;

        // Validate command
        match self.command_system.validate_command(&command) {
            Ok(()) => {
                // Handle navigation commands
                if self.command_system.is_navigation_command(&command) {
                    let transition = self.state_machine.handle_command(&command);
                    match self.state_machine.transition(transition) {
                        Ok(()) => {
                            self.message = Some(format!("Navigated to: {}", self.state_machine.current_state().display_name()));
                        }
                        Err(e) => {
                            self.message = Some(format!("Navigation error: {}", e));
                        }
                    }
                } else {
                    // For now, just acknowledge the command
                    // In Phase 2, we'll actually execute these commands
                    self.message = Some(format!("Command accepted: {} (execution in Phase 2)", command));
                }
            }
            Err(e) => {
                self.message = Some(format!("Error: {}", e));
            }
        }
    }

    fn handle_tab_completion(&mut self) {
        let partial = self.input.value();
        let completions = self.command_system.get_completions(partial);

        if completions.is_empty() {
            self.message = Some("No completions available".to_string());
        } else if completions.len() == 1 {
            // Single completion - apply it
            self.input = Input::default().with_value(completions[0].clone());
            // Move cursor to end
            for _ in 0..completions[0].len() {
                self.input.handle_event(&Event::Key(event::KeyEvent::from(KeyCode::Right)));
            }
            self.message = None;
        } else {
            // Multiple completions - show them
            self.message = Some(format!("Completions: {}", completions.join(", ")));
        }
    }

    fn navigate_history_up(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None => self.command_history.len() - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        };

        self.history_index = Some(new_index);
        self.input = Input::default().with_value(self.command_history[new_index].clone());

        // Move cursor to end
        for _ in 0..self.command_history[new_index].len() {
            self.input.handle_event(&Event::Key(event::KeyEvent::from(KeyCode::Right)));
        }
    }

    fn navigate_history_down(&mut self) {
        if self.command_history.is_empty() || self.history_index.is_none() {
            return;
        }

        let current_index = self.history_index.unwrap();
        if current_index >= self.command_history.len() - 1 {
            // At the end - clear input
            self.history_index = None;
            self.input.reset();
        } else {
            let new_index = current_index + 1;
            self.history_index = Some(new_index);
            self.input = Input::default().with_value(self.command_history[new_index].clone());

            // Move cursor to end
            for _ in 0..self.command_history[new_index].len() {
                self.input.handle_event(&Event::Key(event::KeyEvent::from(KeyCode::Right)));
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut app = App::new();
    app.run()?;
    Ok(())
}
