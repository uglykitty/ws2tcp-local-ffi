use std::{
    ffi::{CStr, CString, c_char},
    path::PathBuf,
    sync::{Mutex, Once},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use tokio::{runtime::Runtime, sync::oneshot};
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    EnvFilter, Layer, Registry, layer::Context as LayerContext, prelude::*, registry::LookupSpan,
};
use ws2tcp_local_core::{
    DEFAULT_BUFFER_SIZE, DEFAULT_LISTEN, DEFAULT_RULE_REFRESH_INTERVAL_SECS, ProxyMode, Settings,
    run_proxy,
};

#[repr(C)]
pub enum Ws2TcpStatus {
    Stopped = 0,
    Running = 1,
}

pub struct Ws2TcpHandle {
    runtime: Runtime,
    state: Mutex<State>,
}

type LogCallback = unsafe extern "C" fn(message: *const c_char, user_data: *mut std::ffi::c_void);

struct LogCallbackState {
    callback: Option<LogCallback>,
    user_data: usize,
}

static LOG_CALLBACK: Mutex<LogCallbackState> = Mutex::new(LogCallbackState {
    callback: None,
    user_data: 0,
});
static LOGGING_INIT: Once = Once::new();

struct State {
    task: Option<tokio::task::JoinHandle<Result<(), String>>>,
    shutdown: Option<oneshot::Sender<()>>,
    last_error: CString,
}

#[derive(Debug, Deserialize)]
struct FfiSettings {
    listen: Option<std::net::SocketAddr>,
    gateway: String,
    basic_auth: Option<String>,
    buffer_size: Option<usize>,
    log_level: Option<String>,
    custom_domain_rules: Option<PathBuf>,
    rule_refresh_interval_secs: Option<u64>,
    proxy_mode: Option<ProxyMode>,
    verify_server_certificate: Option<bool>,
}

#[unsafe(no_mangle)]
pub extern "C" fn ws2tcp_handle_new() -> *mut Ws2TcpHandle {
    match Runtime::new() {
        Ok(runtime) => Box::into_raw(Box::new(Ws2TcpHandle {
            runtime,
            state: Mutex::new(State {
                task: None,
                shutdown: None,
                last_error: empty_c_string(),
            }),
        })),
        Err(_) => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws2tcp_handle_free(handle: *mut Ws2TcpHandle) {
    if handle.is_null() {
        return;
    }

    let mut handle = unsafe { Box::from_raw(handle) };
    let _ = stop_handle(&mut handle);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws2tcp_init_logging(log_level: *const c_char) -> i32 {
    let log_level = match optional_c_str(log_level) {
        Ok(value) => value,
        Err(err) => return ffi_error_without_handle(err),
    };

    match ws2tcp_local_core::init_logging(log_level.as_deref()) {
        Ok(()) => WS2TCP_OK,
        Err(err) => ffi_error_without_handle(err),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws2tcp_set_log_callback(
    callback: Option<LogCallback>,
    user_data: *mut std::ffi::c_void,
    log_level: *const c_char,
) -> i32 {
    {
        let mut state = LOG_CALLBACK.lock().unwrap_or_else(|err| err.into_inner());
        state.callback = callback;
        state.user_data = user_data as usize;
    }

    let log_level = match optional_c_str(log_level) {
        Ok(value) => value,
        Err(err) => return ffi_error_without_handle(err),
    };
    let result = init_callback_logging(log_level.as_deref());
    emit_log("ws2tcp-local log callback installed");
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws2tcp_start(
    handle: *mut Ws2TcpHandle,
    config_json: *const c_char,
) -> i32 {
    let handle = match unsafe { handle.as_mut() } {
        Some(handle) => handle,
        None => return WS2TCP_ERROR_NULL_HANDLE,
    };

    if is_running(handle) {
        lock_state(handle).last_error = c_string_lossy("proxy is already running");
        return WS2TCP_ERROR_ALREADY_RUNNING;
    }

    match start_handle(handle, config_json) {
        Ok(()) => WS2TCP_OK,
        Err(err) => {
            set_last_error(handle, err);
            WS2TCP_ERROR_RUNTIME
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws2tcp_stop(handle: *mut Ws2TcpHandle) -> i32 {
    let handle = match unsafe { handle.as_mut() } {
        Some(handle) => handle,
        None => return WS2TCP_ERROR_NULL_HANDLE,
    };

    match stop_handle(handle) {
        Ok(()) => WS2TCP_OK,
        Err(err) => {
            set_last_error(handle, err);
            WS2TCP_ERROR_RUNTIME
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws2tcp_status(handle: *mut Ws2TcpHandle) -> Ws2TcpStatus {
    let handle = match unsafe { handle.as_mut() } {
        Some(handle) => handle,
        None => return Ws2TcpStatus::Stopped,
    };

    let mut state = lock_state(handle);
    reap_finished_task(handle, &mut state);
    if state.task.is_some() {
        Ws2TcpStatus::Running
    } else {
        Ws2TcpStatus::Stopped
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws2tcp_last_error(handle: *mut Ws2TcpHandle) -> *const c_char {
    let handle = match unsafe { handle.as_mut() } {
        Some(handle) => handle,
        None => return c"null handle".as_ptr(),
    };

    lock_state(handle).last_error.as_ptr()
}

const WS2TCP_OK: i32 = 0;
const WS2TCP_ERROR_NULL_HANDLE: i32 = 1;
const WS2TCP_ERROR_INVALID_ARGUMENT: i32 = 2;
const WS2TCP_ERROR_ALREADY_RUNNING: i32 = 3;
const WS2TCP_ERROR_RUNTIME: i32 = 4;

fn start_handle(handle: &mut Ws2TcpHandle, config_json: *const c_char) -> Result<()> {
    let json = required_c_str(config_json).context("config_json is required")?;
    let settings = parse_settings(&json)?;
    emit_log(&format!(
        "starting proxy listen={} gateway={} proxy_mode={:?}",
        settings.listen, settings.gateway, settings.proxy_mode
    ));

    let mut state = lock_state(handle);
    reap_finished_task(handle, &mut state);
    if state.task.is_some() {
        bail!("proxy is already running");
    }

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let task = handle.runtime.spawn(async move {
        let result = run_proxy(settings, async {
            let _ = shutdown_rx.await;
        })
        .await
        .map_err(|err| format!("{err:#}"));
        match &result {
            Ok(()) => emit_log("proxy task stopped"),
            Err(err) => emit_log(&format!("proxy task failed: {err}")),
        }
        result
    });

    state.shutdown = Some(shutdown_tx);
    state.task = Some(task);
    state.last_error = empty_c_string();
    Ok(())
}

fn stop_handle(handle: &mut Ws2TcpHandle) -> Result<()> {
    let task = {
        let mut state = lock_state(handle);
        if let Some(shutdown) = state.shutdown.take() {
            emit_log("stopping proxy");
            let _ = shutdown.send(());
        }
        state.task.take()
    };

    if let Some(task) = task {
        match handle.runtime.block_on(task) {
            Ok(Ok(())) => {}
            Ok(Err(err)) => bail!("{err}"),
            Err(err) => bail!("proxy task join failed: {err}"),
        }
    }

    Ok(())
}

fn reap_finished_task(handle: &Ws2TcpHandle, state: &mut State) {
    let is_finished = state.task.as_ref().is_some_and(|task| task.is_finished());
    if !is_finished {
        return;
    }

    if let Some(task) = state.task.take() {
        state.shutdown = None;
        match handle.runtime.block_on(task) {
            Ok(Ok(())) => {}
            Ok(Err(err)) => state.last_error = c_string_lossy(err),
            Err(err) => state.last_error = c_string_lossy(format!("proxy task join failed: {err}")),
        }
    }
}

fn is_running(handle: &Ws2TcpHandle) -> bool {
    let mut state = lock_state(handle);
    reap_finished_task(handle, &mut state);
    state.task.is_some()
}

fn parse_settings(json: &str) -> Result<Settings> {
    let settings: FfiSettings =
        serde_json::from_str(json).context("failed to parse config_json")?;
    let buffer_size = settings.buffer_size.unwrap_or(DEFAULT_BUFFER_SIZE);
    if buffer_size == 0 {
        bail!("buffer_size must be greater than 0");
    }
    let rule_refresh_interval_secs = settings
        .rule_refresh_interval_secs
        .unwrap_or(DEFAULT_RULE_REFRESH_INTERVAL_SECS);
    if rule_refresh_interval_secs == 0 {
        bail!("rule_refresh_interval_secs must be greater than 0");
    }

    Ok(Settings {
        listen: settings.listen.unwrap_or(
            DEFAULT_LISTEN
                .parse()
                .context("invalid default listen address")?,
        ),
        gateway: settings.gateway,
        basic_auth: settings.basic_auth,
        buffer_size,
        log_level: settings.log_level,
        custom_domain_rules: settings.custom_domain_rules,
        rule_refresh_interval: Duration::from_secs(rule_refresh_interval_secs),
        proxy_mode: settings.proxy_mode.unwrap_or(ProxyMode::Global),
        verify_server_certificate: settings.verify_server_certificate.unwrap_or(false),
    })
}

fn lock_state(handle: &Ws2TcpHandle) -> std::sync::MutexGuard<'_, State> {
    handle.state.lock().unwrap_or_else(|err| err.into_inner())
}

fn set_last_error(handle: &Ws2TcpHandle, err: anyhow::Error) {
    lock_state(handle).last_error = c_string_lossy(format!("{err:#}"));
}

fn required_c_str(ptr: *const c_char) -> Result<String> {
    if ptr.is_null() {
        bail!("null string pointer");
    }

    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map(str::to_owned)
        .context("string pointer is not valid UTF-8")
}

fn optional_c_str(ptr: *const c_char) -> Result<Option<String>> {
    if ptr.is_null() {
        return Ok(None);
    }

    required_c_str(ptr).map(Some)
}

fn ffi_error_without_handle(err: anyhow::Error) -> i32 {
    let _ = err;
    WS2TCP_ERROR_INVALID_ARGUMENT
}

fn init_callback_logging(log_level: Option<&str>) -> i32 {
    let mut init_result = Ok(());
    LOGGING_INIT.call_once(|| {
        let filter = match log_level {
            Some(filter) => filter.to_owned(),
            None => std::env::var("RUST_LOG").unwrap_or_else(|_| "ws2tcp_local=info".to_owned()),
        };
        let subscriber = Registry::default()
            .with(EnvFilter::new(filter))
            .with(CallbackLogLayer);
        init_result = tracing::subscriber::set_global_default(subscriber)
            .map_err(|err| format!("failed to initialize callback logging: {err}"));
    });

    match init_result {
        Ok(()) => WS2TCP_OK,
        Err(_) => WS2TCP_ERROR_RUNTIME,
    }
}

struct CallbackLogLayer;

impl<S> Layer<S> for CallbackLogLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: LayerContext<'_, S>) {
        let mut visitor = LogVisitor::default();
        event.record(&mut visitor);
        let metadata = event.metadata();
        let message = if visitor.fields.is_empty() {
            format!("{} {}", metadata.level(), metadata.target())
        } else {
            format!(
                "{} {}: {}",
                metadata.level(),
                metadata.target(),
                visitor.fields.join(" ")
            )
        };
        emit_log(&message);
    }
}

#[derive(Default)]
struct LogVisitor {
    fields: Vec<String>,
}

impl tracing::field::Visit for LogVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.record_value(field, value);
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let value = format!("{value:?}");
        self.record_value(field, value);
    }
}

impl LogVisitor {
    fn record_value(&mut self, field: &tracing::field::Field, value: impl AsRef<str>) {
        if field.name() == "message" {
            self.fields.push(value.as_ref().to_owned());
        } else {
            self.fields
                .push(format!("{}={}", field.name(), value.as_ref()));
        }
    }
}

fn emit_log(message: &str) {
    let c_message = c_string_lossy(message);
    let state = LOG_CALLBACK.lock().unwrap_or_else(|err| err.into_inner());
    if let Some(callback) = state.callback {
        unsafe {
            callback(c_message.as_ptr(), state.user_data as *mut std::ffi::c_void);
        }
    }
}

fn empty_c_string() -> CString {
    c"".to_owned()
}

fn c_string_lossy(value: impl AsRef<str>) -> CString {
    let without_nuls = value.as_ref().replace('\0', "\\0");
    CString::new(without_nuls).unwrap_or_else(|_| empty_c_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::{CStr, CString, c_char},
        sync::Mutex,
    };

    static TEST_LOGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

    unsafe extern "C" fn collect_log(message: *const c_char, _user_data: *mut std::ffi::c_void) {
        if message.is_null() {
            return;
        }
        let message = unsafe { CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned();
        TEST_LOGS
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(message);
    }

    #[test]
    fn parses_minimal_settings_with_defaults() {
        let settings = parse_settings(r#"{"gateway":"ws://127.0.0.1:8000"}"#).unwrap();

        assert_eq!(settings.listen.to_string(), DEFAULT_LISTEN);
        assert_eq!(settings.gateway, "ws://127.0.0.1:8000");
        assert_eq!(settings.buffer_size, DEFAULT_BUFFER_SIZE);
        assert_eq!(settings.proxy_mode, ProxyMode::Global);
        assert!(!settings.verify_server_certificate);
    }

    #[test]
    fn rejects_zero_buffer_size() {
        let err =
            parse_settings(r#"{"gateway":"ws://127.0.0.1:8000","buffer_size":0}"#).unwrap_err();

        assert!(err.to_string().contains("buffer_size"));
    }

    #[test]
    fn ffi_start_stop_lifecycle() {
        TEST_LOGS
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clear();
        assert_eq!(
            unsafe {
                ws2tcp_set_log_callback(
                    Some(collect_log),
                    std::ptr::null_mut(),
                    c"ws2tcp_local=info,ws2tcp_local_ffi=info".as_ptr(),
                )
            },
            WS2TCP_OK
        );

        let handle = ws2tcp_handle_new();
        assert!(!handle.is_null());

        let config = CString::new(
            r#"{"listen":"127.0.0.1:0","gateway":"ws://127.0.0.1:1","proxy_mode":"global"}"#,
        )
        .unwrap();

        assert_eq!(unsafe { ws2tcp_start(handle, config.as_ptr()) }, WS2TCP_OK);
        assert!(matches!(
            unsafe { ws2tcp_status(handle) },
            Ws2TcpStatus::Running
        ));
        assert_eq!(unsafe { ws2tcp_stop(handle) }, WS2TCP_OK);
        assert!(matches!(
            unsafe { ws2tcp_status(handle) },
            Ws2TcpStatus::Stopped
        ));

        unsafe { ws2tcp_handle_free(handle) };

        let logs = TEST_LOGS.lock().unwrap_or_else(|err| err.into_inner());
        assert!(
            logs.iter()
                .any(|line| line.contains("starting proxy listen=127.0.0.1:0")),
            "logs did not contain startup line: {logs:?}"
        );
    }
}
