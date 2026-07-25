use std::io::Read;
use std::time::Duration;

use anyhow::{Context, Result};

/// warframe.com rejects non-browser agents on some endpoints.
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";

/// A blocking HTTP agent with a global per-call timeout.
pub fn agent() -> ureq::Agent {
    agent_as(USER_AGENT)
}

/// An agent that identifies itself as `user_agent`. The agent carries the header so exactly
/// one goes out: a second, per-request `User-Agent` reads as malformed to Cloudflare, which
/// answers 403.
pub fn agent_as(user_agent: &str) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(300)))
        .user_agent(user_agent)
        .build();
    ureq::Agent::new_with_config(config)
}

/// GET a URL and read the full body.
pub fn get(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>> {
    get_with(agent, url, &[])
}

/// GET a URL with extra request headers.
pub fn get_with(agent: &ureq::Agent, url: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>> {
    let mut req = agent.get(url);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let mut res = req.call().with_context(|| format!("GET {url}"))?;
    let mut buf = Vec::new();
    res.body_mut().as_reader().read_to_end(&mut buf)?;
    Ok(buf)
}
