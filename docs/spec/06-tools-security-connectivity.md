# Tools, Security, Plugins, and Connectivity

> Canonical split specification. Generated from the original PRD by
> `scripts/split-prd.ps1`; edit the split documents directly after the initial
> generation. The archived original remains available for provenance.

## 7. Owner Constitution

The owner constitution is a policy layer above MCG.

Examples:

```text
Never expose stored secrets to model context.

Never modify protected system paths without approval.

Never delete backups.

Production deployment requires approval.

Maximum daily API spend is $X.

MCG cannot disable audit logging.

MCG cannot modify or weaken constitutional policy.
```

MCG may change lower-level policies but may not change this layer.

---

---

## 26. Tool Runtime

Tools should be typed and first-class.

Initial primitives:

```text
fs.read
fs.write
fs.patch
fs.list
fs.mkdir
fs.move
fs.copy
fs.delete

process.exec
process.spawn
process.status
process.stdin
process.kill

shell.exec

http.request
http.download

system.info
system.env
system.processes

git.status
git.diff
git.commit
git.checkout

scheduler.create
scheduler.remove
scheduler.list

memory.search
memory.read
memory.write
```

### 26.1 Structured Process Execution

Prefer:

```text
executable = "git"
argv = ["status", "--short"]
cwd = "/repo"
```

over blindly executing shell strings.

An explicit shell tool remains available for cases where shell semantics are required.

---

---

## 27. Capability and Permission System

Every agent receives explicit capabilities.

Example:

```text
Coder:
  filesystem:
    read:
      - /projects/**
    write:
      - /projects/**
    deny:
      - ~/.ssh/**
      - ~/.config/mcg/secrets/**

  process:
    allow:
      - git
      - cargo
      - npm
      - node
      - python

  destructive:
    require_approval: true
```

Permissions are enforced by the harness, not by model prompt compliance.

---

---

## 28. Browser Use

Browser sessions are persistent resources rather than one-off calls.

A browser session should track:

- Profile.
- Owner.
- Current task.
- Tabs.
- URLs.
- Authentication state references.
- Last activity.
- Session status.

Agents may:

- Acquire.
- Operate.
- Release.
- Hand off.

---

---

## 29. Computer Use

Computer-use sessions should represent a device or desktop environment.

Possible capability fields:

```text
screen.read
mouse
keyboard
clipboard
window.list
process.inspect
```

Computer-use capabilities are governed by the same policy engine as shell/filesystem tools.

---

---

## 30. Multi-Host Operation

Primary multi-host capability should come from the tool runtime's ability to use SSH and remote execution.

MCG may:

- SSH into another host.
- Inspect system capabilities.
- Execute commands.
- Transfer files.
- Run remote agents/tasks.
- Route work to machines with needed capabilities.

Example use cases:

- Build on macOS from a Linux controller.
- Use a GPU server for local inference.
- Test on Windows.
- Interact with a remote Linux server.

A separate distributed-agent protocol may be added later if SSH-based operation becomes insufficient.

---

---

## 34. Plugin Architecture

Support two plugin classes.

### Trusted Native Plugins

- Rust/native.
- Highest performance.
- Direct integration.

### Sandboxed Plugins

- WASM/WASI.
- Capability-restricted.
- Suitable for third-party/community plugins.

---

---

## 49. Remote Connectivity

GUI and CLI should be able to connect to a local or remote MCG daemon.

Examples:

```bash
mcg --host localhost
mcg --host devbox
mcg --host https://mcg.example
```

This allows a local workstation to control an MCG runtime on a GPU server or remote machine.

---

---

## 50. Security

Security should be enforced structurally.

Core requirements:

- Typed tool calls.
- Explicit capabilities.
- Owner constitution.
- Secret isolation.
- Audit log.
- Protected paths.
- Approval gates.
- Sandboxed third-party plugins.
- Versioned config.
- Reversible organizational changes.
- Optional remote-auth layer.
- Encrypted sensitive backup data.

---
