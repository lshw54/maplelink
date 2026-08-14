//! Network diagnostics: a best-effort geo-IP lookup used to tailor
//! China-specific hints (e.g. the "rename to Beanfun.exe" accelerator
//! suggestion). No DNS changes are made — that feature was removed.

/// Cached result of [`geo_lookup`], so the startup checks that each need the
/// country don't each pay for their own request.
static GEO_CACHE: std::sync::OnceLock<(String, String)> = std::sync::OnceLock::new();

/// [`geo_lookup`], looked up once per run.
///
/// A failed lookup isn't cached: the network may simply not be up yet, and a
/// later caller deserves a real answer rather than an empty one.
pub async fn geo_lookup_cached(client: &reqwest::Client) -> (String, String) {
    if let Some(hit) = GEO_CACHE.get() {
        return hit.clone();
    }
    let fresh = geo_lookup(client).await;
    if fresh.1.is_empty() {
        return fresh;
    }
    let _ = GEO_CACHE.set(fresh.clone());
    fresh
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
