# Third-Party Notices

## OpenAI Codex login protocol

Parts of `crates/mcg-providers/src/oauth_device.rs` are adapted from the
device-authorization implementation in the OpenAI Codex repository.

- Upstream: <https://github.com/openai/codex>
- Upstream file: `codex-rs/login/src/device_code_auth.rs`
- Copyright: 2025 OpenAI
- License: Apache License 2.0
- Local changes: Reworked into MCG's standalone provider-auth boundary; removed
  Codex runtime/configuration/credential-file dependencies; added redacted token
  types, issuer validation, injected test server support, and MCG errors.

The upstream Apache License 2.0 text is available at
<https://github.com/openai/codex/blob/main/LICENSE>. A distributable MCG package
must include that license text alongside this notice; release packaging tracks
this as an explicit gate until the repository's final licensing layout is chosen.

## Bytecode Alliance Wasmtime

`crates/mcg-plugins` embeds Wasmtime and its WASI implementation for bounded,
capability-restricted third-party plugins.

- Upstream: <https://github.com/bytecodealliance/wasmtime>
- Copyright: Wasmtime Project Developers
- License: Apache License 2.0 with LLVM Exceptions
- Local configuration: Rust-1.93-compatible release 45.0.3 with only the
  Cranelift, runtime, standard-library, WAT, and WASIp1 features required by the
  MCG plugin boundary.

The complete upstream license, including the LLVM exceptions appended to the
Apache 2.0 terms, must be included before the plugin runtime enters a packaged
MCG binary. Packaging integration remains an explicit plugin-milestone gate.
