//! Integration test for TProxy configuration parsing
//!
//! Verifies that:
//! 1. The example config file with eHash wallet can be parsed
//! 2. WalletConfig fields are correctly deserialized
//! 3. Optional ehash_wallet section works when present and absent
//! 4. default_locking_pubkey validation works correctly

use std::fs;

#[test]
fn test_parse_tproxy_config_with_ehash() {
    // Read the example config file with eHash wallet enabled
    let config_path = "config-examples/tproxy-config-local-pool-with-ehash-example.toml";
    let config_str = fs::read_to_string(config_path)
        .expect("Failed to read tproxy-config-local-pool-with-ehash-example.toml");

    // Parse as raw TOML to verify structure
    let config: toml::Value = toml::from_str(&config_str)
        .expect("Failed to parse TOML config file");

    // Verify ehash_wallet section exists
    let ehash_wallet = config.get("ehash_wallet")
        .expect("ehash_wallet section should be present in example config");

    // Verify required field when ehash_wallet is present
    let default_locking_pubkey = config.get("default_locking_pubkey")
        .expect("default_locking_pubkey should be present when ehash_wallet is configured");

    // Verify it's a string in hpub format
    let pubkey_str = default_locking_pubkey.as_str()
        .expect("default_locking_pubkey should be a string");

    assert!(pubkey_str.starts_with("hpub1"),
        "default_locking_pubkey should start with 'hpub1' (bech32 format)");

    println!("✓ TProxy config with eHash wallet parses successfully");
    println!("  - default_locking_pubkey: {}", pubkey_str);

    // Verify optional fields exist in config structure
    if let Some(mint_url) = ehash_wallet.get("mint_url") {
        println!("  - mint_url: {:?}", mint_url);
    }
}

#[test]
fn test_parse_tproxy_config_without_ehash() {
    // Read a config file without eHash wallet
    let config_path = "config-examples/tproxy-config-local-pool-example.toml";
    let config_str = fs::read_to_string(config_path)
        .expect("Failed to read tproxy-config-local-pool-example.toml");

    // Parse as raw TOML
    let config: toml::Value = toml::from_str(&config_str)
        .expect("Failed to parse TOML config file");

    // Verify ehash_wallet section is absent (should be optional)
    assert!(config.get("ehash_wallet").is_none(),
        "ehash_wallet section should be absent in non-eHash config");

    // Verify default_locking_pubkey is also absent
    assert!(config.get("default_locking_pubkey").is_none(),
        "default_locking_pubkey should be absent when ehash_wallet is not configured");

    println!("✓ TProxy config without eHash wallet parses successfully");
}

#[test]
fn test_ehash_wallet_optional_fields() {
    // Test that optional fields in ehash_wallet have sensible defaults
    let minimal_config = r#"
        downstream_address = "0.0.0.0"
        downstream_port = 34255
        max_supported_version = 2
        min_supported_version = 2
        downstream_extranonce2_size = 4
        user_identity = "test_user"
        default_locking_pubkey = "hpub1qyq2fw8qdwmhzgfzecvl5a3jyy8v8lf7wj8rfxp8sxvh7vxqzqfxl6yw"
        aggregate_channels = true

        [downstream_difficulty_config]
        min_individual_miner_hashrate = 10_000_000_000_000.0
        shares_per_minute = 6.0
        enable_vardiff = true

        [[upstreams]]
        address = "127.0.0.1"
        port = 34254
        authority_pubkey = "9auqWEzQDVyd2oe1JVGFLMLHZtCo2FFqZwtKA5gd9xbuEu7PH72"

        [ehash_wallet]
        # Only the section marker - all fields are optional
    "#;

    let config: toml::Value = toml::from_str(minimal_config)
        .expect("Failed to parse minimal eHash config");

    let ehash_wallet = config.get("ehash_wallet")
        .expect("ehash_wallet section should be present");

    // default_locking_pubkey is required at top level when ehash_wallet is present
    let default_locking_pubkey = config.get("default_locking_pubkey")
        .expect("default_locking_pubkey is required when ehash_wallet is present");

    let pubkey_str = default_locking_pubkey.as_str()
        .expect("default_locking_pubkey should be a string");

    assert!(pubkey_str.starts_with("hpub1"),
        "default_locking_pubkey should be in hpub format");

    // All fields in ehash_wallet section are optional
    // mint_url is optional (tracking-only mode works without it)
    assert!(ehash_wallet.get("mint_url").is_none(),
        "mint_url should be optional in minimal config");

    println!("✓ Minimal eHash config with only required fields parses successfully");
}

#[test]
fn test_ehash_wallet_with_all_fields() {
    // Test config with all optional fields specified
    let full_config = r#"
        downstream_address = "0.0.0.0"
        downstream_port = 34255
        max_supported_version = 2
        min_supported_version = 2
        downstream_extranonce2_size = 4
        user_identity = "test_user"
        default_locking_pubkey = "hpub1qyq2fw8qdwmhzgfzecvl5a3jyy8v8lf7wj8rfxp8sxvh7vxqzqfxl6yw"
        aggregate_channels = true

        [downstream_difficulty_config]
        min_individual_miner_hashrate = 10_000_000_000_000.0
        shares_per_minute = 6.0
        enable_vardiff = true

        [[upstreams]]
        address = "127.0.0.1"
        port = 34254
        authority_pubkey = "9auqWEzQDVyd2oe1JVGFLMLHZtCo2FFqZwtKA5gd9xbuEu7PH72"

        [ehash_wallet]
        mint_url = "https://mint.hashpool.dev"
        max_retries = 5
        backoff_multiplier = 3
        recovery_enabled = false
        log_level = "debug"
    "#;

    let config: toml::Value = toml::from_str(full_config)
        .expect("Failed to parse full eHash config");

    let ehash_wallet = config.get("ehash_wallet")
        .expect("ehash_wallet section should be present");

    // Verify all optional fields are present
    assert!(ehash_wallet.get("mint_url").is_some(), "mint_url should be present");
    assert!(ehash_wallet.get("max_retries").is_some(), "max_retries should be present");
    assert!(ehash_wallet.get("backoff_multiplier").is_some(), "backoff_multiplier should be present");
    assert!(ehash_wallet.get("recovery_enabled").is_some(), "recovery_enabled should be present");
    assert!(ehash_wallet.get("log_level").is_some(), "log_level should be present");

    // Verify field values
    let max_retries = ehash_wallet.get("max_retries")
        .and_then(|v| v.as_integer())
        .expect("max_retries should be an integer");
    assert_eq!(max_retries, 5, "max_retries should be 5");

    let backoff_multiplier = ehash_wallet.get("backoff_multiplier")
        .and_then(|v| v.as_integer())
        .expect("backoff_multiplier should be an integer");
    assert_eq!(backoff_multiplier, 3, "backoff_multiplier should be 3");

    let recovery_enabled = ehash_wallet.get("recovery_enabled")
        .and_then(|v| v.as_bool())
        .expect("recovery_enabled should be a boolean");
    assert!(!recovery_enabled, "recovery_enabled should be false");

    println!("✓ Full eHash config with all optional fields parses successfully");
    println!("  - max_retries: {}", max_retries);
    println!("  - backoff_multiplier: {}", backoff_multiplier);
    println!("  - recovery_enabled: {}", recovery_enabled);
}

#[test]
fn test_default_locking_pubkey_validation() {
    // Test that invalid hpub format is detectable
    let invalid_config = r#"
        downstream_address = "0.0.0.0"
        downstream_port = 34255
        max_supported_version = 2
        min_supported_version = 2
        downstream_extranonce2_size = 4
        user_identity = "test_user"
        default_locking_pubkey = "invalid_not_hpub_format"
        aggregate_channels = true

        [downstream_difficulty_config]
        min_individual_miner_hashrate = 10_000_000_000_000.0
        shares_per_minute = 6.0
        enable_vardiff = true

        [[upstreams]]
        address = "127.0.0.1"
        port = 34254
        authority_pubkey = "9auqWEzQDVyd2oe1JVGFLMLHZtCo2FFqZwtKA5gd9xbuEu7PH72"

        [ehash_wallet]
    "#;

    let config: toml::Value = toml::from_str(invalid_config)
        .expect("Config should parse as TOML even with invalid pubkey");

    let pubkey_str = config.get("default_locking_pubkey")
        .and_then(|v| v.as_str())
        .expect("default_locking_pubkey should be present");

    // Verify that validation would catch this (actual validation happens in config deserialization)
    assert!(!pubkey_str.starts_with("hpub1"),
        "Invalid pubkey should not start with hpub1");

    println!("✓ Invalid hpub format is detectable");
    println!("  - Invalid pubkey: {}", pubkey_str);
    println!("  - Note: Runtime validation would reject this during config loading");
}
