use anyhow::{Context, Result, bail};
use std::net::{IpAddr, ToSocketAddrs};

pub fn validate_domain(value: &str) -> Result<()> {
    if !value.contains('.')
        || value.starts_with('.')
        || value.ends_with('.')
        || value
            .bytes()
            .any(|b| !(b.is_ascii_alphanumeric() || b == b'.' || b == b'-'))
    {
        bail!("invalid fully-qualified domain: {value}");
    }
    Ok(())
}

pub fn resolve_domain(value: &str) -> Result<Vec<IpAddr>> {
    validate_domain(value)?;
    let mut addresses = (value, 443)
        .to_socket_addrs()
        .with_context(|| {
            format!(
                "DNS lookup failed for {value}; configure a public A, AAAA, or CNAME record before continuing"
            )
        })?
        .map(|address| address.ip())
        .collect::<Vec<_>>();
    addresses.sort_unstable();
    addresses.dedup();
    if addresses.is_empty() {
        bail!(
            "DNS lookup returned no addresses for {value}; configure a public A, AAAA, or CNAME record before continuing"
        );
    }
    Ok(addresses)
}

pub fn require_domain_resolution(value: &str) -> Result<()> {
    resolve_domain(value).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_validation_accepts_normal_fqdns() {
        for domain in [
            "argus.example.com",
            "content.argus.example.com",
            "a-b.example.co.uk",
        ] {
            validate_domain(domain).unwrap();
        }
    }

    #[test]
    fn domain_validation_rejects_invalid_names() {
        for domain in [
            "localhost",
            ".example.com",
            "example.com.",
            "https://example.com",
            "example com",
        ] {
            assert!(validate_domain(domain).is_err(), "{domain}");
        }
    }
}
