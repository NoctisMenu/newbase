use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

use log::{Level, LevelFilter, Log, Metadata, Record};

const MAX_ENTRIES: usize = 512;

#[derive(Clone, Copy, Debug)]
pub struct LogEntry {
    pub level: Level,
    pub seconds: f32,
    pub target: [u8; 48],
    pub target_len: u8,
    pub message: [u8; 192],
    pub message_len: u8,
}

impl LogEntry {
    pub fn message_str(&self) -> &str {
        std::str::from_utf8(&self.message[..self.message_len as usize]).unwrap_or("")
    }

    pub fn target_str(&self) -> &str {
        std::str::from_utf8(&self.target[..self.target_len as usize]).unwrap_or("")
    }
}

struct Capture {
    entries: Mutex<VecDeque<LogEntry>>,
    console_level: Mutex<LevelFilter>,
    epoch: Instant,
}

static CAPTURE: std::sync::LazyLock<Capture> = std::sync::LazyLock::new(|| Capture {
    entries: Mutex::new(VecDeque::with_capacity(MAX_ENTRIES)),
    console_level: Mutex::new(LevelFilter::Info),
    epoch: Instant::now(),
});

struct CaptureLogger;

fn strip_crate_prefix(bytes: &[u8]) -> &[u8] {
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b':' && bytes[i + 1] == b':' {
            return &bytes[i + 2..];
        }
        i += 1;
    }
    bytes
}

fn copy_into<const N: usize>(dst: &mut [u8; N], src: &[u8]) -> u8 {
    let len = src.len().min(N);
    dst[..len].copy_from_slice(&src[..len]);
    len as u8
}

impl Log for CaptureLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let seconds = CAPTURE.epoch.elapsed().as_secs_f32();

        let stripped = strip_crate_prefix(record.target().as_bytes());
        let mut target = [0u8; 48];
        let target_len = copy_into(&mut target, stripped);

        let args = record.args().to_string();
        let mut message = [0u8; 192];
        let message_len = copy_into(&mut message, args.as_bytes());

        let entry = LogEntry {
            level: record.level(),
            seconds,
            target,
            target_len,
            message,
            message_len,
        };

        if let Ok(mut entries) = CAPTURE.entries.lock() {
            if entries.len() >= MAX_ENTRIES {
                entries.pop_front();
            }
            entries.push_back(entry);
        }

        let console_level = CAPTURE
            .console_level
            .lock()
            .map(|guard| *guard)
            .unwrap_or(LevelFilter::Info);
        if record.level() <= console_level {
            let level_str = match record.level() {
                Level::Error => "ERROR",
                Level::Warn => "WARN",
                Level::Info => "INFO",
                Level::Debug => "DEBUG",
                Level::Trace => "TRACE",
            };
            let target_str = std::str::from_utf8(stripped).unwrap_or(record.target());
            println!("[{seconds:8.2} {level_str:5} {target_str}] {}", record.args());
        }
    }

    fn flush(&self) {}
}

static LOGGER: CaptureLogger = CaptureLogger;

pub fn init() {
    log::set_logger(&LOGGER).expect("logger already set");
    log::set_max_level(LevelFilter::Trace);
}

pub fn set_level(level: LevelFilter) {
    if let Ok(mut guard) = CAPTURE.console_level.lock() {
        *guard = level;
    }
}

pub fn drain_json() -> String {
    let entries: Vec<LogEntry> = CAPTURE
        .entries
        .lock()
        .map(|mut queue| queue.drain(..).collect())
        .unwrap_or_default();
    if entries.is_empty() {
        return String::from("[]");
    }
    let mut json = String::with_capacity(entries.len() * 80);
    json.push('[');
    for (i, entry) in entries.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        let level = match entry.level {
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
            Level::Debug => "debug",
            Level::Trace => "trace",
        };
        let message = entry
            .message_str()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', " ");
        let target = entry
            .target_str()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        json.push_str(&format!(
            "{{\"l\":\"{level}\",\"t\":{:.1},\"src\":\"{target}\",\"m\":\"{message}\"}}",
            entry.seconds
        ));
    }
    json.push(']');
    json
}
