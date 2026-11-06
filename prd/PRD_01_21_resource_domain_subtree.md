# PRD 01.21 — resource domain subtree (T3)

Status: **study + structure only** (owner direction 2026-11-06 evening:
learn and structure first; **do not start code or tests until the plan is
complete**). This is the largest subtree and therefore gets its own document.
It changes no source, version, capability row, `cu-abi`, `[no]` row or the G
boundary. **Everything below is research and planning, not authority.**

`PRD_01_08` assigns process / CPU / memory / GPU / device / network
management outside `screen`; the readback proves desktop state, never process,
memory or device state. `PRD_01_17` records the controller's G decision
(2026-11-06): **keep G closed (a)**, with four reopen triggers in
`PRD_01_24`.
Sibling subtrees: `PRD_01_19` (T1), `PRD_01_20` (T2), `PRD_01_22` (T4/T6),
`PRD_01_23` (T5/T7).

Legend: `[study]` research note, `[no]` explicit non-goal for `screen`,
`[decision]` a dated controller decision already taken. Evidence grades: **O**
official doc, **B** benchmark board, **P** paper / preprint, **V** vendor
claim, **S** survey / secondary, **X** local measurement.

## T3 — resource domain tree

```text
T3 Resource domain (learned, NOT screen authority)
├── R1 process / pid                                               [no]
│   ├── R1.1 observation metadata: pid in `observe.windows[]`        [v] identity only
│   ├── R1.2 namespaces (pid/net/mnt/ipc/uts) + pids.max             [study]
│   ├── R1.3 fork-bomb containment, reparenting leakage paths         [study]
│   └── R1.4 process placement at creation vs epilogue cleanup        [study]
├── R2 CPU                                                         [no]
│   ├── R2.1 EEVDF (6.6+) replacing CFS; vruntime / lag / deadline   [study]
│   ├── R2.2 cgroup v2 `cpu.max` quota/period, `cpu.weight` 1..10000 [study]
│   ├── R2.3 `cpu.stat` nr_throttled / throttled_usec diagnosis       [study]
│   ├── R2.4 `cpuset.cpus` / `cpuset.mems`, NUMA affinity             [study]
│   └── R2.5 `cpu.pressure` (PSI) for early stall signals             [study]
├── R3 memory                                                      [no]
│   ├── R3.1 `memory.max` hard / `memory.high` soft reclaim           [study]
│   ├── R3.2 `memory.low` / `memory.min` protection floors            [study]
│   ├── R3.3 `memory.oom.group`, OOM scoring, `memory.stat`           [study]
│   ├── R3.4 THP / HugeTLB / multi-gen LRU (6.6)                      [study]
│   └── R3.5 `memory.pressure` (PSI), direct reclaim stalls           [study]
├── R4 GPU / accelerator                                           [no]
│   ├── R4.1 hardware partition: MIG (<=7 instances, A100/H100)      [study]
│   ├── R4.2 SR-IOV VF + IOMMU (AMD/Intel 32 VF class)                [study]
│   ├── R4.3 vGPU time-slicing (logical isolation, 3-8% overhead)     [study]
│   ├── R4.4 MPS shared context (no memory/fault isolation)           [study]
│   ├── R4.5 `CUDA_VISIBLE_DEVICES` = visibility, NOT a boundary       [study]
│   ├── R4.6 memory isolation != compute isolation                     [study]
│   └── R4.7 WDDM / GPU paravirtualisation on Windows guests           [study]
├── R5 I/O and storage                                             [no]
│   ├── R5.1 `io.max` per-device rbps/wbps/iops, `io.weight`          [study]
│   ├── R5.2 `io.pressure`, blk-mq, io_uring (5.1+/6.6)               [study]
│   ├── R5.3 tmpfs-only model loading, ephemeral volumes              [study]
│   └── R5.4 snapshot / checkpoint with memory, fork from snapshot     [study]
├── R6 device and network                                          [no]
│   ├── R6.1 eBPF device filtering, XDP/TC, Tetragon/Cilium policy    [study]
│   ├── R6.2 default-deny egress + domain/CIDR allow-list             [study]
│   ├── R6.3 block 169.254.169.254 (cloud metadata)                   [study]
│   └── R6.4 Landlock / seccomp-BPF / AppArmor / SELinux              [study]
├── R7 isolation ladder                                            [study]
│   ├── R7.1 shared-kernel container (weakest, fastest)               [study]
│   ├── R7.2 gVisor user-space kernel (syscall interception)          [study]
│   ├── R7.3 Firecracker / Kata microVM, one guest kernel per task    [study]
│   └── R7.4 full KVM VM; "do not downgrade a microVM to a container" [study]
├── R8 credentials, lifecycle and audit                            [study]
│   ├── R8.1 egress proxy injects real secrets; agent holds a token   [study]
│   ├── R8.2 three timeouts: tool call / task loop / sandbox life      [study]
│   ├── R8.3 immutable audit log, cost attribution, quotas             [study]
│   └── R8.4 `stop` is the single biggest cost lever (25x idle cost)   [study]
├── R9 observability and cost                                      [study]
│   ├── R9.1 PSI cpu/memory/io; EEVDF + cgroup v2 + eBPF stack        [study]
│   ├── R9.2 2 vCPU / 4 GiB price spread 7.3x ($0.052-$0.378)         [study]
│   ├── R9.3 memory is 72.6-79.6% of the bill, not CPU                 [study]
│   ├── R9.4 bill per *successful* task, not per call                  [study]
│   └── R9.5 model/vendor/framework/application four-layer value chain [study]
└── R10 acceptance from inside the guest                           [study]
    ├── R10.1 try host paths, device files, metadata endpoint          [study]
    ├── R10.2 try each denied network class                            [study]
    ├── R10.3 saturate CPU/memory, confirm caps without neighbour harm [study]
    └── R10.4 cleanup on exit / crash / timeout / forced stop          [study]
```

## Findings, 2026-11-06 (third W0 pass)

**Kernel control surface** [O + S]: cgroups v2 is a single unified hierarchy
(one process belongs to one cgroup); `cpu.max` is `"quota period"` in
microseconds and `cpu.weight` is 1..10000; a too-low CPU limit shows up as
latency spikes (`nr_throttled`, `throttled_usec`), not as a utilisation alarm.
`memory.max` is hard, `memory.high` triggers proactive reclaim,
`memory.low`/`min` are reclaim-resistant floors, `memory.oom.group` kills the
whole group instead of leaving half-dead applications, and `memory.stat`
breaks down anon / page cache / swap. `io.max` limits bandwidth and IOPS per
device; `pids.max` contains fork bombs; `cpuset.cpus`/`mems` pin CPU and NUMA
nodes. PSI (`cpu.pressure`, `memory.pressure`, `io.pressure`) gives stall
signals before visible latency. EEVDF (6.6+) replaced CFS for `SCHED_NORMAL`,
tracking vruntime and lag to prioritise under-served tasks via virtual
deadlines. A cgroup is a **boundary, not a placement algorithm**.

**GPU has no native isolation primitive** [S + O]: a GPU is a stateful
accelerator (CUDA context, VRAM, SM/GPC, on-chip scheduler) historically
passed through as a whole PCIe device. Four layers: SR-IOV/IOMMU virtual
functions (hardware, up to ~32 VFs on 2025-validated AMD/Intel parts), MIG
(hardware partition, up to 7 instances on A100/H100, dedicated GPC / L2 /
VRAM / bandwidth, ~1-3% overhead, real fault isolation), MPS (driver-level
shared context, reduced launch contention but **no memory isolation and a
shared failure domain**), and `CUDA_VISIBLE_DEVICES` (visibility only — a
container can still saturate the whole card). Software vGPU is time-slicing
with logical isolation (~3-8% overhead). **Memory isolation is not compute
isolation**: two models each holding half the VRAM can still starve each
other's SMs. MIG is unavailable on consumer / workstation SKUs (5090,
6000 Pro). `CUDA_VISIBLE_DEVICES` is a hint, never a security boundary.

**Isolation ladder is a threat-model choice** [S + O]: containers share the
kernel; gVisor intercepts and re-implements syscalls in user space;
Firecracker / Kata microVMs give one guest kernel per workload and are the
correct answer for adversarial multi-tenant code; full KVM VMs sit above.
Cold starts are environment-dependent (~150 ms from a Firecracker snapshot,
sub-second for gVisor, 60-90 ms for warm container pools) and must not be
compared across setups. Engineering rule: do not downgrade a microVM to a
container for a small price difference — the two are tens of percent apart,
not multiples, and the security boundary is not the same.

**Four production boundaries** [O + S]: isolation, default-deny egress with
explicit allow-lists (including blocking the metadata endpoint), credential
brokerage (the agent never holds a long-lived key; an egress proxy injects
real secrets only for a verified destination), and lifecycle + immutable
audit + enforceable quotas. A representative declarative shape is NVIDIA
OpenShell (Landlock, seccomp, cgroups v2, egress proxy, MIG profiles).
`pids.max` plus three nested timeouts are the standard containment set.

**Cost is a first-class engineering signal** [S + V, cross-checked]: for the
same 2 vCPU / 4 GiB busy hour, listed prices ran from $0.052 to $0.378
(7.3x) and *both ends were full KVM VMs*. What actually determines the bill:
whether vCPUs are dedicated, wall-clock vs active-CPU billing, whether memory
is charged while the agent waits for the model, and pause semantics. On
active-CPU platforms memory is 72.6-79.6% of the bill; an unclosed sandbox
costs ~25x a closed one; a 5-minute minimum can bill a 180-second run as 300
seconds. The correct unit is **cost per successful task** = (tokens +
orchestration + retrieval + tools + retries + human review + amortised build)
/ success rate. Human-review minutes and the success-rate denominator
dominate; tokens are not the main term. Vendor market-share numbers (especially
domestic cloud comparisons published on a vendor's own domain) are **V** and
must be checked against IDC / Synergy originals before any decision.

**Acceptance must be guest-side** [O + S]: attempt to read host paths, device
files and `169.254.169.254`; try each denied network class; saturate CPU and
memory and confirm the cap fires without harming a neighbour; test cleanup on
normal exit, crash, timeout and forced stop. This mirrors the owner-view rule
this repo already applies to public Releases.

## G boundary — decision taken 2026-11-06 22:15

`PRD_01_08` lists four structural unlock conditions for ever widening CU into
host resources: a new PRD with its own threat model, a fourth tool (or a
widening of `exec`) with its own grant and bounds, a readback/evidence
semantics with an independent reader and three-OS journey, and a version gate.
`PRD_01_17` records three mutually exclusive directions. **The controller
decides (a)** under the standing `/goal` delegation; the owner's 22:15
direction corrected an earlier escalation that offered this choice back.
Full record and reopen triggers: `PRD_01_24` (T8).

| Direction | Meaning | Cost | State |
|---|---|---|---|
| (a) keep G closed | learn and upsert the whole domain, implement only K/V/M/W/S | no process/CPU/memory/GPU verbs | **decided 2026-11-06** |
| (b) additive read-only telemetry | typed CPU/memory pressure, process count or GPU inventory in `disclose` | lifts one `[no]`; needs a three-OS probe, named `unknown` reasons and a fresh PRD | `[_]` deferred, 4 triggers |
| (c) real resource control | cgroups / MIG / microVM lifecycle actions | new tool or ABI, new grants, policy engine, audit surface — a different product | `[no]` |

**Telemetry trust split (pass 4).** Host-side signals (per-cgroup CPU/memory,
KVM exits, egress flow records, OOM events) sit on the far side of the
isolation boundary and can be evidence. Guest self-reports (process lists,
in-guest usage) are debugging data: a compromised guest can forge them, so
they can never be the sole basis of a security or quota decision. This is why
direction (b) is not a harmless "read-only" row. Source classes O / P / S;
no X-grade measurement exists in this repo.

No research finding is a decision. This subtree implements none of (a/b/c)
and grants `screen` no resource authority.

## Learning notes (dated, process not just conclusions)

| Date | Pass | What was queried | Source class | Conflict / caution recorded | Went to |
|---|---|---|---|---|---|
| 2026-11-06 | 1 | isolation primitives, quotas, credential brokerage | O / S / V | boot timings from different environments are not comparable | `PRD_01_17`, R7 |
| 2026-11-06 | 1 | GPU isolation (MIG/MPS/SR-IOV) | O / S | MPS "shares capacity" is not fault isolation; `CUDA_VISIBLE_DEVICES` is not a boundary | `PRD_01_17`, R4 |
| 2026-11-06 | 1 | guest-side acceptance of isolation | S | no CI evidence in this repo; it is a method note, not a shipped test | `PRD_01_17`, R10 |
| 2026-11-06 | 3 | cgroups v2 knobs, EEVDF, PSI, io_uring | O / S | "6.6 default" values differ by distro; kernel version must be pinned per claim | R2/R3/R5/R9 |
| 2026-11-06 | 3 | GPU virtualisation layers and overhead | S / V | 1-3% / 3-8% overhead figures are vendor-adjacent (V), not independent benchmarks | R4 |
| 2026-11-06 | 3 | sandbox pricing, four-layer value chain, TCO | S / V (multi-source) | domestic share numbers are vendor self-reports; price tables age fast | R7/R9 |
| 2026-11-06 | 3 | four production boundaries, `pids.max`, three timeouts | O / S | policy examples are representative shapes, not a standard | R6/R8 |
| 2026-11-06 | 4 | host-side vs guest-side telemetry trust; PSI as a pressure signal | O / P / S | guest self-report is forgeable and is debugging data, not a security boundary; `usage/limit` is lagging, PSI `some`/`full` measures stall time | R9/R10, G decision |
| 2026-11-06 | 4 | decision authority correction under `/goal` | owner direction | G (a) is decided by the controller; (b) has four triggers; (c) is `[no]` | `PRD_01_24`, T8 |

## Non-goals

- `[no]` any process, CPU, memory, GPU, device or network control verb in
  `screen`; no `pid`-based authority (a `pid` is identity metadata).
- `[no]` a runtime service, resident owner, durable job lease or colocated
  runtime library: these contradict "one file, one invocation".
- `[no]` claiming any R-node as `[v]`. All are `[study]` until a separate PRD,
  a dated controller decision and three-OS CI evidence exist.
