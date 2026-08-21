# Installed cross-project memory transfer qualification

Exact harness commit `4ec61c45e783de9b924b7bc1b8d55e728a1252c2` ran the complete
non-GUI package lanes against the manifest-bound `8f50b99` Windows and Linux
packages on owner-controlled VMs. GitHub-hosted compute was not used.

Both platforms produced byte-identical secret-free verifier reports:

- exercise: `D3377AE4E246BC065C365287664982BBCC6F78CF9795C0DBFA077D62E29D9649`
- encrypted-restore/restart: `5B928FDD1D0CC8EA5D7AA6DE8E2809BCCD381D75376210ECEA3ECFC90F3292F2`

The reports prove an explicit digest-reviewed durable claim transfer, one target
claim, immutable inbound/outbound history, CLI-to-MCP exact replay without another
event, changed-replay conflict, encrypted restore, and daemon restart. The package
logs and atomic completion summaries bind the complete lanes; all temporary UAT
roots and bundles were removed after zero-process/listener/package audits.
