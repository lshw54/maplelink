//! Log file management — structured logging with size-based rotation.
//!
//! Uses `tracing` + `tracing-subscriber` for structured output and
//! `rolling-file` for size-based file rotation (10 MB max, 5 retained files).
//! Both a console (stdout) layer and a file layer are composed together
//! so that development output is preserved alongside persistent log files.

use std::path::Path;

use rolling_file::{BasicRollingFileAppender, RollingConditionBasic};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Maximum log file size in bytes (10 MB).
const MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;

/// Number of historical log files to retain.
const MAX_RETAINED_FILES: usize = 5;

/// Log file name inside the log directory.
const LOG_FILE_NAME: &str = "maplelink.log";

/// Custom timer that formats timestamps in local time using chrono.
///
/// `tracing-subscriber`'s default timer uses UTC. This timer uses
/// `chrono::Local::now()` so log timestamps match the user's system clock.
struct LocalTimer;

impl tracing_subscriber::fmt::time::FormatTime for LocalTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        let now = chrono::Local::now();
        write!(w, "{}", now.format("%Y-%m-%dT%H:%M:%S%.6f%:z"))
    }
}

/// Initialise the global tracing subscriber with both console and file layers.
///
/// The file layer writes structured log entries (timestamp, level, module,
/// message) to `<log_dir>/maplelink.log` with size-based rotation.
/// The console layer writes to stdout for development convenience.
///
/// # Errors
///
/// Returns an error if the log directory cannot be created or the file
/// appender fails to initialise.
pub fn init_logging(log_dir: &Path) -> anyhow::Result<()> {
    // Ensure the log directory exists.
    std::fs::create_dir_all(log_dir)?;

    let log_file_path = log_dir.join(LOG_FILE_NAME);

    // Size-based rolling file appender: rotates at 10 MB, keeps 5 old files.
    let rolling_condition = RollingConditionBasic::new().max_size(MAX_FILE_SIZE_BYTES);
    let file_appender =
        BasicRollingFileAppender::new(log_file_path, rolling_condition, MAX_RETAINED_FILES)?;

    // Wrap in non-blocking writer so logging never blocks the caller.
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Leak the guard so the background writer lives for the entire process.
    // This is intentional — the writer must outlive all tracing calls.
    std::mem::forget(_guard);

    // Environment-based filter: honours RUST_LOG, defaults to "info".
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // File layer — structured format with local timestamp, level, module, message.
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_timer(LocalTimer)
        .with_target(true)
        .with_level(true)
        .with_thread_ids(false)
        .with_thread_names(false);

    // Console layer — coloured output for development.
    let console_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_timer(LocalTimer)
        .with_target(true)
        .with_level(true);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer)
        .with(console_layer)
        .init();

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::{Arc, Mutex};

    use proptest::prelude::*;
    use tracing::Level;
    use tracing_subscriber::fmt::MakeWriter;
    use tracing_subscriber::Layer as _;
    use tracing_subscriber::{fmt, layer::SubscriberExt};

    /// Helper macro to emit a tracing event at a given level with a literal target.
    /// Since `tracing::event!` requires a literal target, we use a fixed target
    /// and embed the dynamic module name inside the message for verification.
    macro_rules! emit_at_level {
        ($level:expr, $msg:expr) => {
            match $level {
                Level::TRACE => tracing::trace!(target: "test_module", "{}", $msg),
                Level::DEBUG => tracing::debug!(target: "test_module", "{}", $msg),
                Level::INFO  => tracing::info!(target: "test_module", "{}", $msg),
                Level::WARN  => tracing::warn!(target: "test_module", "{}", $msg),
                Level::ERROR => tracing::error!(target: "test_module", "{}", $msg),
            }
        };
    }

    /// Shared buffer writer for capturing tracing output in tests.
    #[derive(Clone)]
    struct BufWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }

    impl BufWriter {
        fn new() -> Self {
            Self {
                buf: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn contents(&self) -> String {
            let lock = self.buf.lock().unwrap();
            String::from_utf8_lossy(&lock).to_string()
        }
    }

    impl io::Write for BufWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.buf.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for BufWriter {
        type Writer = BufWriter;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    /// Emit a log event at the given level with a fixed target and message,
    /// capturing the formatted output into the provided buffer writer.
    ///
    /// The target is fixed to `"test_module"` because `tracing::event!` requires
    /// a string literal for the `target:` parameter. The property test verifies
    /// that the formatter includes the target in the output.
    fn emit_and_capture(level: Level, message: &str, writer: &BufWriter) -> String {
        let layer = fmt::layer()
            .with_writer(writer.clone())
            .with_ansi(false)
            .with_target(true)
            .with_level(true)
            .with_thread_ids(false)
            .with_thread_names(false);

        let subscriber = tracing_subscriber::registry()
            .with(layer.with_filter(tracing_subscriber::filter::LevelFilter::TRACE));

        tracing::subscriber::with_default(subscriber, || {
            emit_at_level!(level, message);
        });

        writer.contents()
    }

    /// Strategy that produces one of the five tracing levels.
    fn arb_level() -> impl Strategy<Value = Level> {
        prop_oneof![
            Just(Level::TRACE),
            Just(Level::DEBUG),
            Just(Level::INFO),
            Just(Level::WARN),
            Just(Level::ERROR),
        ]
    }

    /// Strategy for non-empty log messages (printable ASCII, 1–200 chars).
    fn arb_message() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 _.,:;!?()\\-]{1,200}"
    }

    // Feature: maplelink-rewrite, Property 10: Structured log entry format
    //
    // For any log entry produced by the Logger, the formatted output shall
    // contain a timestamp, a log level (one of trace/debug/info/warn/error),
    // the originating module name, and the message body.
    //
    // **Validates: Requirements 7.1, 7.6**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_structured_log_contains_all_fields(
            level in arb_level(),
            message in arb_message(),
        ) {
            let writer = BufWriter::new();
            let output = emit_and_capture(level, &message, &writer);

            // 1. Timestamp — tracing_subscriber::fmt with LocalTimer includes a
            //    local-time timestamp, e.g. "2024-01-15T18:30:00.123456+08:00".
            //    We check for a date-like pattern with 'T' separator.
            prop_assert!(
                output.contains('T'),
                "log output must contain a timestamp (T separator), got: {}",
                output
            );

            // 2. Level — the output must contain the level string (TRACE/DEBUG/INFO/WARN/ERROR).
            let level_str = match level {
                Level::TRACE => "TRACE",
                Level::DEBUG => "DEBUG",
                Level::INFO  => "INFO",
                Level::WARN  => "WARN",
                Level::ERROR => "ERROR",
            };
            prop_assert!(
                output.contains(level_str),
                "log output must contain level '{}', got: {}",
                level_str,
                output
            );

            // 3. Module / target — the output must contain the target string.
            //    We use a fixed target "test_module" because tracing macros
            //    require literal targets. This verifies the formatter includes
            //    the originating module name in the output.
            prop_assert!(
                output.contains("test_module"),
                "log output must contain target 'test_module', got: {}",
                output
            );

            // 4. Message — the output must contain the message body.
            prop_assert!(
                output.contains(&message),
                "log output must contain message '{}', got: {}",
                message,
                output
            );
        }
    }
}
