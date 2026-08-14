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
