use ciborium::{from_reader, into_writer};
use ed25519_dalek::Signer;
use k256::{
    ecdsa,
    ecdsa::signature::hazmat::{PrehashSigner, PrehashVerifier},
};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use sha3::{Digest, Sha3_256};

use crate::unix_ms;

const PERMITTED_DRIFT: u64 = 10; // seconds

// Token format: [expire_at in seconds, agent, signature]
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct Token(pub u64, pub String, pub ByteBuf);

pub fn ed25519_sign(key: &ed25519_dalek::SigningKey, expire_at: u64, agent: String) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    into_writer(&(expire_at, &agent), &mut buf).expect("failed to encode data in CBOR format");

    let sig = key.sign(&buf).to_bytes();
    buf.clear();
    into_writer(&(expire_at, agent, ByteBuf::from(sig)), &mut buf)
        .expect("failed to encode in CBOR format");
    buf
}

pub fn ed25519_verify(keys: &[ed25519_dalek::VerifyingKey], data: &[u8]) -> Result<Token, String> {
    let token: Token = from_reader(data).map_err(|_err| "failed to decode CBOR data")?;
    if token.0 + PERMITTED_DRIFT < unix_ms() / 1000 {
        return Err("token expired".to_string());
    }
    let sig = ed25519_dalek::Signature::from_slice(token.2.as_slice())
        .map_err(|_err| "failed to parse Ed25519 signature")?;
    let mut buf: Vec<u8> = Vec::new();
    into_writer(&(token.0, &token.1), &mut buf).expect("failed to encode data in CBOR format");
    for key in keys.iter() {
        if key.verify_strict(&buf, &sig).is_ok() {
            return Ok(token);
        }
    }

    Err("failed to verify Ed25519 signature".to_string())
}

// Secp256k1
pub fn ecdsa_sign(key: &ecdsa::SigningKey, expire_at: u64, agent: String) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    into_writer(&(expire_at, &agent), &mut buf).expect("failed to encode data in CBOR format");
    let digest = sha3_256(&buf);
    let sig: ecdsa::Signature = key
        .sign_prehash(&digest)
        .expect("failed to sign Secp256k1 signature");
    buf.clear();
    into_writer(&(expire_at, agent, ByteBuf::from(sig.to_vec())), &mut buf)
        .expect("failed to encode in CBOR format");
    buf
}

// Secp256k1
pub fn ecdsa_verify(keys: &[ecdsa::VerifyingKey], data: &[u8]) -> Result<Token, String> {
    let token: Token = from_reader(data).map_err(|_err| "failed to decode CBOR data")?;
    if token.0 + PERMITTED_DRIFT < unix_ms() / 1000 {
        return Err("token expired".to_string());
    }
    let sig = ecdsa::Signature::try_from(token.2.as_slice())
        .map_err(|_err| "failed to parse Secp256k1 signature")?;
    let mut buf: Vec<u8> = Vec::new();
    into_writer(&(token.0, &token.1), &mut buf).expect("failed to encode data in CBOR format");
    let digest = sha3_256(&buf);

    for key in keys.iter() {
        if key.verify_prehash(digest.as_slice(), &sig).is_ok() {
            return Ok(token);
        }
    }

    Err("failed to verify ECDSA/Secp256k1 signature".to_string())
}

pub fn sha3_256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    hasher.finalize().into()
}

#[cfg(test)]
mod test {
    use super::*;
    use base64::{engine::general_purpose, Engine};
    use k256::PublicKey;
    use rand_core::{OsRng, RngCore};

    #[test]
    fn test_ed25519_token() {
        let mut secret_key = [0u8; 32];
        OsRng.fill_bytes(&mut secret_key);
        let signing_key: ed25519_dalek::SigningKey =
            ed25519_dalek::SigningKey::from_bytes(&secret_key);
        let agent = "alice".to_string();
        let expire_at = unix_ms() / 1000 + 3600;
        let signed = super::ed25519_sign(&signing_key, expire_at, agent.clone());
        let token = super::ed25519_verify(&[signing_key.verifying_key()], &signed).unwrap();
        assert_eq!(token.0, expire_at);
        assert_eq!(token.1, agent);
    }

    #[test]
    #[ignore]
    fn test_secp256k1_token() {
        let signing_key = ecdsa::SigningKey::random(&mut OsRng);
        let agent = "alice".to_string();
        let expire_at = unix_ms() / 1000 + 3600;
        let signed = super::ecdsa_sign(&signing_key, expire_at, agent.clone());
        let token =
            super::ecdsa_verify(&[ecdsa::VerifyingKey::from(&signing_key)], &signed).unwrap();
        assert_eq!(token.0, expire_at);
        assert_eq!(token.1, agent);

        println!(
            "token: {:?}",
            general_purpose::URL_SAFE_NO_PAD.encode(&signed)
        );

        let pk: PublicKey = signing_key.verifying_key().into();
        let pk = pk.to_sec1_bytes();
        let pk = general_purpose::URL_SAFE_NO_PAD.encode(&pk);
        println!("{:?}", pk);

        // let pk = general_purpose::URL_SAFE_NO_PAD
        //     .decode("A44DZpzDwDvq9HwW3_dynOfDgkMJHKgOxUyCOrv5Pl3O")
        //     .expect("invalid base64");
        // let pk = ecdsa::VerifyingKey::from_sec1_bytes(&pk).expect("invalid ecdsa public key");
        // let data = general_purpose::URL_SAFE_NO_PAD
        //     .decode("gxpmZDmJaklDUGFuZGFEQU9YQMQr36UI8JV2jJEM_PMe96GsgymHzjfsbZyAsFSHF0FUNsuj6LKsqHg2dzYG9RoxQRtrcGsphYsNiQJwG3g9Ju4")
        //     .expect("invalid base64");
        // let token = super::ecdsa_verify(&[pk], &data).unwrap();
        println!("{:?}", token);
        // Token(1717844361, "ICPandaDAO", [196, 43, 223, ... 61, 38, 238])
    }
}
