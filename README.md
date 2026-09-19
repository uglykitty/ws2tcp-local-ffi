# ws2tcp-local-ffi

C ABI adapter for `ws2tcp-local-core`.

This crate builds `cdylib`, `staticlib`, and `rlib` artifacts for embedding the
proxy service in native applications such as the Qt GUI.

## Build

```bash
cargo build
```

The public C header is available at
[`include/ws2tcp_local_ffi.h`](include/ws2tcp_local_ffi.h).

## Startup check

`ws2tcp_start` returns as soon as the proxy task is spawned. The task first checks
the gateway, so a wrong password (or an unusable gateway) shows up a moment later
as the proxy stopping. Poll `ws2tcp_status`; once it reports
`WS2TCP_STATUS_STOPPED`, `ws2tcp_last_error` has the message and
`ws2tcp_last_error_kind` says what happened:

```c
if (ws2tcp_status(handle) == WS2TCP_STATUS_STOPPED &&
    ws2tcp_last_error_kind(handle) == WS2TCP_ERROR_KIND_AUTH_FAILED) {
  /* Ask the user to check the credentials. */
}
```

## License

MIT. See [`LICENSE`](LICENSE).
