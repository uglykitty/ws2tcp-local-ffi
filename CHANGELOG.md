# Changelog

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
