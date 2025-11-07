use anyhow::{anyhow, Result};
use std::collections::HashMap;

/// Represents a whitelisted command with its template and allowed parameters
#[derive(Debug, Clone)]
pub struct CommandTemplate {
    /// The base command (e.g., "pool_sv2", "cdk-cli")
    pub base: String,
    /// Full command pattern with placeholders (e.g., "pool_sv2 --config {config_file}")
    pub pattern: String,
    /// Description of what the command does
    pub description: String,
    /// List of valid completions for placeholders
    pub placeholders: HashMap<String, Vec<String>>,
}

impl CommandTemplate {
    pub fn new(base: &str, pattern: &str, description: &str) -> Self {
        Self {
            base: base.to_string(),
            pattern: pattern.to_string(),
            description: description.to_string(),
            placeholders: HashMap::new(),
        }
    }

    pub fn with_placeholder(mut self, name: &str, values: Vec<String>) -> Self {
        self.placeholders.insert(name.to_string(), values);
        self
    }

    /// Check if a command matches this template
    pub fn matches(&self, command: &str) -> bool {
        if !command.starts_with(&self.base) {
            return false;
        }

        // Simple pattern matching - can be enhanced later
        let pattern_parts: Vec<&str> = self.pattern.split_whitespace().collect();
        let command_parts: Vec<&str> = command.split_whitespace().collect();

        if pattern_parts.len() != command_parts.len() {
            return false;
        }

        for (pattern_part, command_part) in pattern_parts.iter().zip(command_parts.iter()) {
            if pattern_part.starts_with('{') && pattern_part.ends_with('}') {
                // This is a placeholder - accept any value for now
                continue;
            } else if pattern_part != command_part {
                return false;
            }
        }

        true
    }

    /// Get completions for a partial command
    pub fn get_completions(&self, partial: &str) -> Vec<String> {
        let mut completions = Vec::new();

        if partial.is_empty() || self.base.starts_with(partial) {
            completions.push(self.base.clone());
        }

        // If partial starts with the base command, suggest the full pattern
        if partial.starts_with(&self.base) {
            let pattern_without_placeholders = self.pattern
                .replace("{config_file}", "")
                .replace("{wallet_name}", "")
                .replace("{hpub}", "")
                .replace("{pubkey}", "")
                .replace("{process}", "");

            if pattern_without_placeholders.starts_with(partial) || partial.starts_with(&pattern_without_placeholders.trim()) {
                completions.push(self.pattern.clone());
            }
        }

        completions
    }
}

/// Manages the whitelisted command system
pub struct CommandSystem {
    commands: Vec<CommandTemplate>,
    navigation_commands: Vec<String>,
}

impl CommandSystem {
    pub fn new() -> Self {
        let mut system = Self {
            commands: Vec::new(),
            navigation_commands: vec!["help".to_string(), "next".to_string(), "back".to_string()],
        };

        // Pool Operator commands
        system.add_command(CommandTemplate::new(
            "pool_sv2",
            "pool_sv2 --config pool-config-ehash.toml",
            "Start the Pool with eHash minting enabled",
        ));

        // Translator/Proxy commands
        system.add_command(CommandTemplate::new(
            "translator_sv2",
            "translator_sv2 --config tproxy-config-ehash.toml",
            "Start the Translation Proxy with eHash support",
        ));

        // Mining device commands
        system.add_command(CommandTemplate::new(
            "mining_device",
            "mining_device --pool-address 127.0.0.1:34255 --user-identity {hpub}",
            "Start mining device with eHash pubkey",
        ).with_placeholder("hpub", vec!["hpub1...".to_string()]));

        // CDK wallet commands
        system.add_command(CommandTemplate::new(
            "cdk-cli",
            "cdk-cli wallet create --name {wallet_name} --mint-url http://127.0.0.1:3338",
            "Create a new Cashu wallet",
        ).with_placeholder("wallet_name", vec!["proxy-wallet".to_string(), "pioneer-wallet".to_string()]));

        system.add_command(CommandTemplate::new(
            "cdk-cli",
            "cdk-cli wallet info {wallet_name}",
            "Display wallet information and derive hpub",
        ).with_placeholder("wallet_name", vec!["proxy-wallet".to_string(), "pioneer-wallet".to_string()]));

        system.add_command(CommandTemplate::new(
            "cdk-cli",
            "cdk-cli wallet balance {wallet_name}",
            "Check wallet balance",
        ).with_placeholder("wallet_name", vec!["proxy-wallet".to_string(), "pioneer-wallet".to_string()]));

        // Monitoring commands
        system.add_command(CommandTemplate::new(
            "ps",
            "ps aux | grep -E '(pool_sv2|translator_sv2|mining_device)'",
            "Check if processes are running",
        ));

        system.add_command(CommandTemplate::new(
            "tail",
            "tail -f logs/{process}.log",
            "View process logs in real-time",
        ).with_placeholder("process", vec!["pool".to_string(), "tproxy".to_string(), "miner".to_string()]));

        system.add_command(CommandTemplate::new(
            "curl",
            "curl http://127.0.0.1:3338/v1/info",
            "Query mint information",
        ));

        system.add_command(CommandTemplate::new(
            "curl",
            "curl http://127.0.0.1:3338/v1/mint/quotes/pubkey/{pubkey}",
            "Query mint quotes for a pubkey",
        ).with_placeholder("pubkey", vec!["<hpub>".to_string()]));

        system
    }

    fn add_command(&mut self, template: CommandTemplate) {
        self.commands.push(template);
    }

    /// Validate if a command is allowed
    pub fn validate_command(&self, command: &str) -> Result<()> {
        let trimmed = command.trim();

        // Check navigation commands
        if self.navigation_commands.contains(&trimmed.to_string()) {
            return Ok(());
        }

        // Check against command templates
        for template in &self.commands {
            if template.matches(trimmed) {
                return Ok(());
            }
        }

        Err(anyhow!(
            "Command not available in tutorial. Try 'help' to see available commands."
        ))
    }

    /// Get command completions for a partial input
    pub fn get_completions(&self, partial: &str) -> Vec<String> {
        let mut completions = Vec::new();

        // Navigation commands
        for nav_cmd in &self.navigation_commands {
            if nav_cmd.starts_with(partial) {
                completions.push(nav_cmd.clone());
            }
        }

        // Command templates
        for template in &self.commands {
            completions.extend(template.get_completions(partial));
        }

        completions.sort();
        completions.dedup();
        completions
    }

    /// Get available commands for help display
    pub fn get_available_commands(&self, context: &str) -> Vec<String> {
        let mut commands = Vec::new();

        match context {
            "Welcome" => {
                commands.push("Navigation: help, next".to_string());
            }
            "PoolOperator" => {
                commands.push("Pool Commands:".to_string());
                commands.push("  pool_sv2 --config pool-config-ehash.toml".to_string());
                commands.push("  ps aux | grep pool_sv2".to_string());
                commands.push("  tail -f logs/pool.log".to_string());
                commands.push("Navigation: help, next, back".to_string());
            }
            "ProxyOperator" => {
                commands.push("Wallet & Proxy Commands:".to_string());
                commands.push("  cdk-cli wallet create --name proxy-wallet --mint-url http://127.0.0.1:3338".to_string());
                commands.push("  cdk-cli wallet info proxy-wallet".to_string());
                commands.push("  translator_sv2 --config tproxy-config-ehash.toml".to_string());
                commands.push("  ps aux | grep translator_sv2".to_string());
                commands.push("Navigation: help, next, back".to_string());
            }
            "Pioneer" => {
                commands.push("Mining Commands:".to_string());
                commands.push("  cdk-cli wallet create --name pioneer-wallet --mint-url http://127.0.0.1:3338".to_string());
                commands.push("  cdk-cli wallet info pioneer-wallet".to_string());
                commands.push("  mining_device --pool-address 127.0.0.1:34255 --user-identity <hpub>".to_string());
                commands.push("  cdk-cli wallet balance pioneer-wallet".to_string());
                commands.push("  curl http://127.0.0.1:3338/v1/info".to_string());
                commands.push("Navigation: help, next, back".to_string());
            }
            "Complete" => {
                commands.push("Tutorial complete! You can exit or review previous chapters.".to_string());
                commands.push("Navigation: help, back".to_string());
            }
            _ => {
                commands.push("Navigation: help, next, back".to_string());
            }
        }

        commands
    }

    /// Check if a command is a navigation command
    pub fn is_navigation_command(&self, command: &str) -> bool {
        self.navigation_commands.contains(&command.trim().to_string())
    }
}

impl Default for CommandSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_validation() {
        let system = CommandSystem::new();

        // Valid commands
        assert!(system.validate_command("help").is_ok());
        assert!(system.validate_command("next").is_ok());
        assert!(system.validate_command("back").is_ok());
        assert!(system.validate_command("pool_sv2 --config pool-config-ehash.toml").is_ok());

        // Invalid commands
        assert!(system.validate_command("rm -rf /").is_err());
        assert!(system.validate_command("sudo something").is_err());
        assert!(system.validate_command("random_command").is_err());
    }

    #[test]
    fn test_completions() {
        let system = CommandSystem::new();

        let completions = system.get_completions("hel");
        assert!(completions.contains(&"help".to_string()));

        let completions = system.get_completions("pool");
        assert!(completions.iter().any(|c| c.starts_with("pool_sv2")));
    }
}
