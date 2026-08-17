//! TW game start by way of the Gamania Games Manager.
//!
//! beanfun's TW credential endpoint now answers only the Gamania Games Manager:
//! it wants the caller's assembly version and the SHA-256 of GGM's own binary,
//! and refuses anything else with "Query String Error". Its website stopped
//! asking too — the game-start page hands GGM an opaque ticket over a
//! `gamaniagames://` URL and lets GGM do the fetch.
//!
//! So we do the same. GGM fetches the credentials and starts "the game", which
//! the Gamania registry key points back at MapleLink; that intercepted process
//! reports the credentials to us and exits, and from there the launch continues
//! exactly as it always did — the OTP appears in its box, auto-fill runs, and
//! the game starts through Locale Remulator.

use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::models::game_account::GameCredentials;
use crate::models::session::Session;
use crate::services::{beanfun_service, process_service, web_launch};

/// How long to wait for the interception to report back. The game manager may
/// update itself first, which is slow but finite.
const HANDOFF_TIMEOUT: Duration = Duration::from_secs(90);

/// How often to look for its answer. Frequent enough to feel immediate, cheap
/// enough to be free — it is one `stat` of a file that usually isn't there.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// A launch's result, waiting for the second half of the same request.
///
/// One press asks twice: once to show the code, once to type it. Both want the
/// code from the *same* launch — fetching again would start the game manager a
/// second time and produce a different one-time password. So the result is kept
/// for exactly one more taker and then dropped: the next press is a new press,
/// and gets a new code. A password reused after it has been spent is refused by
/// the game, so holding one any longer would break the thing it was meant to
/// fix.
struct Spare {
    account_id: String,
    credentials: GameCredentials,
    at: Instant,
}

/// Safety net for the second taker that never comes — a caller that failed
/// before asking, say. Far shorter than the code's own lifetime.
const SPARE_TTL: Duration = Duration::from_secs(30);

/// The spare, and the lock that makes a concurrent asker queue behind the
/// launch in progress rather than starting its own.
static SPARE: Mutex<Option<Spare>> = Mutex::const_new(None);

/// Start a TW game through the game manager and return the credentials it got.
pub async fn credentials_via_ggm(
    client: &reqwest::Client,
    session: &Session,
    account_id: &str,
    cookie_jar: &std::sync::Arc<reqwest::cookie::Jar>,
) -> Result<GameCredentials, beanfun_service::LoginError> {
    // Held across the whole handoff, so a second caller waits for this launch
    // instead of starting another one — and then finds its spare below.
    let mut spare = SPARE.lock().await;
    if let Some(held) = spare.take() {
        if held.account_id == account_id && held.at.elapsed() < SPARE_TTL {
            tracing::info!(account_id, "ggm: handing over this launch's other half");
            return Ok(held.credentials);
        }
        // Someone else's, or too old to still be good: let it go and fetch.
    }

    // Nothing here works without the game manager, and opening its URL when it
    // isn't installed puts a Windows dialog on screen — "your device needs a
    // new app to open this link" — that says nothing about what MapleLink was
    // doing or what went wrong before it. Stop short of that.
    if !process_service::ggm_installed() {
        return Err(beanfun_service::launch_handoff_error(
            "the Gamania Games Manager is not installed",
        ));
    }

    // The fallback has no cached list to hand over; it fetches one, which on
    // this path is the least of the costs.
    let ticket =
        beanfun_service::tw_ggm_ticket(client, session, account_id, cookie_jar, &[]).await?;

    // Without the interception the game manager starts the game directly and we
    // never see the account: no locale emulation, no auto-fill, and nothing to
    // put in the OTP box.
    match web_launch::register_direct() {
        Ok(()) => tracing::debug!("ggm: interception points at MapleLink"),
        Err(e) => tracing::warn!("ggm: could not point the interception at us: {e}"),
    }

    web_launch::mark_ggm_pending();

    let uri = ticket.launch_uri();
    tracing::info!(uri_len = uri.len(), "ggm: opening the launch URL");
    if let Err(e) = process_service::open_ggm_uri(&uri) {
        web_launch::clear_ggm_pending();
        return Err(beanfun_service::launch_handoff_error(&format!(
            "could not reach the game manager: {e}"
        )));
    }

    let started = std::time::Instant::now();
    while started.elapsed() < HANDOFF_TIMEOUT {
        if let Some(creds) = web_launch::take_ggm_handoff() {
            tracing::info!(
                account_id,
                waited_ms = started.elapsed().as_millis(),
                "ggm: credentials received from the interception"
            );
            let credentials = GameCredentials {
                account_id: account_id.to_string(),
                otp: creds.otp,
                retrieved_at: chrono::Utc::now(),
                command_line_template: Some(
                    "tw.login.maplestory.beanfun.com 8484 BeanFun %s %s".to_string(),
                ),
            };
            // Left for the other half of this same request; whoever doesn't
            // come for it loses it to the TTL.
            *spare = Some(Spare {
                account_id: account_id.to_string(),
                credentials: credentials.clone(),
                at: Instant::now(),
            });
            return Ok(credentials);
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }

    web_launch::clear_ggm_pending();
    Err(beanfun_service::launch_handoff_error(
        "the game manager did not report back in time",
    ))
}
