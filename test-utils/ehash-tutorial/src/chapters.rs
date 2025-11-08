use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::state::TutorialState;

/// Get the content lines for a specific chapter
pub fn get_chapter_content(state: &TutorialState) -> Vec<Line<'static>> {
    match state {
        TutorialState::Welcome => welcome_content(),
        TutorialState::PoolOperator => pool_operator_content(),
        TutorialState::ProxyOperator => proxy_operator_content(),
        TutorialState::Pioneer => pioneer_content(),
        TutorialState::Complete => complete_content(),
    }
}

fn welcome_content() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from(Span::styled(
            "Welcome to the eHash Tutorial!",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("This interactive tutorial will guide you through setting up and using"),
        Line::from("the eHash protocol with Stratum v2 mining."),
        Line::from(""),
        Line::from(Span::styled(
            "What you'll learn:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from("  • How to set up a Pool with eHash minting"),
        Line::from("  • How to configure a Translation Proxy with eHash support"),
        Line::from("  • How to mine with eHash and earn tokens"),
        Line::from(""),
        Line::from(Span::styled(
            "Guided CLI Session:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from("This tutorial uses real production CLI commands in a controlled,"),
        Line::from("secure environment. You'll type actual commands like:"),
        Line::from(""),
        Line::from(Span::styled(
            "  pool_sv2 --config pool-config-ehash.toml",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            "  cdk-cli wallet create --name my-wallet",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            "  mining_device --pool-address 127.0.0.1:34255",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from("• Tab completion helps you discover valid commands"),
        Line::from("• Command history (↑↓) lets you recall previous commands"),
        Line::from("• Only whitelisted commands are allowed for security"),
        Line::from(""),
        Line::from(Span::styled(
            "Ready to begin?",
            Style::default().fg(Color::Green),
        )),
        Line::from("Type 'next' to start, or 'help' to see available commands."),
    ]
}

fn pool_operator_content() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from(Span::styled(
            "Chapter 1: Pool Operator",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Welcome, Pool Operator! Let's set up your eHash mint."),
        Line::from(""),
        Line::from(Span::styled(
            "What is a Pool?",
            Style::default().fg(Color::Yellow),
        )),
        Line::from("The Pool is responsible for:"),
        Line::from("  • Coordinating mining work distribution to miners"),
        Line::from("  • Validating submitted shares from miners"),
        Line::from("  • Minting eHash tokens for valid shares"),
        Line::from("  • Managing the eHash mint backend (Cashu)"),
        Line::from(""),
        Line::from(Span::styled(
            "Your Task:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from("Start the Pool with eHash minting enabled by running:"),
        Line::from(""),
        Line::from(Span::styled(
            "  pool_sv2 --config pool-config-ehash.toml",
            Style::default().fg(Color::Green),
        )),
        Line::from(""),
        Line::from("This will:"),
        Line::from("  1. Launch the Pool process with Stratum v2 protocol"),
        Line::from("  2. Enable eHash minting for incoming shares"),
        Line::from("  3. Start listening for miner connections"),
        Line::from(""),
        Line::from(Span::styled(
            "Tip:",
            Style::default().fg(Color::Magenta),
        )),
        Line::from("Use Tab for command completion. The config file will be"),
        Line::from("generated automatically with correct eHash settings."),
        Line::from(""),
        Line::from("Type the command above to continue."),
    ]
}

fn proxy_operator_content() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from(Span::styled(
            "Chapter 2: Proxy Operator",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Great! Now let's set up the Translation Proxy."),
        Line::from(""),
        Line::from(Span::styled(
            "What is a Translation Proxy?",
            Style::default().fg(Color::Yellow),
        )),
        Line::from("The Proxy (TProxy) acts as a bridge that:"),
        Line::from("  • Translates between Stratum v1 and v2 protocols"),
        Line::from("  • Allows v1 miners to participate in eHash mining"),
        Line::from("  • Locks eHash tokens to your pubkey (hpub)"),
        Line::from("  • Aggregates work from multiple v1 miners"),
        Line::from(""),
        Line::from(Span::styled(
            "Your Tasks:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "1. Create a wallet to receive eHash tokens:",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "   cdk-cli wallet create --name proxy-wallet --mint-url http://127.0.0.1:3338",
            Style::default().fg(Color::Green),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "2. Get your wallet info and hpub address:",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "   cdk-cli wallet info proxy-wallet",
            Style::default().fg(Color::Green),
        )),
        Line::from("   (The hpub will be used for locking eHash tokens)"),
        Line::from(""),
        Line::from(Span::styled(
            "3. Start the Translation Proxy:",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "   translator_sv2 --config tproxy-config-ehash.toml",
            Style::default().fg(Color::Green),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Note:",
            Style::default().fg(Color::Magenta),
        )),
        Line::from("In Phase 2, these commands will actually execute and create"),
        Line::from("real wallets and processes. For now, just practice the syntax!"),
    ]
}

fn pioneer_content() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from(Span::styled(
            "Chapter 3: Pioneer (Miner)",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Excellent! Now let's start mining and earning eHash tokens."),
        Line::from(""),
        Line::from(Span::styled(
            "What is a Pioneer?",
            Style::default().fg(Color::Yellow),
        )),
        Line::from("A Pioneer is a miner who:"),
        Line::from("  • Contributes computational work to find valid shares"),
        Line::from("  • Uses their unique hpub address for token rewards"),
        Line::from("  • Earns eHash tokens proportional to their work"),
        Line::from("  • Can redeem tokens from the Cashu mint"),
        Line::from(""),
        Line::from(Span::styled(
            "Your Tasks:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "1. Create your mining wallet:",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "   cdk-cli wallet create --name pioneer-wallet --mint-url http://127.0.0.1:3338",
            Style::default().fg(Color::Green),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "2. Get your hpub for mining rewards:",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "   cdk-cli wallet info pioneer-wallet",
            Style::default().fg(Color::Green),
        )),
        Line::from("   (Copy the hpub address from the output)"),
        Line::from(""),
        Line::from(Span::styled(
            "3. Start mining (replace <hpub> with your actual hpub):",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "   mining_device --pool-address 127.0.0.1:34255 --user-identity <hpub>",
            Style::default().fg(Color::Green),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "4. Check your balance:",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "   cdk-cli wallet balance pioneer-wallet",
            Style::default().fg(Color::Green),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Bonus Commands:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from("  • Query mint directly: curl http://127.0.0.1:3338/v1/info"),
        Line::from("  • Check processes: ps aux | grep mining_device"),
        Line::from(""),
        Line::from("Practice these commands to complete the tutorial!"),
    ]
}

fn complete_content() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from(Span::styled(
            "🎉 Congratulations! 🎉",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("You've completed the eHash tutorial!"),
        Line::from(""),
        Line::from(Span::styled(
            "What You've Learned:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  ✓ Pool Operator Role",
            Style::default().fg(Color::Green),
        )),
        Line::from("    - Starting a Pool with eHash minting enabled"),
        Line::from("    - Understanding the pool_sv2 command and configuration"),
        Line::from(""),
        Line::from(Span::styled(
            "  ✓ Proxy Operator Role",
            Style::default().fg(Color::Green),
        )),
        Line::from("    - Creating Cashu wallets with cdk-cli"),
        Line::from("    - Deriving hpub addresses for token locking"),
        Line::from("    - Starting a Translation Proxy with translator_sv2"),
        Line::from(""),
        Line::from(Span::styled(
            "  ✓ Pioneer (Miner) Role",
            Style::default().fg(Color::Green),
        )),
        Line::from("    - Creating a mining wallet"),
        Line::from("    - Mining with unique hpub identity"),
        Line::from("    - Checking balances and managing tokens"),
        Line::from(""),
        Line::from(Span::styled(
            "Next Steps:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("In Phase 2, you'll be able to actually execute these commands"),
        Line::from("and see real processes running, tokens being minted, and"),
        Line::from("shares being submitted!"),
        Line::from(""),
        Line::from("You now have hands-on experience with the complete eHash"),
        Line::from("ecosystem and are ready to deploy it in production."),
        Line::from(""),
        Line::from(Span::styled(
            "Commands to remember:",
            Style::default().fg(Color::Cyan),
        )),
        Line::from("  • Review chapters: type 'back'"),
        Line::from("  • Get help: type 'help'"),
        Line::from("  • Exit tutorial: press Ctrl+C or Esc"),
        Line::from(""),
        Line::from("Thank you for completing the eHash tutorial!"),
    ]
}
