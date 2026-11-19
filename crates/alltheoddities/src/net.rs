//! Read a page or a file from the network.

use anyhow::{Result, bail};

/// Reads from HTTPS. The tests use a fake.
pub trait Http {
    fn get(&self, url: &str) -> Result<String>;
    fn get_bytes(&self, url: &str) -> Result<Vec<u8>>;
}

/// The client of a build with no network. A later step replaces it.
pub struct NoHttp;

impl Http for NoHttp {
    fn get(&self, _url: &str) -> Result<String> {
        bail!("this build has no network")
    }
    fn get_bytes(&self, _url: &str) -> Result<Vec<u8>> {
        bail!("this build has no network")
    }
}
