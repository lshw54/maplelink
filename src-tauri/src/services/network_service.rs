//! Network diagnostics + DNS helpers.
//!
//! Some users (often in mainland China) are on flaky ISP DNS that fails to
//! resolve `google.com` / beanfun, which breaks reCAPTCHA and login. This module
//! detects the situation and can switch the active adapter to Alibaba public DNS
//! (`223.5.5.5` / `223.6.6.6`) — the switch needs admin, so it's run through an
//! elevated PowerShell (UAC), and can be reverted back to automatic (DHCP).

/// Alibaba (AliDNS) public resolvers — reliable inside mainland China.
pub const RECOMMENDED_DNS: [&str; 2] = ["223.5.5.5", "223.6.6.6"];

#[derive(serde::Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DnsStatus {
    /// Public IP as seen by the geo-IP service (empty if lookup failed).
    pub public_ip: String,
    /// ISO country code, e.g. `CN` (empty if lookup failed).
    pub country_code: String,
    /// The public IP geolocates to mainland China.
    pub is_china: bool,
    /// DNS servers currently set on the active (default-route) adapter.
    pub current_dns: Vec<String>,
    /// The active adapter is already using the recommended DNS.
    pub using_recommended: bool,
}

#[derive(serde::Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DnsTestResult {
    /// `login.beanfun.com` resolved.
    pub beanfun_ok: bool,
    /// `www.google.com` (needed for reCAPTCHA) resolved.
    pub google_ok: bool,
}

/// Geo-IP lookup via ip-api.com. Returns `(public_ip, country_code)`, empty on
/// failure. Best-effort — never errors.
pub async fn geo_lookup(client: &reqwest::Client) -> (String, String) {
    let url = "http://ip-api.com/json/?fields=status,countryCode,query";
    match client.get(url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(j) => (
                j["query"].as_str().unwrap_or_default().to_string(),
                j["countryCode"].as_str().unwrap_or_default().to_string(),
            ),
            Err(_) => (String::new(), String::new()),
        },
        Err(_) => (String::new(), String::new()),
    }
}

/// Resolve the two domains login/reCAPTCHA depend on, via the OS resolver (so it
/// reflects whatever DNS is currently configured after a switch + cache clear).
pub async fn test_resolution() -> DnsTestResult {
    async fn resolves(host: &str) -> bool {
        tokio::net::lookup_host((host, 443))
            .await
            .map(|mut it| it.next().is_some())
            .unwrap_or(false)
    }
    DnsTestResult {
        beanfun_ok: resolves("login.beanfun.com").await,
        google_ok: resolves("www.google.com").await,
    }
}

// ---------------------------------------------------------------------------
// Windows: read / set / reset the active adapter's DNS
// ---------------------------------------------------------------------------

/// PowerShell snippet resolving the default-route IPv4 interface index into `$i`.
#[cfg(target_os = "windows")]
const ACTIVE_IF: &str = "$i=(Get-NetRoute -DestinationPrefix '0.0.0.0/0' -EA SilentlyContinue | Sort-Object RouteMetric | Select-Object -First 1).InterfaceIndex;";

/// DNS servers on the active adapter (IPv4).
#[cfg(target_os = "windows")]
pub fn current_dns() -> Vec<String> {
    use std::os::windows::process::CommandExt;
    let script = format!(
        "{ACTIVE_IF} (Get-DnsClientServerAddress -InterfaceIndex $i -AddressFamily IPv4 -EA SilentlyContinue).ServerAddresses -join ','"
    );
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Switch the active adapter to the recommended DNS (elevated).
#[cfg(target_os = "windows")]
pub fn set_recommended_dns() -> Result<(), String> {
    let addrs = RECOMMENDED_DNS
        .iter()
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(",");
    run_elevated(&format!(
        "{ACTIVE_IF} Set-DnsClientServerAddress -InterfaceIndex $i -ServerAddresses {addrs}; Clear-DnsClientCache"
    ))
}

/// Revert the active adapter to automatic DNS (DHCP) (elevated).
#[cfg(target_os = "windows")]
pub fn reset_dns() -> Result<(), String> {
    run_elevated(&format!(
        "{ACTIVE_IF} Set-DnsClientServerAddress -InterfaceIndex $i -ResetServerAddresses; Clear-DnsClientCache"
    ))
}

/// Run `inner` in an elevated PowerShell (UAC). The script is UTF-16LE-base64
/// encoded (`-EncodedCommand`) to sidestep quoting. Returns `Err` if the user
/// declines the UAC prompt or the command fails.
#[cfg(target_os = "windows")]
fn run_elevated(inner: &str) -> Result<(), String> {
    use base64::Engine;
    use std::os::windows::process::CommandExt;

    let mut utf16 = Vec::new();
    for w in inner.encode_utf16() {
        utf16.extend_from_slice(&w.to_le_bytes());
    }
    let encoded = base64::engine::general_purpose::STANDARD.encode(&utf16);

    // Non-elevated outer shell launches the elevated inner one and waits for it.
    let outer = format!(
        "try {{ $p = Start-Process powershell -Verb RunAs -Wait -PassThru -WindowStyle Hidden -ArgumentList '-NoProfile','-NonInteractive','-EncodedCommand','{encoded}'; exit $p.ExitCode }} catch {{ exit 1223 }}"
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &outer])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()
        .map_err(|e| e.to_string())?;

    if out.status.success() {
        Ok(())
    } else {
        // 1223 = ERROR_CANCELLED (user declined UAC).
        Err(match out.status.code() {
            Some(1223) => "cancelled".to_string(),
            _ => {
                let err = String::from_utf8_lossy(&out.stderr);
                if err.trim().is_empty() {
                    "failed".to_string()
                } else {
                    err.trim().to_string()
                }
            }
        })
    }
}

#[cfg(not(target_os = "windows"))]
pub fn current_dns() -> Vec<String> {
    Vec::new()
}
#[cfg(not(target_os = "windows"))]
pub fn set_recommended_dns() -> Result<(), String> {
    Err("DNS switching is only supported on Windows".to_string())
}
#[cfg(not(target_os = "windows"))]
pub fn reset_dns() -> Result<(), String> {
    Err("DNS switching is only supported on Windows".to_string())
}
