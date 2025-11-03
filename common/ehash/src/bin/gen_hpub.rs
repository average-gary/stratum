use bitcoin::secp256k1::{Secp256k1, SecretKey, PublicKey};
use ehash_integration::hpub::encode_hpub;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&[1u8; 32])?;
    let pubkey = PublicKey::from_secret_key(&secp, &secret_key);

    let hpub = encode_hpub(&pubkey)?;
    println!("{}", hpub);
    eprintln!("Private key (hex): {}", secret_key.display_secret());

    Ok(())
}
