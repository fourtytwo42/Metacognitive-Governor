# Installed Provider Success Qualification

Windows and Ubuntu ran their complete installed-package harnesses against the
manifest-bound `8f50b99` packages after adding the shared successful-provider gate.
The fixture is credential-free and loopback-only. It exercises installed CLI human,
JSON, and NDJSON work, requires a passing independent evaluator and successful
governed `process.exec` audit for each task, disconnects the provider, restores the
encrypted database backup, restarts the daemon, and proves all three terminal runs
and their side-effect-free replays remain consistent without another provider call.

Package identities:

- Windows NSIS SHA-256: `F0D0ED6E3D4927B25634C6CBD9005F7443570034E479538AF1AA011597B54668`
- Linux `.deb` SHA-256: `412283A25EB5F27C48DA8DE242772F2D948C4D8AF5085A012397106B629307FB`
- Linux AppImage SHA-256: `56EB0F9AA8324753013B02B74B34ED32E704FC48F60947C11A6699DFC12CD805`

Verifier identities used identically on the VMs:

- `test-installed-provider-success.py`: `9B6E12FB4BF1377F5683BA86445F39035AC87AE3049820115F137AB9E468B02F`
- Windows harness: `844965A8329FF20F27CF5E4BDFCB29F386FD2160A4FC35E8DB0F2BD57649146A`
- Linux harness: `2FFC05228ECB9D890B283F212328E9F13F11CA9ED5321BFAABCC0DB087D2372A`

The Windows and Ubuntu report pairs are byte-identical:

- Exercise report SHA-256: `3DA5115C43FA1A9207B0036049A9B480E55BEA6544650C62331651EBF2CA0345`
- Restart report SHA-256: `AA46AFDF094C89B6C17ECC3D721BA7EA924AC9189AE43EA65E593AEA12016F3D`

Both harnesses passed install, diagnostics, encrypted backup/restore, restart,
consumer, MCP/plugin, repair/reinstall retention, and final package removal. Final
audits found zero candidate processes or qualification listeners. Credential-bearing
UAT roots and staging directories were removed after these four secret-free reports
were copied into the repository. No GitHub-hosted compute ran.
