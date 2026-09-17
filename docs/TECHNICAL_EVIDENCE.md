# Technical Evidence: EATMCP — mcp-terminal-commander

**Why AI agents using MCP terminal servers are operationally superior to raw shell access**

---

## 1. The Core Problem: Agents Running Commands Blind

When an AI agent has access only to a raw shell tool (`bash`, `pwsh`, `cmd`), it operates under a structural constraint: it must guess what environment it is in before executing any command. This guessing generates a predictable class of failures.

### 1.1 Dialect Blindness: Commands That Fail Silently

A common agent failure pattern is emitting a command that is syntactically correct in one environment but invalid in another.

**Without EATMCP** — agent sends `ls -la` on a Windows host:

```
'ls' is not recognized as an internal or external command,
operable program or batch file.
```

The agent receives this as an unstructured error string. It must now re-parse free-text stderr to diagnose what happened, re-reason about the environment, and try again — consuming additional tokens and round-trips.

**With EATMCP** — `command_execute` returns structured JSON including the diagnosis:

```json
{
  "exit_code": 1,
  "stderr": "'ls' não é reconhecido...",
  "error_class": "COMMAND_NOT_FOUND",
  "likely_cause": "'ls' is a POSIX/Linux command but environment is Windows.",
  "suggested_next_action": "On Windows, use 'dir' or run via PowerShell with use_shell: true."
}
```

The agent receives a machine-readable `error_class`, `likely_cause`, and `suggested_next_action` without needing to interpret free-text error messages. Recovery logic is deterministic, not probabilistic.

---

### 1.2 The Resolver: What Actually Runs vs. What the Agent Typed

Every command execution passes through a multi-stage resolver. This is the operational core of EATMCP.

```
Input: "npm"
Resolution path tried:
  1. CMD builtins? No.
  2. PowerShell cmdlets? No.
  3. POSIX commands? No.
  4. Env variables? No (CUSTOM_APP_PATH not set).
  5. PATH + PATHEXT search → found: C:\Program Files\nodejs\npm.cmd
  6. Dialect: WindowsBatch → requires cmd.exe /c wrapper
  7. Shell compatibility: cmd=true, powershell=false, bash=false
Output: cmd.exe /c "C:\Program Files\nodejs\npm.cmd"
```

The agent does not need to know Windows `.cmd` files require `cmd.exe /c`. It does not need to know about PATHEXT. It does not need to quote paths with spaces. The resolver handles all of this.

**Evidence from the test suite (`execution_tests.rs`):**

```rust
// Auto-resolves npm to its .cmd wrapper without agent involvement
let res = resolve_command("npm", "auto");
assert_eq!(res.recommended_shell, "cmd");
assert_eq!(res.args_prefix[0], "/c");
assert!(res.args_prefix[1].to_lowercase().ends_with("npm.cmd"));
```

```rust
// Telnet (System32 tool) correctly marked as cmd-only, not bash-compatible
let res = resolve_command("telnet", "auto");
assert!(res.resolved_executable.unwrap().to_lowercase().contains("system32"));
assert_eq!(res.recommended_shell, "cmd");
assert!(!res.shell_compatibility.bash);
```

These invariants are verified automatically. They do not depend on the agent knowing Windows internals.

---

### 1.3 Path Normalization: Eliminating a Class of Windows Failures

Windows paths with spaces are the single most common cause of command injection and argument parsing failures when agents construct shell strings manually.

**Without EATMCP** — agent constructs:

```
dir C:\Program Files\nodejs
```

This splits into `dir C:\Program` and `Files\nodejs` — two arguments. The command fails or lists the wrong directory.

**With EATMCP** — `command_execute` receives `args` as a structured JSON array:

```json
{
  "command": "dir",
  "args": ["C:\\Program Files"]
}
```

The resolver normalizes quoting deterministically:

```rust
// From execution_tests.rs:
assert_eq!(
    normalize_quoted_command(r#"C:\Program Files\nodejs\npm.cmd"#),
    r#""C:\Program Files\nodejs\npm.cmd""#
);
assert_eq!(
    normalize_quoted_arg(r#"C:\Program Files\My Project"#),
    r#""C:\Program Files\My Project""#
);
```

The test also validates the inverse — stripping spurious internal quotes before re-quoting:

```rust
assert_eq!(
    clean_unquoted_path(r#"C:\"Program Files"\nodejs\npm.cmd"#),
    r#"C:\Program Files\nodejs\npm.cmd"#
);
```

Agents using raw shell access must implement this normalization manually or embed paths inside string interpolation — both of which are error-prone.

---

## 2. Safety Enforcement: What an Agent Cannot Accidentally Do

Raw shell access has no inherent safety layer. Any command the agent constructs is executed verbatim. EATMCP enforces a pre-execution safety classification on every call to `command_execute`.

### 2.1 CRITICAL — Hard Deny, Never Executed

The following patterns are blocked before any process is spawned:

| Pattern | Risk |
|---|---|
| `rm -rf /` | Full filesystem wipe on Linux |
| `del /s /q C:\` | Full filesystem wipe on Windows |
| `rd /s /q C:\` | Full directory tree removal |
| `format.com C:` | Disk format |
| `:(){ :\|:& };:` | Fork bomb |
| `mkfs`, `fdisk`, `parted` | Disk partitioning |
| `mimikatz`, `Invoke-Mimikatz` | Credential harvesting |
| Access to `/etc/shadow` | Shadow password file |

**Live evidence from this session:**

```json
// command_validate("rm", ["-rf", "/"])
{
  "risk_level": "CRITICAL",
  "action": "DENY",
  "reason": "Destructive command or credential access detected matching 'rm -rf /'"
}
```

```json
// command_validate("del", ["/s", "/q", "C:\\"])
{
  "risk_level": "CRITICAL",
  "action": "DENY",
  "reason": "Destructive command or credential access detected matching 'del /s /q c:\\'"
}
```

These calls return a denial **before any process is spawned**. The shell is never invoked.

### 2.2 HIGH — Requires Explicit Confirmation

Commands with significant side effects — force-kill processes, recursive directory deletion, firewall rule flush — are classified `HIGH` and return `action: CONFIRM`.

```json
// command_validate("taskkill", ["/f", "/im", "notepad.exe"])
{
  "risk_level": "HIGH",
  "action": "CONFIRM",
  "reason": "High-impact mutation detected matching 'taskkill /f'",
  "resolved_executable": "C:\\WINDOWS\\system32\\taskkill.exe"
}
```

The harness can implement a human-in-the-loop gate at this boundary. EATMCP provides the signal; the harness decides the policy.

### 2.3 Four-Level Risk Taxonomy

```
CRITICAL  → PolicyAction::Deny       — process never spawned
HIGH      → PolicyAction::Confirm    — harness must confirm
MEDIUM    → PolicyAction::Allow      — logged, executed (package managers, docker, git push)
LOW       → PolicyAction::Allow      — read-only inspection commands
```

Every execution record in the log (`/dev/log` endpoint) includes the risk level and policy action that was applied. This produces an auditable execution trail — something that raw shell access cannot provide.

---

## 3. Environment Fingerprinting: Inspect Before Executing

### 3.1 What `environment_inspect` Returns

A single tool call returns the complete host fingerprint with zero guessing:

```json
// Live result from this session:
{
  "platform": "windows",
  "distribution": "Windows 11",
  "architecture": "x86_64",
  "shell": "powershell",
  "available_shells": ["powershell", "cmd", "bash"],
  "package_manager": "winget",
  "available_package_managers": ["winget"],
  "init_system": "windows-services",
  "user": "robml",
  "is_admin": false,
  "is_container": false,
  "docker_available": true,
  "hostname": "ROBSON-PC"
}
```

This fingerprint is cached per-process. Subsequent calls return `cache_age_ms` instead of re-running detection, reducing latency.

### 3.2 What the Fingerprint Prevents

Without environment inspection, an agent on a Termux host (Android) would attempt:
- `systemctl restart sshd` → fails (Termux uses `sv`, not systemd)
- `apt install redis` → fails (Termux uses `pkg`)
- `/etc/os-release` → the file exists but is structurally different

EATMCP detects Termux explicitly:

```rust
// From environment/detector.rs:
if Path::new("/data/data/com.termux").exists()
    || std::env::var("TERMUX_VERSION").is_ok()
{
    return ("Termux (Android)".to_string(), "Android".to_string(), kernel);
}
```

The same detection logic runs for WSL, Docker containers, systemd-based Linux, OpenRC, and Windows. A single `environment_inspect` call makes the agent's subsequent command selection deterministic.

---

## 4. Post-Execution Validation: Proof, Not Assumption

### 4.1 `service_restart` Is Not Just a Command — It Validates

When an agent calls `service_restart("wuauserv")`, EATMCP:

1. Runs `sc.exe stop wuauserv`
2. Waits 500ms for the service to reach stopped state
3. Runs `sc.exe start wuauserv`
4. Calls `validate_service_status("wuauserv")` to confirm active state
5. Returns a structured result including the post-restart validation

Without EATMCP, an agent running `sc stop wuauserv && sc start wuauserv` must then separately run a status check and parse its output to confirm success. That is three round-trips. EATMCP collapses them to one.

**Live evidence from this session:**

```json
// service_status("wuauserv")
{
  "service": "wuauserv",
  "status": "SUCCESS",
  "message": "Service 'wuauserv' is RUNNING",
  "details": { "state": "RUNNING" }
}
```

### 4.2 `network_port_check` — Binary Signal, No Parsing Required

Checking whether a server started successfully via raw shell:

```bash
netstat -an | grep :8080 | grep LISTEN
```

The agent receives multi-line text, must parse it, handle platform differences between `netstat` output on Linux vs. Windows vs. macOS, and distinguish LISTEN from ESTABLISHED.

Via EATMCP:

```json
// network_port_check(port=8080)
{
  "status": "SUCCESS",
  "message": "Port 8080 is open and listening",
  "open": true
}
```

Binary result. No parsing. No platform variance.

---

## 5. The Skill Layer: Directing Agent Behavior at Zero Extra Cost

### 5.1 What the Skill Does

The `eatmcp` skill (`skills/eatmcp/SKILL.md`) is a 41-line instruction file that changes agent behavior before any tool call. It establishes four operational principles:

1. **Inspect before executing** — `environment_inspect` before any environment-dependent command
2. **Validate before mutating** — `command_validate` for destructive or administrative operations
3. **Prefer direct process spawning** — avoids shell injection surface and escape complexity
4. **Validate post-execution** — confirm service state, port availability, file existence after changes

Without the skill, an agent defaults to whatever behavior the harness implies — typically raw shell with no pre-inspection. The skill costs zero additional tokens at inference time because it is loaded into the system prompt at session start, not repeatedly injected.

### 5.2 Operational Flow Comparison

**Without the skill** (agent uses raw `pwsh` or `bash`):

```
1. Agent decides what to do
2. Agent constructs a shell string
3. Shell executes the string
4. Agent receives stdout/stderr as raw text
5. Agent interprets the text to determine success/failure
6. If failed, agent re-reasons about environment and retries
```

**With the skill + EATMCP**:

```
1. Agent calls environment_inspect → receives structured fingerprint
2. Agent decides what to do (with confirmed environment context)
3. Agent calls command_validate → receives risk level and resolution metadata
4. Agent calls command_execute → receives structured JSON with exit_code,
   dialect, error_class, likely_cause, suggested_next_action
5. Agent calls service_status / network_port_check / filesystem_exists
   to confirm post-condition
```

Steps 4 and 5 in the second flow return machine-readable structured output at every stage. The agent never interprets free-text error messages.

---

## 6. Execution Record and Audit Trail

Every tool call is recorded in the application state (`AppState`) with:

```rust
pub struct ExecutionRecord {
    pub timestamp_nanos: u128,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub risk_level: RiskLevel,
    pub policy_action: PolicyAction,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub validation_status: ValidationStatus,
    pub is_error: bool,
    pub error_class: Option<String>,
    pub likely_cause: Option<String>,
    pub suggested_next_action: Option<String>,
}
```

This record is exposed via the `/dev/log` SSE endpoint. Every agent action that touches the system is logged with its risk classification, execution duration, and outcome — including whether it was denied before execution.

Raw shell access produces no such record unless the harness adds its own instrumentation layer — which is exactly what EATMCP provides as a built-in.

---

## 7. Quantitative Summary

| Metric | Raw Shell Access | EATMCP |
|---|---|---|
| Environment awareness before first command | 0 (guessed) | Full fingerprint in 1 call |
| Error format | Unstructured stderr text | Structured JSON with `error_class`, `likely_cause`, `suggested_next_action` |
| CRITICAL command blocking | None — commands execute | Hard deny before process spawn |
| HIGH-risk command gate | None | `action: CONFIRM` signal to harness |
| Path-with-spaces handling | Agent responsibility | Resolver normalizes automatically |
| `.cmd`/`.bat` wrapping on Windows | Agent must know to use `cmd /c` | Resolver applies automatically |
| Cross-platform command translation | Agent must know alternatives | Resolver rejects + provides alternative |
| Post-execution validation | Separate shell commands + text parsing | Built into `service_restart`, `network_port_check`, `filesystem_exists` |
| Execution audit trail | None (unless harness adds it) | Built-in per-call record with risk level |
| Timeout handling | Platform-dependent, often no signal | Structured `timed_out: true`, `suggested_next_action` |
| Shell injection surface | Full string interpolation | Structured args array, no shell string construction |

---

## 8. What This Means for Harness and Agent Developers

The mcp-terminal-commander is not a thin wrapper around shell execution. It is an operational layer that makes agent-to-OS interaction deterministic, auditable, and safe by construction.

**For harness developers:**
- Replace raw shell tool exposure with EATMCP. The surface area of what agents can accidentally destroy is bounded by the safety classifier, not by agent reasoning quality.
- The four-level risk taxonomy (`CRITICAL/HIGH/MEDIUM/LOW`) gives the harness a structured hook for human-in-the-loop confirmation policies without custom parsing.

**For agent developers:**
- Load the `eatmcp` skill. The skill costs nothing at inference time and eliminates an entire category of environment-guessing errors before the first command runs.
- Use `environment_inspect` once at session start. Every subsequent command decision is made with ground-truth environment data, not statistical inference about what OS the host is likely running.

**For teams operating AI agents in production:**
- The execution record provides the audit trail that compliance and incident response require. Every command, its risk classification, its outcome, and its duration are recorded automatically.
- CRITICAL patterns are blocked by the server, not by agent reasoning. This is a hard boundary, not a soft guideline.

---

*Generated from live session evidence on Windows 11 (26200) with EATMCP v0.1.0 (eatmcp crate, Rust 2021 edition).*
