MCG Linux portable qualification bundle
========================================

Run ./Start-MCG.sh to start the local daemon and desktop application.

The directory also includes:

  mcg           Command-line interface
  mcg-daemon    Local daemon/headless service
  mcg-desktop   Native desktop application

The desktop requires the GTK/WebKit libraries listed in the release
documentation. This unsigned portable bundle is for owner-controlled release
qualification. Production packages, signing, desktop integration, service
registration, upgrade, repair, and uninstall remain separate release gates.

Automatic shadow build/test/static-analysis verification additionally requires
a distro-managed, root-owned, non-setuid bubblewrap 0.11.2 or later and
/usr/bin/prlimit. If this secure capability is absent or cannot
enter its namespaces, MCG runs no fallback host command and reports the shadow
condition as inconclusive. Do not weaken global AppArmor or user-namespace
security settings to enable it.
