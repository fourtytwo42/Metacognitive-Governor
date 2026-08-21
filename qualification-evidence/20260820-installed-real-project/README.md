# Installed real-project package qualification

Private harness checkpoint `dafae20f437a4cc574df6c0f6dd0d3a41890103c`
ran the complete Windows and Ubuntu installed-package lanes against the existing
manifest-qualified artifacts. The successful macOS rerun used the same checkpoint
plus only the bounded external-capture timeout correction recorded with this
evidence. The qualified artifacts use Windows/Linux package source
`8f50b99672b38f8e8d8958a15e3d73c15542845a` and macOS package source
`27fa6e5346f9d91d1a6f0b62751129c92c8c28aa`. The
harness-only checkpoint does not change packaged product binaries.

Windows NSIS, Ubuntu `.deb`/AppImage, and macOS app/DMG all passed the new
representative website exercise before encrypted backup and its verification after
restore/daemon restart. Each exercise proves the seeded regression failed, exactly
three governed writes built the site, the generated regression and independent
evaluation passed, all three local references rendered at 1280x720, and exact
replay was side-effect-free without another provider call. Restart reports require
unchanged files/audit history and provider-free replay. Exercise/restart screenshots
are byte-identical within each platform and were visually reviewed.

Ubuntu records `browser_process_sandboxed=false` because that VM's AppArmor policy
denies Playwright's browser user namespace; the loopback-only render uses
`--no-sandbox`. The complete Ubuntu package lane separately passed its mandatory
Bubblewrap/AppArmor product sandbox checks. Windows and macOS record the browser
process sandbox as enabled.

The first macOS package run reached native capture but the former 60-second external
capture deadline expired during privacy review; its data and raw CRD frame were not
retained. The bounded default is now 600 seconds. The rerun passed with a maximized,
privacy-reviewed app capture, and only the website reports/screenshots from that
successful run are retained here. Credential-bearing UAT roots, staged harnesses,
providers, services, listeners, and candidate installs were removed after copying
this compact evidence set. No GitHub-hosted compute ran.
