# Native accessibility and credential-privacy qualification

This compact, secret-free evidence set proves that installed MCG packages cross each operating system's native accessibility bridge rather than inferring support from browser DOM tests.

## Qualified artifacts

- Windows NSIS: clean product source `01f41b0d5bf1ffec8105b5333038f64b1d419d62`; installer SHA-256 `B3082E88CB0A0720BA77CA1DE4E46FED30A24421912EC776FA4677E9DE7E2329`.
- Ubuntu `.deb`: clean product source `01f41b0d5bf1ffec8105b5333038f64b1d419d62`; SHA-256 `e87b0362f75d9fb223065aea86b67f794bef79215cea7e0816b391fb6f013e3b`.
- Ubuntu AppImage: the same clean product source; SHA-256 `b7f0195a4f0e04a9d236532ffdaf7894c2eadcf83761724cdf73e9a2d2218c21`.
- macOS app/DMG: clean product source `c1881f221978a4409575096e06feed6a393b89bd`; DMG SHA-256 `a70ffcb2761d8585e72a7a301fad729b180806586a9656cfdfcf20d87ec13cc4`; release-manifest SHA-256 `2a883368e1c90be7e7c791387527188f70b64356bff643d8cef3f77a8e7fe151`. This is an unsigned owner-built qualification artifact, not the final notarized release.

The Windows, Linux, and macOS reports all came from complete installer/package lanes. The matched macOS lane also passed DMG verification, install/repair/reinstall/uninstall data retention, packaged sidecars, CLI and MCP, daemon restart, provider-backed replay, and the three representative website/API/full-stack projects. Its atomic harness exit record was `0` and no candidate process or listener remained after cleanup.

## Required native behavior

Every report proves:

- named application, web document, primary navigation, Settings, and core task/provider controls;
- no unnamed focusable interactive controls;
- native keyboard focus `Goal → Open folder → Create task`;
- a native Simple/Advanced mode action;
- an OS-native protected API-key control; and
- a typed canary that remains absent from accessible names and inaccessible as cleartext through the native value API.

Windows WebView2 is launched with `--force-renderer-accessibility` during the deterministic UI Automation gate. That activates the same renderer accessibility mode normally requested by assistive technology without changing the installed payload. macOS AX qualification fails closed unless its client is explicitly trusted by the disposable VM.

All retained screenshots were visually reviewed and contain no credential or canary. The macOS capture is PID-bound to the exact MCG window, excluding unrelated desktop windows and terminal scrollback. The identical `.deb`/AppImage reports and screenshots are retained separately because they prove both package families.
