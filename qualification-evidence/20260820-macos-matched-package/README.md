# Matched macOS package qualification

- Platform: macOS 26.0, x86_64, owner-controlled disposable VM.
- Source commit: `27fa6e5346f9d91d1a6f0b62751129c92c8c28aa`, clean and manifest-bound.
- DMG: 40,397,035 bytes, SHA-256 `771F5078B3C10AF51AC4B0C478BDD2203CF10709C43E24CBBB4A0A6D37EAF877`.
- Release manifest SHA-256: `8AD878C8021C9469ADA0B7662930B0B7FC9BAD834D30C5C96A8B41CD7237807E`.
- Package harness exit: `0`; log SHA-256 `849134B3D106482E29A0EF967FAA4C1BC43DD0ED6E3E6001B109F55DB2A0D04A`.
- Owner-built package signature state: unsigned. This is qualification evidence, not a production release or a substitute for Developer ID signing/notarization.

The installed harness passed DMG verification, payload validation, plugin execution, repair, daemon health and restart, CLI/doctor, provider-backed useful work, independent evaluation, REST/OpenAI-compatible/MCP success, cross-project memory transfer, encrypted recovery, native desktop launch, data retention, reinstall, and final removal. The `Terminated: 15` line in the log is the harness's expected bounded shutdown of the packaged desktop after capture; the harness itself exited successfully.

The harness's native desktop screenshot was taken from the visible Chrome Remote Desktop console through its external-capture contract because macOS does not grant an SSH login session screen-capture authority. That raw frame also included an unrelated terminal behind the app, so it was deliberately excluded from retained evidence during privacy review. `desktop.png` is a clean maximized recapture from the same exact packaged application and source, with SHA-256 `6A144D5FBCFD311FAEF9FEB21961F2F2771F0C46D180F18EBF7DD6BF1CBEF411`.

Agent-operated native Improvement UAT then created a fresh promoted experiment through the installed REST/CLI/MCP contract, restarted the packaged desktop against the same durable store, loaded the promoted organization, and inspected both modes. `native-experiments.json` and `native-experiments-restart.json` are byte-identical with SHA-256 `3E0314F160E04D8A72843D5399E3C54CC4A46E7792396F8CCDEDE3B06B1B7201`. The simple view visibly reports one observation, one experiment, one promotion, the independent benchmark/decision counts, and baseline protection; SHA-256 `6452DF7AEB2BCBD442B571B828EF407254FDEDA3C0462D6BC6234FFDAFCCC38F`. The advanced view additionally exposes immutable experiment/candidate IDs and the measured-action controls; SHA-256 `BA884911593406D78BE247F7033BB309948EA683C5D831390225D8F5117A5482`.

Cross-platform deterministic report hashes match the already-qualified Windows and Ubuntu package lanes byte for byte:

- Provider exercise: `3DA5115C43FA1A9207B0036049A9B480E55BEA6544650C62331651EBF2CA0345`
- Provider restart: `AA46AFDF094C89B6C17ECC3D721BA7EA924AC9189AE43EA65E593AEA12016F3D`
- Memory transfer exercise: `D3377AE4E246BC065C365287664982BBCC6F78CF9795C0DBFA077D62E29D9649`
- Memory transfer restart: `5B928FDD1D0CC8EA5D7AA6DE8E2809BCCD381D75376210ECEA3ECFC90F3292F2`
- Interface exercise: `AE5C5E375250388FFAA8A177DF7A9248A37BBE5BA710251F2AA5739C1B0E7E7B`
- Interface restart: `A7527CBFC24668B17D541E4D56B06C21A7F15A902EE74B364C48F2B697E56982`

The retained text files were scanned for the VM password, authorization headers, API keys, access/refresh tokens, client secrets, private keys, and the private fixture secret; no matches were found. The retained image was visually reviewed and contains only the maximized MCG app. Final package-harness audits found no candidate MCG process or listeners on ports 17779/7777, no installed UAT application, and no remaining credential-bearing package-UAT root.
