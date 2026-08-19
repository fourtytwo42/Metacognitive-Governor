use std::{env, fs};

use base64::Engine;
use minisign_verify::{PublicKey, Signature};

fn decode_tauri_envelope(path: &std::ffi::OsStr) -> Result<String, Box<dyn std::error::Error>> {
    let encoded = fs::read_to_string(path)?;
    let decoded = base64::engine::general_purpose::STANDARD.decode(encoded.trim())?;
    Ok(String::from_utf8(decoded)?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os().skip(1);
    let public_key_path = arguments.next().ok_or("missing minisign public-key path")?;
    let artifact_path = arguments.next().ok_or("missing signed artifact path")?;
    let signature_path = arguments.next().ok_or("missing signature path")?;
    if arguments.next().is_some() {
        return Err("unexpected updater signature verifier argument".into());
    }

    let public_key = PublicKey::decode(&decode_tauri_envelope(&public_key_path)?)?;
    let signature = Signature::decode(&decode_tauri_envelope(&signature_path)?)?;
    let mut artifact = fs::read(&artifact_path)?;
    if artifact.is_empty() {
        return Err("signed updater fixture is empty".into());
    }

    public_key.verify(&artifact, &signature, false)?;
    artifact[0] ^= 1;
    if public_key.verify(&artifact, &signature, false).is_ok() {
        return Err("tampered updater artifact unexpectedly verified".into());
    }

    println!("Updater signature verification and tamper rejection passed.");
    Ok(())
}
