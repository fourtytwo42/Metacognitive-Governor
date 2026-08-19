# Exact v27 Windows Signed-Updater Qualification

This directory is reserved for secret-free evidence from the owner-controlled
Win11-VM. A green helper test, successful download, or installer launch is not a
package qualification pass by itself.

## Fixed source boundary

- Source archive SHA-256: `AB6E4AF14F9345F3F163CBEBE18ADCBCE21914170B48267DA25255F62B4CF06B`
- Baseline: `0.1.0`, prepared commit `690574417def4f28023468c8af7959edc26a367b`
- Candidate: `0.1.1-uat.1`, prepared commit `d6438cbe66cf775001aaff61b28f7d0b4cd31055`
- Feed: `https://127.0.0.1:18443/latest.json`
- Updater public-key SHA-256: `ABE1461E941707BCCFBAB71E3B46DC5E50C3591F226C6E0F1A1C82AC778D4FB8`

Package hashes, sizes, signatures, and release-manifest identities must be added
only after both builds finish and every manifest entry is independently rehashed.

## Required scenario evidence

Each lane must record its selected scenario, visible desktop result, baseline or
candidate executable/package hash, installation registration, authenticated
service health, backup inventory and verification, process inventory, port owner,
updater temporary-file inventory, source cleanliness, and free disk space.

The matrix must prove:

1. No-update result without download or state mutation.
2. Corrupt-manifest rejection.
3. Unavailable-feed rejection.
4. Bad-signature rejection after download.
5. Version-change-between-review-and-install rejection.
6. Backup failure before shutdown or installation.
7. Shutdown timeout with a foreign loopback claimant, visible failure, baseline
   preservation, zero orphan process, and authenticated recovery after release.
8. Real installer failure after verified backup/shutdown, visible failure,
   baseline preservation or supported rollback, and authenticated recovery.
9. Valid signed update, verified backup, clean shutdown, candidate installation,
   restart/relaunch, data/schema migration, and retained provider-free state.
10. Supported rollback behavior with a verified backup and no partial package,
    temporary updater file, stale registration, or zombie process.

## Disposable CA boundary

Do not add trust until the owner gives the pending action-time confirmation. When
authorized, record the public CA DER SHA-256 and the exact newly imported
`Cert:\CurrentUser\Root` thumbprint. Before teardown, re-match both identities;
remove only that thumbprint, prove it is absent, and require a fresh HTTPS request
to fail certificate validation. Never export or retain the CA private key.

## Privacy and integrity

- Never copy access tokens, installation bearer tokens, credentials, private keys,
  full private descriptors, request headers, query strings, prompt/output content,
  or user data into evidence.
- Hash every retained payload and list it in `SHA256SUMS.txt`.
- The final `qualification-summary.json` must name every lane as `pass`, `fail`, or
  `not_run`; absence is never a pass.
- No GitHub-hosted build, test, or paid automation may be used.
