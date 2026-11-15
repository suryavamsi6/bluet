# vendored `xattr` 0.2.3

`xattr` 0.2.3 references `libc::ENOATTR`, which recent `libc` releases no longer
export on Linux (the correct constant there is `ENODATA`). The upstream crate has
not been patched, so the dependency chain
`gpui → gpui_http_client → zed-async-tar → xattr` fails to compile against a
current `libc`:

```
error[E0425]: cannot find value `ENOATTR` in crate `::libc`
   --> xattr-0.2.3/src/sys/mod.rs:12:56
```

`Cargo.toml` points the crate at this directory with a `[patch.crates-io]`
entry. The only change from the published 0.2.3 sources is in
`src/sys/mod.rs`, where `ENOATTR` is aliased to `ENODATA` on Linux and Android:

```rust
#[cfg(any(target_os = "linux", target_os = "android"))]
pub const ENOATTR: ::libc::c_int = ::libc::ENODATA;

#[cfg(any($(target_os = $platform),*))]
#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub const ENOATTR: ::libc::c_int = ::libc::ENOATTR;
```

Every other file is byte-identical to the crates.io release. Drop this directory
and the `[patch.crates-io]` block once upstream ships a fix.
