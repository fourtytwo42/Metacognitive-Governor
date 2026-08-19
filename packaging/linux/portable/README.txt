MCG Linux portable qualification bundle
========================================

Run ./Start-MCG.sh to start the local daemon and desktop application.

The directory also includes:

  mcg           Command-line interface
  mcg-daemon    Local daemon/headless service
  mcg-desktop   Native desktop application
  mcg-updater   Packaged updater supervisor (inactive on Linux)

The desktop requires the GTK/WebKit libraries listed in the release
documentation. This unsigned portable bundle is for owner-controlled release
qualification. Production packages, signing, desktop integration, service
registration, upgrade, repair, and uninstall remain separate release gates.

Automatic shadow build/test/static-analysis/service verification additionally
requires a protected, root-owned, non-setuid Bubblewrap 0.11.2 or later and
/usr/bin/prlimit. The production `.deb` installs the reviewed Bubblewrap build
and a capability-stripping AppArmor profile without weakening global policy.
This portable bundle and the AppImage cannot install privileged host policy; if
a compatible administrator-installed backend is absent or cannot enter its
namespaces, MCG runs no fallback host command and reports the shadow condition
as inconclusive. Do not disable AppArmor or user-namespace restrictions.
