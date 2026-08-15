MCG Windows portable qualification bundle
==========================================

Double-click Start-MCG.cmd to start the local daemon and desktop application.

The folder also includes:

  mcg.exe          Command-line interface
  mcg-daemon.exe   Local daemon/headless service
  mcg-desktop.exe  Native desktop application
  mcg-shadow-runner.exe  Internal disposable-sandbox command helper

Automatic shadow build/test/static-analysis verification additionally requires
Windows Sandbox with its `wsb.exe` command-line interface (Windows 11 24H2 or
later on a supported Pro, Enterprise, or Education edition). MCG uses a
networkless disposable VM and read-only host mappings. If that capability or
the adjacent helper is missing, MCG runs no fallback host command and reports
the shadow objective as inconclusive.

This unsigned portable bundle is for owner-controlled release qualification.
The production installer, signing, upgrade, repair, and uninstall gates remain
separate release requirements.
