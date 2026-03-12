use anyhow::{Result, bail};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerSelector {
    Port {
        raw: String,
        port: u16,
    },
    AddressPort {
        raw: String,
        address: String,
        port: u16,
    },
    Address {
        raw: String,
        address: String,
    },
}

impl RunnerSelector {
    pub fn parse(input: &str) -> Result<Self> {
        if input.is_empty() {
            bail!("runner selector cannot be empty");
        }

        if input.chars().all(|ch| ch.is_ascii_digit()) {
            let port = input
                .parse::<u16>()
                .map_err(|_| anyhow::anyhow!("invalid port selector '{}'", input))?;
            return Ok(Self::Port {
                raw: input.to_owned(),
                port,
            });
        }

        let colon_count = input.chars().filter(|ch| *ch == ':').count();
        if colon_count == 1 {
            let (address, port) = input
                .split_once(':')
                .ok_or_else(|| anyhow::anyhow!("invalid runner selector '{}'", input))?;
            if address.is_empty() {
                bail!("invalid runner selector '{}'", input);
            }
            let port = port
                .parse::<u16>()
                .map_err(|_| anyhow::anyhow!("invalid port in runner selector '{}'", input))?;
            return Ok(Self::AddressPort {
                raw: input.to_owned(),
                address: address.to_owned(),
                port,
            });
        }

        if colon_count > 1 || input.contains('/') || input.contains("://") {
            bail!("invalid runner selector '{}'", input);
        }

        Ok(Self::Address {
            raw: input.to_owned(),
            address: input.to_owned(),
        })
    }

    pub fn matches(&self, address: &str, port: u16) -> bool {
        match self {
            Self::Port {
                port: selector_port,
                ..
            } => *selector_port == port,
            Self::AddressPort {
                address: selector_address,
                port: selector_port,
                ..
            } => selector_address == address && *selector_port == port,
            Self::Address {
                address: selector_address,
                ..
            } => selector_address == address,
        }
    }

    pub fn raw(&self) -> &str {
        match self {
            Self::Port { raw, .. } | Self::AddressPort { raw, .. } | Self::Address { raw, .. } => {
                raw
            }
        }
    }
}

pub fn normalize_attach_runner(value: &str) -> Result<String> {
    match RunnerSelector::parse(value)? {
        RunnerSelector::Port { port, .. } => Ok(format!("http://127.0.0.1:{port}")),
        RunnerSelector::AddressPort { address, port, .. } => Ok(format!("http://{address}:{port}")),
        RunnerSelector::Address { address, .. } => Ok(format!("http://{address}:55880")),
    }
}

pub fn parse_stack_name(value: &str) -> Result<String> {
    if value.is_empty() {
        bail!("context name cannot be empty");
    }
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        bail!("context name cannot be empty");
    };
    if !first.is_ascii_alphanumeric() {
        bail!("invalid context name '{}'", value);
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-')) {
        bail!("invalid context name '{}'", value);
    }
    Ok(value.to_owned())
}
