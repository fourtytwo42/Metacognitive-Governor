# Native accessibility and credential-privacy qualification

This compact, secret-free evidence set proves that installed MCG packages cross each operating system's native accessibility bridge rather than inferring support from browser DOM tests.

## Qualified artifacts

- Windows NSIS: clean product source `01f41b0d5bf1ffec8105b5333038f64b1d419d62`; installer SHA-256 `B3082E88CB0A0720BA77CA1DE4E46FED30A24421912EC776FA4677E9DE7E2329`.
- Ubuntu `.deb`: clean product source `01f41b0d5bf1ffec8105b5333038f64b1d419d62`; SHA-256 `e87b0362f75d9fb223065aea86b67f794bef79215cea7e0816b391fb6f013e3b`.
- Ubuntu AppImage: the same clean product source; SHA-256 `b7f0195a4f0e04a9d236532ffdaf7894c2eadcf83761724cdf73e9a2d2218c21`.
- macOS app prototype: existing clean owner-built package source `27fa6e5346f9d91d1a6f0b62751129c92c8c28aa`. This is an unsigned qualification artifact, not the final notarized release.

The Windows and Linux reports came from their complete installer/package lanes. The macOS report is a focused AX prototype against the previously complete-package-qualified app; the shared verifier is now integrated into the macOS package harness and must pass the next complete matched-candidate lane.

## Required native behavior

Every report proves:

- named application, web document, primary navigation, Settings, and core task/provider controls;
- no unnamed focusable interactive controls;
- native keyboard focus `Goal → Open folder → Create task`;
- a native Simple/Advanced mode action;
- an OS-native protected API-key control; and
- a typed canary that remains absent from accessible names and inaccessible as cleartext through the native value API.

Windows WebView2 is launched with `--force-renderer-accessibility` during the deterministic UI Automation gate. That activates the same renderer accessibility mode normally requested by assistive technology without changing the installed payload. macOS AX qualification fails closed unless its client is explicitly trusted by the disposable VM.

The retained Windows and Linux screenshots were visually reviewed. They contain only the MCG fixture UI plus ordinary disposable-VM chrome; no credential or canary is visible. The identical `.deb`/AppImage reports and screenshots are retained separately because they prove both package families.
