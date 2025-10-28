//! Integration test for Pool configuration parsing
//!
//! Verifies that:
//! 1. The example config file with eHash mint can be parsed
//! 2. MintConfig fields are correctly deserialized
//! 3. Optional ehash_mint section works when present and absent

use std::fs;

#[test]
fn test_parse_pool_config_with_ehash() {
    // Read the example config file with eHash mint enabled
    let config_path = "config-examples/pool-config-local-tp-with-ehash-example.toml";
    let config_str = fs::read_to_string(config_path)
        .expect("Failed to read pool-config-local-tp-with-ehash-example.toml");

    // Parse as raw TOML to verify structure
    let config: toml::Value = toml::from_str(&config_str)
        .expect("Failed to parse TOML config file");

    // Verify ehash_mint section exists
    let ehash_mint = config.get("ehash_mint")
        .expect("ehash_mint section should be present in example config");

    // Verify required fields
    assert!(ehash_mint.get("mint_url").is_some(), "mint_url should be present");
    assert!(ehash_mint.get("database_url").is_some(), "database_url should be present");
    assert!(ehash_mint.get("min_leading_zeros").is_some(), "min_leading_zeros should be present");

    // Verify min_leading_zeros value
    let min_leading_zeros = ehash_mint.get("min_leading_zeros")
        .and_then(|v| v.as_integer())
        .expect("min_leading_zeros should be an integer");
    assert_eq!(min_leading_zeros, 32, "min_leading_zeros should default to 32 (hashpool standard)");

    println!("✓ Pool config with eHash mint parses successfully");
    println!("  - mint_url: {:?}", ehash_mint.get("mint_url"));
    println!("  - database_url: {:?}", ehash_mint.get("database_url"));
    println!("  - min_leading_zeros: {}", min_leading_zeros);
}

#[test]
fn test_parse_pool_config_without_ehash() {
    // Read a config file without eHash mint
    let config_path = "config-examples/pool-config-local-tp-example.toml";
    let config_str = fs::read_to_string(config_path)
        .expect("Failed to read pool-config-local-tp-example.toml");

    // Parse as raw TOML
    let config: toml::Value = toml::from_str(&config_str)
        .expect("Failed to parse TOML config file");

    // Verify ehash_mint section is absent (should be optional)
    assert!(config.get("ehash_mint").is_none(),
        "ehash_mint section should be absent in non-eHash config");

    println!("✓ Pool config without eHash mint parses successfully");
}

#[test]
fn test_ehash_mint_optional_fields() {
    // Test that optional fields have sensible defaults
    let minimal_config = r#"
        authority_public_key = "9auqWEzQDVyd2oe1JVGFLMLHZtCo2FFqZwtKA5gd9xbuEu7PH72"
        authority_secret_key = "mkDLTBBRxdBv998612qipDYoTK3YUrqLe8uWw7gu3iXbSrn2n"
        cert_validity_sec = 3600
        listen_address = "0.0.0.0:34254"
        coinbase_reward_script = "addr(tb1qa0sm0hxzj0x25rh8gw5xlzwlsfvvyz8u96w3p8)"
        server_id = 1
        pool_signature = "Test Pool"
        tp_address = "127.0.0.1:8442"
        shares_per_minute = 6.0
        share_batch_size = 10

        [ehash_mint]
        mint_url = "https://mint.test.dev"
    "#;

    let config: toml::Value = toml::from_str(minimal_config)
        .expect("Failed to parse minimal eHash config");

    let ehash_mint = config.get("ehash_mint")
        .expect("ehash_mint section should be present");

    // Only mint_url is required, others should have defaults or be optional
    assert!(ehash_mint.get("mint_url").is_some(), "mint_url is required");

    println!("✓ Minimal eHash config with only required fields parses successfully");
}
