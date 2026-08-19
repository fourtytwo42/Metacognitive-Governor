# Third-Party Notices

## Tauri updater

The native desktop uses `tauri-plugin-updater` 2.10.1 and its
`minisign-verify` dependency to download and verify owner-signed application
updates before installation.

- Upstream: <https://github.com/tauri-apps/plugins-workspace>
- License: Apache License 2.0 OR MIT; MCG distributes it under the Apache-2.0 option.
- Local boundary: update endpoints and public keys are compile-time release inputs;
  unsigned builds fail closed, downgrades remain disabled, and installation requires
  a verified portable backup plus clean daemon shutdown.

The complete Apache License 2.0 text is shipped in MCG release packages.

## yaml-rust2

`crates/mcg-projects` uses `yaml-rust2` 0.11.1 to parse bounded Docker Compose
and OpenAPI manifests into the provenance-bearing project world model.

- Upstream: <https://github.com/Ethiraric/yaml-rust2>
- License: MIT OR Apache License 2.0; MCG distributes it under the Apache-2.0 option.
- Local boundary: Inputs are capped at 2 MiB, documents/services/items and the
  resulting graph are bounded, and credential-bearing server URLs are discarded.

The complete Apache License 2.0 text is shipped in MCG release packages.

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
Apache 2.0 terms, is shipped as
`licenses/Wasmtime-Apache-2.0-WITH-LLVM-exception.txt` in every package that
contains the plugin runtime.

## Bubblewrap

The Linux `.deb` package contains Bubblewrap 0.11.2 as the namespace boundary
for automatic shadow command and service verification.

- Upstream: <https://github.com/containers/bubblewrap>
- Reviewed commit: `1b80120ef26a28e065e67f89bfef873f13bdd317`
- Copyright: Alexander Larsson and Bubblewrap contributors
- License: GNU General Public License version 2 or later
- Local configuration: non-setuid build with SELinux support; MCG supplies a
  path-specific AppArmor profile that strips capabilities from sandbox children.

The `.deb` ships the upstream `COPYING` file and the complete corresponding
source archive under `/usr/lib/MCG`. The AppImage does not embed Bubblewrap or
install privileged host policy.

The path-specific `metacognitive-governor-bwrap` policy is adapted from the
AppArmor project's `bwrap-userns-restrict` extra profile.

- Upstream: <https://gitlab.com/apparmor/apparmor>
- License: GNU General Public License version 2

## ed25519-dalek and curve25519-dalek

`crates/mcg-plugin-trust` uses `ed25519-dalek` 3.0.0 and its
`curve25519-dalek` 5.0.0 dependency to verify detached plugin package signatures.

- Upstream: <https://github.com/dalek-cryptography/curve25519-dalek>
- License: BSD 3-Clause

Copyright (c) 2017-2019 isis agora lovecruft. All rights reserved.

Copyright (c) 2016-2021 isis agora lovecruft. All rights reserved.
Copyright (c) 2016-2021 Henry de Valence. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its contributors
   may be used to endorse or promote products derived from this software without
   specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR
ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
(INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON
ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
