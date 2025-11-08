mod chapters;
mod commands;
mod state;

use anyhow::Result;
use chapters::get_chapter_content;
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
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use state::TutorialStateMachine;
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
        let content = self.get_chapter_content_widget();
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

    fn get_chapter_content_widget(&self) -> Paragraph<'_> {
        let current_state = self.state_machine.current_state();
        let content_lines = get_chapter_content(current_state);

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
