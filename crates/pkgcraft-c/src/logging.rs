use std::cell::RefCell;
use std::ffi::{c_char, CString};
use std::fmt::{Debug, Write};

use tracing::field::{Field, Visit};
use tracing::{subscriber::DefaultGuard, Event, Level, Subscriber};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::{prelude::*, registry::Registry, Layer};

use crate::macros::*;

#[derive(Debug, Clone)]
#[repr(C)]
pub enum LogLevel {
    Off,
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl<'a> From<&'a Level> for LogLevel {
    fn from(level: &'a Level) -> Self {
        match *level {
            Level::TRACE => Self::Trace,
            Level::DEBUG => Self::Debug,
            Level::INFO => Self::Info,
            Level::WARN => Self::Warn,
            Level::ERROR => Self::Error,
        }
    }
}

impl From<LogLevel> for LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Off => LevelFilter::OFF,
            LogLevel::Trace => LevelFilter::TRACE,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Error => LevelFilter::ERROR,
        }
    }
}

#[repr(C)]
pub struct PkgcraftLog {
    message: *mut c_char,
    level: LogLevel,
}

impl<'a> From<&'a Event<'_>> for PkgcraftLog {
    fn from(event: &'a Event<'_>) -> Self {
        let mut message = String::new();
        let mut visitor = MessageVisitor { message: &mut message };
        event.record(&mut visitor);

        Self {
            message: try_ptr_from_str!(message),
            level: event.metadata().level().into(),
        }
    }
}

impl Drop for PkgcraftLog {
    fn drop(&mut self) {
        unsafe {
            drop(CString::from_raw(self.message));
        }
    }
}

/// Free a log.
///
/// # Safety
/// The argument must be a non-null PkgcraftLog pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_log_free(l: *mut PkgcraftLog) {
    if !l.is_null() {
        unsafe { drop(Box::from_raw(l)) };
    }
}

type LogCallback = extern "C" fn(*mut PkgcraftLog);

pub struct PkgcraftLayer {
    log_cb: LogCallback,
}

impl PkgcraftLayer {
    fn new(log_cb: LogCallback) -> Self {
        Self { log_cb }
    }
}

struct MessageVisitor<'a> {
    message: &'a mut String,
}

impl Visit for MessageVisitor<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "message" {
            write!(self.message, "{value:?}").unwrap();
        }
    }
}

impl<S> Layer<S> for PkgcraftLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        (self.log_cb)(Box::into_raw(Box::new(event.into())));
    }
}

thread_local! {
    static SUBSCRIBER: RefCell<Option<DefaultGuard>> = const { RefCell::new(None) };
}

/// Enable pkgcraft logging support.
#[no_mangle]
pub extern "C" fn pkgcraft_logging_enable(cb: LogCallback, level: LogLevel) {
    let level_filter: LevelFilter = level.into();
    let filter = EnvFilter::builder()
        .with_default_directive(level_filter.into())
        .from_env_lossy();

    let subscriber = Registry::default()
        .with(filter)
        .with(PkgcraftLayer::new(cb));

    // replace the current thread's subscriber
    SUBSCRIBER.with(|prev| *prev.borrow_mut() = None);
    let guard = tracing::subscriber::set_default(subscriber);
    SUBSCRIBER.with(|prev| *prev.borrow_mut() = Some(guard));
}

/// Replay a given PkgcraftLog object for test purposes.
///
/// # Safety
/// The argument must be a non-null PkgcraftLog pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_log_test(msg: *const c_char, level: LogLevel) {
    let message = try_str_from_ptr!(msg);
    use LogLevel::*;
    match level {
        Off => (),
        Trace => tracing::trace!("{message}"),
        Debug => tracing::debug!("{message}"),
        Info => tracing::info!("{message}"),
        Warn => tracing::warn!("{message}"),
        Error => tracing::error!("{message}"),
    }
}
