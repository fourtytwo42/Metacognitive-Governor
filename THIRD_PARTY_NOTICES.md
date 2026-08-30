# Third-Party Notices

## Lucide

The MetaCog desktop uses lucide-react 1.34.0 for its application chrome,
navigation, workspace panes, browser, and terminal iconography.

- Upstream: <https://github.com/lucide-icons/lucide>
- License: ISC, with the listed Feather-derived icons available under MIT.

ISC License

Copyright (c) 2026 Lucide Icons and Contributors

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

The Lucide icons used by MetaCog that are derived from Feather are available
under the MIT License.

Copyright (c) 2013-present Cole Bemis

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

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

## process-wrap

The governed action parent uses `process-wrap` 10.0.0 to place each action runner and its
descendants in a POSIX process group or Windows Job Object before execution begins.

- Upstream: <https://github.com/watchexec/process-wrap>
- License: Apache License 2.0 OR MIT; MCG distributes it under the Apache-2.0 option.
- Local boundary: the child environment and I/O remain bounded, deadline termination targets
  the whole process tree, and the parent still waits/reaps before classifying an uncertain effect.

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
