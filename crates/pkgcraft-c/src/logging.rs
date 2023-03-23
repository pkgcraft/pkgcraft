use std::ffi::{c_char, CString};
use std::fmt::{Debug, Write};

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{prelude::*, Layer};

use crate::macros::*;

#[derive(Debug, Clone)]
#[repr(C)]
pub enum LogLevel {
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

impl<'a> Visit for MessageVisitor<'a> {
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

/// Enable pkgcraft logging support.
#[no_mangle]
pub extern "C" fn pkgcraft_logging_enable(cb: LogCallback) {
    let layer = PkgcraftLayer::new(cb);
    tracing_subscriber::registry().with(layer).init();
}

/// Replay a given PkgcraftLog object for test purposes.
///
/// # Safety
/// The argument must be a non-null PkgcraftLog pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_log_test(l: *const PkgcraftLog) {
    let log = try_ref_from_ptr!(l);
    let message = try_str_from_ptr!(log.message);
    use LogLevel::*;
    match log.level {
        Trace => tracing::trace!("{message}"),
        Debug => tracing::debug!("{message}"),
        Info => tracing::info!("{message}"),
        Warn => tracing::warn!("{message}"),
        Error => tracing::error!("{message}"),
    }
}
