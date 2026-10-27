# minicon-harness product requirements

minicon-harness is a bounded agent worker in one portable file: exactly two
tools, `file` and `exec`, an OpenAI-compatible model loop, and no installer,
no bundled runtime and no permission-policy engine. Its packaging model
follows the minicon.com method of the sibling `partnernetsoftware/minicon`
repository: native cells per OS/ISA, a Cosmopolitan fat-trampoline
`minicon-harness.com` with a ZipOS overlay, and a six-cell native execution
court before anything is published.

Legend: `[v]` delivered with evidence, `[-]` partial/open, `[_]` planned,
`[no]` explicit non-goal.

## Product outcome

A script or another program hands one narrowly scoped local file/exec task to
a model and gets the final answer on stdout, in one bounded invocation, from
a single executable that runs on Windows/macOS/Linux x86_64 and ARM64.

```text
minicon-harness — one-file bounded agent worker
├── Worker core (this repo, crates-free Rust)
│   ├── [v] file tool: root-confined read/write, no .. or symlink escape,
│   │   256 KiB bounded reads, atomic publish-by-rename
│   ├── [v] exec tool: allow-listed basename argv, no shell, env allow-list,
│   │   30 s kill, 1 MiB bounded combined capture with marker
│   ├── [v] bounded loop: 16 model turns, 64 tool calls, 10-minute deadline;
│   │   refusals are tool results, never aborts
│   └── [v] OpenAI-compatible codec over ureq; no redirects, 4 MiB bounded body
├── [v] CLI: --root --task --base-url --model --allow-cmd --key-env
│   └── [v] --version banner (exact LF literal, court-verified)
├── Delivery: minicon-harness.com → prd/PRD_01_01_com_delivery.md
│   ├── [v] six native cells (win/osx/lnx × x86_64/aarch64), release profile
│   ├── [v] cosmocc APE trampoline + ZipOS /zip/cells overlay (5.62 MB packed)
│   ├── [v] Authenticode signability padding (unsigned 0.0.1; slot reserved)
│   └── [v] GitHub CI pack + six-cell execute court + public Release v0.0.1
└── [no] plugin host, permission policy, sessions, streaming, MCP, desktop
    authority. The model API cannot execute tools; the tools cannot reach the
    model API. Scope extensions need a new owner decision here.
```

## Governing invariants

- The two tools' own bounds are the whole policy. There is no second
  permission layer, and a refusal is information returned to the model.
- Every bound is a named constant next to the code that enforces it.
- One invocation, one bounded task, one final answer on stdout. The process
  exits; no service identity, no listener.
- The `.com` artifact is a pack of already-qualified native cells. It never
  recompiles the worker against cosmopolitan libc, and cells are never
  rebuilt on a test runner.

## 0.0.1 frontier

[v] First public release shipped 2026-10-27: six cells packed into
`minicon-harness.com` (5,619,674 bytes, sha256 8ea3eb68...), each cell executed
on its own native GitHub runner against the packed APE bytes, the release
download re-hashed and executed post-publish, published as
`minicon-harness.com` + `.sha256` + `build-receipt.json` + `README.md` at
releases/tag/v0.0.1. Unsigned; the Authenticode slot is reserved so a later
signed Candidate does not move APE headers.
