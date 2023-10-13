use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use base64::display::Base64Display;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, Mac};
use md5::Md5;
use rand::RngCore;
use sha2::digest::FixedOutput;
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PostgresPassword {
    /// The plaintext password is given and stored as is with no interpretation.
    Plain(String),
    /// A plaintext or MD5 password is given. If the password is not prefixed with `md5`, then
    /// it is reencoded as md5.
    Md5(String),
    /// A plaintext or SCRAM-SHA-256 password is given. If the password is not prefixed with `SCRAM-SHA-256$`, then
    /// it is reencoded as SCRAM-SHA-256.
    #[serde(rename = "scram-sha-256")]
    ScramSha256(String),
}

impl PostgresPassword {
    pub fn get_password_text(&self, username: &str) -> String {
        match self {
            PostgresPassword::Plain(s) => s.clone(),
            PostgresPassword::Md5(s) if s.starts_with("md5") => s.clone(),
            PostgresPassword::Md5(s)  => md5(s.as_bytes(), username),
            PostgresPassword::ScramSha256(s) if s.starts_with("SCRAM-SHA-256$") => s.clone(),
            PostgresPassword::ScramSha256(s) => scram_sha_256(s.as_bytes()),
        }
    }

    pub fn get_raw_text(&self) -> &str {
        match self {
            PostgresPassword::Plain(v) => v,
            PostgresPassword::Md5(v) => v,
            PostgresPassword::ScramSha256(v) => v,
        }
    }
}


const SCRAM_DEFAULT_ITERATIONS: u32 = 4096;
const SCRAM_DEFAULT_SALT_LEN: usize = 16;

/// Hash password using SCRAM-SHA-256 with a randomly-generated
/// salt.
///
/// The client may assume the returned string doesn't contain any
/// special characters that would require escaping in an SQL command.
fn scram_sha_256(password: &[u8]) -> String {
    let mut salt: [u8; SCRAM_DEFAULT_SALT_LEN] = [0; SCRAM_DEFAULT_SALT_LEN];
    let mut rng = rand::thread_rng();
    rng.fill_bytes(&mut salt);
    scram_sha_256_salt(password, salt)
}

// Internal implementation of scram_sha_256 with a caller-provided
// salt. This is useful for testing.
fn scram_sha_256_salt(password: &[u8], salt: [u8; SCRAM_DEFAULT_SALT_LEN]) -> String {
    // Prepare the password, per [RFC
    // 4013](https://tools.ietf.org/html/rfc4013), if possible.
    //
    // Postgres treats passwords as byte strings (without embedded NUL
    // bytes), but SASL expects passwords to be valid UTF-8.
    //
    // Follow the behavior of libpq's PQencryptPasswordConn(), and
    // also the backend. If the password is not valid UTF-8, or if it
    // contains prohibited characters (such as non-ASCII whitespace),
    // just skip the SASLprep step and use the original byte
    // sequence.
    let prepared: Vec<u8> = match std::str::from_utf8(password) {
        Ok(password_str) => {
            match stringprep::saslprep(password_str) {
                Ok(p) => p.into_owned().into_bytes(),
                // contains invalid characters; skip saslprep
                Err(_) => Vec::from(password),
            }
        }
        // not valid UTF-8; skip saslprep
        Err(_) => Vec::from(password),
    };

    // salt password
    let salted_password = hi(&prepared, &salt, SCRAM_DEFAULT_ITERATIONS);

    // client key
    let mut hmac = Hmac::<Sha256>::new_from_slice(&salted_password)
        .expect("HMAC is able to accept all key sizes");
    hmac.update(b"Client Key");
    let client_key = hmac.finalize().into_bytes();

    // stored key
    let mut hash = Sha256::default();
    hash.update(client_key.as_slice());
    let stored_key = hash.finalize_fixed();

    // server key
    let mut hmac = Hmac::<Sha256>::new_from_slice(&salted_password)
        .expect("HMAC is able to accept all key sizes");
    hmac.update(b"Server Key");
    let server_key = hmac.finalize().into_bytes();

    format!(
        "SCRAM-SHA-256${}:{}${}:{}",
        SCRAM_DEFAULT_ITERATIONS,
        Base64Display::new(&salt, &STANDARD),
        Base64Display::new(&stored_key, &STANDARD),
        Base64Display::new(&server_key, &STANDARD)
    )
}

fn hi(str: &[u8], salt: &[u8], i: u32) -> [u8; 32] {
    let mut hmac =
        Hmac::<Sha256>::new_from_slice(str).expect("HMAC is able to accept all key sizes");
    hmac.update(salt);
    hmac.update(&[0, 0, 0, 1]);
    let mut prev = hmac.finalize().into_bytes();

    let mut hi = prev;

    for _ in 1..i {
        let mut hmac = Hmac::<Sha256>::new_from_slice(str).expect("already checked above");
        hmac.update(&prev);
        prev = hmac.finalize().into_bytes();

        for (hi, prev) in hi.iter_mut().zip(prev) {
            *hi ^= prev;
        }
    }

    hi.into()
}

/// **Not recommended, as MD5 is not considered to be secure.**
///
/// Hash password using MD5 with the username as the salt.
///
/// The client may assume the returned string doesn't contain any
/// special characters that would require escaping.
fn md5(password: &[u8], username: &str) -> String {
    // salt password with username
    let mut salted_password = Vec::from(password);
    salted_password.extend_from_slice(username.as_bytes());

    let mut hash = Md5::new();
    hash.update(&salted_password);
    let digest = hash.finalize();
    format!("md5{:x}", digest)
}

