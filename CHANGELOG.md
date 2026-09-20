# Changelog

## 0.2.0 - 2026-09-20

### Added

- The `config_json` accepts `auth_mode`, `"token"` (the default) or `"basic"`. `"token"` sends
  no health check and logs in once for a short-lived access token that is renewed
  automatically in the background; it needs credentials in `basic_auth` and a gateway with
  token authentication, and a failed login is reported as
  `WS2TCP_ERROR_KIND_GATEWAY_CHECK_FAILED` (or `WS2TCP_ERROR_KIND_AUTH_FAILED` for rejected
  credentials). `"basic"`, a health check and Basic Auth on every connection, is kept for
  compatibility and is to be phased out. Requires the matching `ws2tcp-local-core`.

### Changed

- **The default authentication is now `"token"`.** A gateway without token authentication
  needs `"auth_mode": "basic"`, or startup fails with
  `WS2TCP_ERROR_KIND_GATEWAY_CHECK_FAILED`. Frontends that embed this library and talk to
  older routers must set it.
- **Without `basic_auth` nothing is sent at startup any more:** authentication is not
  enabled, so the health check is skipped and the proxy starts right away.

## 0.1.8 - 2026-09-19

### Added

- `ws2tcp_last_error_kind` and the `Ws2TcpErrorKind` enum tell what the error
  from `ws2tcp_last_error` is: `WS2TCP_ERROR_KIND_AUTH_FAILED` when the gateway
  rejected the Basic Auth credentials (or needs them and none were given),
  `WS2TCP_ERROR_KIND_GATEWAY_CHECK_FAILED` when the gateway is unreachable, times
  out, or is not a `ws2tcp-router` with the `/` health check, and
  `WS2TCP_ERROR_KIND_OTHER` for anything else.

### Changed

- The proxy now checks the gateway before serving (see `ws2tcp-local-core`
  0.1.9). `ws2tcp_start` still returns right away, so a failed check shows up
  as the proxy stopping: `ws2tcp_status` turns `WS2TCP_STATUS_STOPPED`, and
  `ws2tcp_last_error` / `ws2tcp_last_error_kind` describe the failure. The
  token the gateway returns is sent on every tunnel request next to Basic Auth.
- Requires `ws2tcp-local-core` 0.1.9 and a `ws2tcp-router` with the `/` health
  check (0.1.17 or later).

## 0.1.7 - 2026-09-19

### Changed

- `config_json` accepts an optional `headers` object (e.g.
  `{"User-Agent": "ws2tcp-local-qt/0.3.3"}`) that is sent on the gateway
  websocket handshake, replacing `client_label`. A default
  `User-Agent: ws2tcp-local-ffi/<version>` is sent unless overridden.
- Requires `ws2tcp-local-core` 0.1.8.

## 0.1.6 - 2026-09-17

### Added

- `FfiSettings` accepts an optional `client_label` field in `config_json`,
  identifying the embedding frontend (e.g. `ws2tcp-local-qt/0.3.2`) in the
  `User-Agent` sent with gfwlist HTTP requests. Defaults to
  `ws2tcp-local-ffi/0.1.6` when the caller does not set one.
- Passed through the optional `socks_listen` setting, keeping
  `FfiSettings`/`Settings` in sync with `ws2tcp-local-core`'s local SOCKS5
  (socks5h) listener so FFI callers can enable it via `config_json`.

### Changed

- Updated `ws2tcp-local-core` to 0.1.7.

## 0.1.5 - 2026-09-07

### Removed

- Removed `ws2tcp_init_logging`, the stdout/fmt-based logging entry point.
  It raced with `ws2tcp_set_log_callback`'s callback-based subscriber for
  `tracing`'s process-global dispatcher, and no frontend (the Qt GUI included)
  called it. Use `ws2tcp_set_log_callback` to receive log lines.

### Changed

- Updated `ws2tcp-local-core` to 0.1.5.

## 0.1.4 - 2026-09-05

### Changed

- Updated `ws2tcp-local-core` to 0.1.4. Gfwlist is now downloaded from the
  primary mirror
  `https://wangguofang.net/raw.githubusercontent.com/gfwlist/gfwlist/refs/heads/master/gfwlist.txt`
  first, falling back to GitLab when the primary URL is unreachable.

## 0.1.3 - 2026-08-24

### Changed

- Replaced the JSON `verify_server_certificate` setting with `insecure`.
- Updated `ws2tcp-local-core` to 0.1.3 and enabled TLS server certificate
  verification by default.

## 0.1.2 - 2026-07-14

### Changed

- Updated `ws2tcp-local-core` to 0.1.2 so automatic routing falls back to an
  in-memory gfwlist cache when the platform disk cache is unavailable.

## 0.1.5 - 2026-07-08

### Changed

- Changed auto proxy rule loading from startup-only loading to periodic hot reload.
- Added configurable rule refresh interval with `--rule-refresh-interval-secs` and `rule_refresh_interval_secs`; the default is 60 seconds.
- Kept gfwlist downloads conditional on remote `Last-Modified` changes so unchanged lists continue to use the local cache.
- Added hot reload for custom domain rules using the custom rules file modification time.
- Changed auto mode fallback behavior to route directly when rules are unavailable, while still proxying only hosts matched by loaded rules.
- Replaced active routing rules atomically on successful refresh and kept the previous active rules when refresh fails.
- Updated English and Chinese documentation plus the example TOML configuration for the new rule refresh behavior.
