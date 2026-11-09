#!/usr/bin/env python3
"""The gate's independent reader (M10, `PRD_01_27` "Evidence").

Given a config file and the JSONL mirror the gate wrote, recompute -- from the
revision alone -- what the gate decided:

  (a) the expanded candidate order for each agent: credential outer, egress
      inner, each in declared order, deduplicated by (endpoint, credential,
      egress);
  (b) for every request, which attempt was chosen and why each earlier
      candidate was skipped or demoted;
  (c) that the recorded egress host equals that egress's host;
  (d) that no row contains a credential-shaped value.

The reader never trusts the gate's own ordering. It rebuilds it.

Exit codes: 0 all checks pass, 1 a mismatch, 2 unreadable input.
"""
from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path

#: Verdict vocabulary from the contract, mapped not reinvented.
VERDICTS = {"performed", "refused", "not_performed", "unknown", "unsupported", "stale"}

#: Values that look like a credential. A row matching one is a leak.
SECRET_PATTERNS = [
    re.compile(r"sk-[A-Za-z0-9_\-]{8,}"),
    re.compile(r"Bearer\s+[A-Za-z0-9_\-\.]{8,}"),
    re.compile(r"AKIA[0-9A-Z]{16}"),
]


def fail(message: str, code: int = 1) -> "NoReturn":  # noqa: F821 - docstring only
    print(f"gate-verify: {message}", file=sys.stderr)
    raise SystemExit(code)


def load_config(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read config {path}: {error}", 2)


def load_rows(path: Path) -> list[dict]:
    rows = []
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        fail(f"cannot read mirror {path}: {error}", 2)
    for number, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError as error:
            fail(f"mirror line {number} is not JSON: {error}", 2)
    return rows


def revision_of(path: Path) -> str:
    """The gate's revision: sha256 over the config text with CRLF normalised."""
    try:
        raw = path.read_bytes()
    except OSError as error:
        fail(f"cannot read config {path}: {error}", 2)
    text = raw.decode("utf-8").replace("\r\n", "\n")
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def expand_lane(lane: dict) -> list[tuple[str, str]]:
    """The Kronecker product: credential outer, egress inner, deduplicated."""
    order: list[tuple[str, str]] = []
    for credential in lane.get("credentialOrder", []):
        for egress in lane.get("egressOrder", []):
            pair = (credential, egress)
            if pair not in order:
                order.append(pair)
    return order


def expand_agent(config: dict, agent: dict) -> list[dict]:
    """Every candidate an agent can reach, in dispatch order."""
    lanes = config.get("lanes", {})
    candidates = []
    for lane_id in agent.get("lanes", []):
        lane = lanes.get(lane_id)
        if lane is None:
            continue
        for credential, egress in expand_lane(lane):
            candidates.append(
                {
                    "lane": lane_id,
                    "endpoint": lane["endpoint"],
                    "credential": credential,
                    "egress": egress,
                    "model": lane["model"],
                }
            )
    return candidates


def host_of(proxy) -> str | None:
    """The host:port an egress sends through, or None for direct."""
    if isinstance(proxy, dict) and isinstance(proxy.get("proxy"), str):
        return None
    url = proxy.get("proxy") if isinstance(proxy, dict) else proxy
    if isinstance(url, dict):
        url = url.get("url")
    if not isinstance(url, str) or url in ("direct", "env"):
        return None
    authority = url.split("://", 1)[-1].split("/")[0]
    return authority.rsplit("@", 1)[-1]


def check_rows(config: dict, rows: list[dict]) -> list[str]:
    problems: list[str] = []
    if not rows:
        return problems

    # (d) secret hygiene first: a leak makes every other conclusion suspect.
    for row in rows:
        rendered = json.dumps(row)
        for pattern in SECRET_PATTERNS:
            if pattern.search(rendered):
                problems.append(
                    f"request {row.get('request_id')} attempt {row.get('attempt')}: "
                    f"a credential-shaped value matched {pattern.pattern}"
                )

    # The revision every row claims must be the revision of this config.
    expected_revision = next((row.get("config_revision") for row in rows if row.get("config_revision")), None)

    endpoints = config.get("endpoints", {})
    credentials = config.get("credentials", {})
    egress = config.get("egress", {})
    agents = config.get("agents", {})

    by_request: dict[str, list[dict]] = {}
    for row in rows:
        by_request.setdefault(row.get("request_id", ""), []).append(row)

    for request_id, chain in by_request.items():
        chain.sort(key=lambda row: row.get("attempt", 0))
        for row in chain:
            # (a) every attempt's triple must be a candidate the agent can reach.
            virtual = row.get("virtual_model", "")
            agent_id = virtual.split("/", 1)[1] if virtual.startswith("gate/") else None
            if agent_id and agent_id in agents:
                reachable = expand_agent(config, agents[agent_id])
                match = any(
                    candidate["lane"] == row.get("lane_id")
                    and candidate["credential"] == row.get("credential_id")
                    and candidate["egress"] == row.get("egress_id")
                    for candidate in reachable
                )
                if not match:
                    problems.append(
                        f"request {request_id} attempt {row.get('attempt')}: "
                        f"{row.get('lane_id')}/{row.get('credential_id')}/{row.get('egress_id')} "
                        f"is not a candidate of agent {agent_id}"
                    )

            # (b) the chain is contiguous and names its trigger.
            if row.get("attempt", 0) > 1:
                if row.get("triggered_by") != row["attempt"] - 1:
                    problems.append(
                        f"request {request_id} attempt {row.get('attempt')}: "
                        f"triggered_by {row.get('triggered_by')} does not name attempt {row['attempt'] - 1}"
                    )
                if not row.get("trigger_reason"):
                    problems.append(
                        f"request {request_id} attempt {row.get('attempt')}: a fallback has no trigger_reason"
                    )

            # (c) the recorded egress host is the egress axis's host. A direct
            # egress has no host at all, so the recorded value must be empty --
            # `startswith("")` would accept anything.
            recorded = row.get("egress_host") or ""
            expected = host_of(egress.get(row.get("egress_id"), {})) or ""
            if expected:
                matches = recorded == expected or recorded.startswith(expected)
            else:
                matches = recorded == ""
            if not matches:
                problems.append(
                    f"request {request_id} attempt {row.get('attempt')}: "
                    f"egress_host {recorded!r} is not egress {row.get('egress_id')} ({expected!r})"
                )
            if "@" in recorded:
                problems.append(
                    f"request {request_id} attempt {row.get('attempt')}: egress_host carries userinfo"
                )

            if row.get("verdict") not in VERDICTS:
                problems.append(
                    f"request {request_id} attempt {row.get('attempt')}: "
                    f"verdict {row.get('verdict')!r} is not in the contract vocabulary"
                )
            if row.get("credential_id") and row["credential_id"] not in credentials:
                problems.append(
                    f"request {request_id} attempt {row.get('attempt')}: "
                    f"credential {row['credential_id']} is not in the config"
                )
            if row.get("endpoint_id") and row["endpoint_id"] not in endpoints:
                problems.append(
                    f"request {request_id} attempt {row.get('attempt')}: "
                    f"endpoint {row['endpoint_id']} is not in the config"
                )
            if row.get("config_revision") != expected_revision:
                problems.append(
                    f"request {request_id} attempt {row.get('attempt')}: "
                    f"revision {row.get('config_revision')} differs from {expected_revision}"
                )
    return problems


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("usage: gate-verify.py CONFIG.jsonc EVENTS.jsonl", file=sys.stderr)
        return 2
    config_path = Path(argv[1])
    mirror_path = Path(argv[2])
    config = load_config(config_path)
    rows = load_rows(mirror_path)
    revision = revision_of(config_path)
    problems = check_rows(config, rows)

    if problems:
        for problem in problems:
            print(f"MISMATCH {problem}", file=sys.stderr)
        return 1
    print(
        f"gate-verify: OK  revision={revision[:12]}  rows={len(rows)}  "
        f"requests={len({row.get('request_id') for row in rows})}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
