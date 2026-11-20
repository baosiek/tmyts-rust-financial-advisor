use std::fmt;

use serde::Deserialize;
use zeroize::Zeroize;

#[derive(Clone, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    // the builder
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// only way to read the value.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)") // never prints the key, even in {:?} or logs
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize(); // overwrites the memory when the secret is freed
    }
}
