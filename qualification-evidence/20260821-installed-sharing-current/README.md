# Exact-current installed sharing qualification

Both owner-controlled package lanes use clean release packages built from source commit
`cad52f2033ddc21c02d838f86003c612ee86e890`. The installed signed-organization-sharing
verifier is part of that commit's package harness and runs before backup plus after the
restored daemon restart.

- Windows NSIS SHA-256: `dbcce8a421eb8f5909daeacd0a7ed5aeb7e6b409b4b5af6108a897404b4474ec`
- Ubuntu `.deb` SHA-256: `71b27c89030b66e808850c9304502549cf690f60f07f58178fc4384760b99932`
- Ubuntu AppImage SHA-256: `adbb4a0aeeed9d9a0f87c585b64d623b0e476a6cf5e383c30bfcb6d6aeb014e5`

Ubuntu passed its complete package lane directly. Windows UAT exposed that the UIA
keyboard verifier's WinForms Tab synthesis did not enter WebView2 from an interactive
scheduled qualification process. Commits `a28cb6d450a23537b531464079f05acb03a85a31`,
`ddac7ccc96c9655db15b80222f8d58cd45abe808`, and
`415d51277967f512ccace072e53b4b3fb257c89e` make focus waits independent, send real
Win32 keyboard input, and correct the 64-bit `INPUT` union layout. The unchanged
manifest-qualified package then passed the complete lane with real
`Goal → Open folder → Create task` UIA focus evidence.

The retained screenshots were visually reviewed and contain no unrelated credential
content. A focused credential-pattern scan of every retained text artifact passed.
All candidate installations, registrations, UAT roots, scheduled tasks, processes,
and loopback listeners were removed after evidence capture. GitHub-hosted compute was
not used.
