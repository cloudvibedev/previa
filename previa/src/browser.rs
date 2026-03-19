use std::env;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::process::Command;

use anyhow::{Context, Result, bail};
use urlencoding::encode;

const APP_URL: &str = "https://ide.previa.dev";

pub fn build_open_url(address: &str, port: u16) -> Result<String> {
    let main_url = main_url(address, port);
    Ok(format!("{APP_URL}?add_context={}", encode(&main_url)))
}

pub fn open_browser(url: &str) -> Result<()> {
    if let Ok(browser) = env::var("PREVIA_OPEN_BROWSER") {
        return run_browser(Command::new(&browser).arg(url), &browser);
    }

    #[cfg(target_os = "macos")]
    {
        return run_browser(Command::new("open").arg(url), "open");
    }

    #[cfg(target_os = "windows")]
    {
        return run_browser(
            Command::new("rundll32")
                .arg("url.dll,FileProtocolHandler")
                .arg(url),
            "rundll32",
        );
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        run_browser(Command::new("xdg-open").arg(url), "xdg-open")
    }
}

fn run_browser(command: &mut Command, program: &str) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("failed to launch browser command '{program}'"))?;
    if !status.success() {
        bail!("browser command '{program}' exited with status {status}");
    }
    Ok(())
}

pub fn main_url(address: &str, port: u16) -> String {
    let normalized = match address.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) if ip.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        Ok(IpAddr::V6(ip)) if ip.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        Ok(ip) => ip,
        Err(_) => return format!("http://{address}:{port}"),
    };

    match normalized {
        IpAddr::V4(ip) => format!("http://{ip}:{port}"),
        IpAddr::V6(ip) => format!("http://[{ip}]:{port}"),
    }
}

#[cfg(test)]
mod tests {
    use super::build_open_url;

    #[test]
    fn open_url_normalizes_unspecified_ipv4_bind_address() {
        assert_eq!(
            build_open_url("0.0.0.0", 5588).expect("open url"),
            "https://ide.previa.dev?add_context=http%3A%2F%2F127.0.0.1%3A5588"
        );
    }

    #[test]
    fn open_url_preserves_non_wildcard_hostnames() {
        assert_eq!(
            build_open_url("previ-main.internal", 9000).expect("open url"),
            "https://ide.previa.dev?add_context=http%3A%2F%2Fprevi-main.internal%3A9000"
        );
    }
}
