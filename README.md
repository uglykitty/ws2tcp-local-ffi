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

## License

MIT. See [`LICENSE`](LICENSE).
