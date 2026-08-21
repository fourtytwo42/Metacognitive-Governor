# Installed reviewed-topology qualification

The shared `scripts/test-installed-reviewed-topology.py` verifier is bound to
checkpoint `3bcfd829e61d8ff95db44c549554e1f527358d11` and has SHA-256
`b13cf3f3e68b167b2818bdda1bfa2a306bbea5e64270004697efdc67b43b0604`.

Focused exercise/restart runs first passed against the exact current optimized
Windows and Ubuntu release binaries. The verifier was then run from both complete
installed-package harnesses against the already manifest-qualified packages built
from clean source `cad52f2033ddc21c02d838f86003c612ee86e890`:

- Windows NSIS: `dbcce8a421eb8f5909daeacd0a7ed5aeb7e6b409b4b5af6108a897404b4474ec`
- Ubuntu `.deb`: `71b27c89030b66e808850c9304502549cf690f60f07f58178fc4384760b99932`
- Ubuntu AppImage: `adbb4a0aeeed9d9a0f87c585b64d623b0e476a6cf5e383c30bfcb6d6aeb014e5`

Both complete lanes passed. Each initial report proves a three-stage explicit
database/API/web topology, REST/CLI agreement, immutable review replay without
execution, exact durable approval, one MCP-triggered brokered command, one
content-addressed log, observation-only terminal replay, changed-manifest rejection,
and restored settings. Each restart report proves the same plan, log, settings, and
stale-authority rejection across REST, CLI, and MCP after the package harness's
encrypted restore and daemon restart.

Windows additionally passed native UI Automation, 1280x800 desktop capture,
repair/reinstall/final uninstall, and reports 55 MCP tools. Ubuntu passed native
AT-SPI for both the installed `.deb` and AppImage plus repair/reinstall/final removal.
The new report/state privacy scans found no token, bearer header, username, or private
path. Candidate registrations, processes, and loopback listeners were absent after
both lanes. No GitHub-hosted compute ran.
