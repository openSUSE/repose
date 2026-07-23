//! User-facing text / NDJSON sink (Python `repose.console.Console`).

use std::io::{self, Write};

use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warning,
    Error,
}

impl Level {
    #[must_use]
    const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// Collecting writer for tests.
#[derive(Debug, Default)]
pub struct Buffer(pub String);

impl Write for Buffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.push_str(&String::from_utf8_lossy(buf));
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct Console<W: Write> {
    stream: W,
    pub format: OutputFormat,
    pub color: ColorMode,
    /// When `ColorMode::Auto`, honour this (tests inject false).
    pub force_color: Option<bool>,
}

impl<W: Write> Console<W> {
    pub const fn new(stream: W) -> Self {
        Self {
            stream,
            format: OutputFormat::Text,
            color: ColorMode::Auto,
            force_color: Some(false),
        }
    }

    fn use_color(&self) -> bool {
        match self.color {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => {
                if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
                    return false;
                }
                if let Ok(c) = std::env::var("COLOR") {
                    if c.eq_ignore_ascii_case("never") {
                        return false;
                    }
                    if c.eq_ignore_ascii_case("always") {
                        return true;
                    }
                }
                self.force_color.unwrap_or(false)
            }
        }
    }

    pub(crate) fn dry(&mut self, host: &str, cmd: &str) -> io::Result<()> {
        self.emit("dry", Level::Info, json!({"host": host, "cmd": cmd}))
    }

    pub(crate) fn report(
        &mut self,
        host: &str,
        line: &str,
        ok: bool,
        level: Level,
    ) -> io::Result<()> {
        self.emit(
            "report",
            level,
            json!({"host": host, "line": line, "ok": ok}),
        )
    }

    pub(crate) fn error(&mut self, host: &str, msg: &str) -> io::Result<()> {
        self.emit(
            "error",
            Level::Error,
            json!({"host": host, "line": msg, "ok": false}),
        )
    }

    pub(crate) fn info(&mut self, msg: &str) -> io::Result<()> {
        self.emit("info", Level::Info, json!({"line": msg}))
    }

    fn emit(&mut self, event: &str, level: Level, fields: Value) -> io::Result<()> {
        if self.format == OutputFormat::Json {
            let mut payload = json!({"event": event, "level": level.as_str()});
            if let Some(obj) = payload.as_object_mut()
                && let Some(map) = fields.as_object()
            {
                for (k, v) in map {
                    obj.insert(k.clone(), v.clone());
                }
            }
            writeln!(self.stream, "{payload}")?;
            return Ok(());
        }

        let host = fields.get("host").and_then(|h| h.as_str());
        let cmd = fields.get("cmd").and_then(|c| c.as_str());
        let line = fields.get("line").and_then(|l| l.as_str());

        match (event, host, cmd, line) {
            ("dry", Some(host), Some(cmd), _) => {
                let h = self.colorize_host(host, Level::Info);
                writeln!(self.stream, "{h} - {cmd}")?;
            }
            ("report" | "error", Some(host), _, Some(line)) => {
                let h = self.colorize_host(host, level);
                writeln!(self.stream, "{h} - {line}")?;
            }
            ("info", _, _, Some(line)) => {
                writeln!(self.stream, "{line}")?;
            }
            _ => {}
        }
        Ok(())
    }

    fn colorize_host(&self, host: &str, level: Level) -> String {
        if !self.use_color() {
            return host.to_string();
        }
        let seq = match level {
            Level::Info => "\x1b[1;34m",
            Level::Warning => "\x1b[1;33m",
            Level::Error => "\x1b[1;31m",
        };
        format!("{seq}{host}\x1b[1;m")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_text() {
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        c.dry("h1", "zypper -n lr").unwrap();
        assert_eq!(buf.0, "h1 - zypper -n lr\n");
    }

    #[test]
    fn dry_json() {
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        c.format = OutputFormat::Json;
        c.dry("h", "zypper -n lr").unwrap();
        let v: Value = serde_json::from_str(buf.0.trim()).unwrap();
        assert_eq!(v["event"], "dry");
        assert_eq!(v["host"], "h");
        assert_eq!(v["cmd"], "zypper -n lr");
    }

    fn colorized(host: &str) -> String {
        format!("\x1b[1;34m{host}\x1b[1;m")
    }

    // NO_COLOR/COLOR are process-global; mutating them races the whole test
    // binary. These cases exercise the mode short-circuits (Always/Never),
    // which return before any env read and are thus deterministic regardless
    // of ambient env or parallel tests. Env-precedence (NO_COLOR / COLOR) is
    // covered by the per-process subprocess tests in repose-cli/tests/cli.rs.

    #[test]
    fn always_mode_colorizes() {
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        c.color = ColorMode::Always;
        c.report("h1", "ok", true, Level::Info).unwrap();
        assert_eq!(buf.0, format!("{} - ok\n", colorized("h1")));
    }

    #[test]
    fn never_mode_suppresses_color() {
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        c.color = ColorMode::Never;
        // Never must win even when a TTY was detected.
        c.force_color = Some(true);
        c.report("h1", "ok", true, Level::Info).unwrap();
        assert_eq!(buf.0, "h1 - ok\n");
    }

    #[test]
    fn error_and_warning_levels_use_their_sequences() {
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        c.color = ColorMode::Always;
        c.error("h1", "boom").unwrap();
        c.report("h1", "warn", true, Level::Warning).unwrap();
        assert_eq!(
            buf.0,
            "\x1b[1;31mh1\x1b[1;m - boom\n\x1b[1;33mh1\x1b[1;m - warn\n"
        );
    }

    #[test]
    fn ndjson_vector_shapes() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/vectors/ndjson/events.jsonl");
        let raw = std::fs::read_to_string(path).unwrap();
        for line in raw.lines().filter(|l| !l.is_empty()) {
            let v: Value = serde_json::from_str(line).unwrap();
            assert!(v.get("event").is_some());
        }
    }
}
