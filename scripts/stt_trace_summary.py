#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import os
import sys
from dataclasses import dataclass
from typing import Any, Iterable, Optional


@dataclass
class TraceEvent:
    t_ms: int
    kind: str
    revision: Optional[int]
    strategy: Optional[str]
    text: Optional[str]
    details: dict[str, Any]


@dataclass
class PartialEvent:
    t_ms: int
    revision: int
    strategy: str
    text: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Summarize InkFlow STT trace NDJSON files and optionally print a timeline of stt_partial events."
        )
    )
    parser.add_argument(
        "--path",
        required=True,
        help="Path to a stt_session_<ID>.ndjson file.",
    )
    parser.add_argument(
        "--timeline",
        action="store_true",
        help="Print a line-by-line timeline of stt_partial updates.",
    )
    parser.add_argument(
        "--events",
        action="store_true",
        help="Print a line-by-line timeline of all trace events.",
    )
    parser.add_argument(
        "--kinds",
        default="",
        help=(
            "Comma-separated list of event kinds to print with --events "
            "(for example: stt_partial,segment_commit,stt_final)."
        ),
    )
    parser.add_argument(
        "--max-text-len",
        type=int,
        default=140,
        help="Maximum number of characters to print for each timeline text field.",
    )
    return parser.parse_args()


def read_ndjson(path: str) -> Iterable[dict[str, Any]]:
    with open(path, "r", encoding="utf-8") as f:
        for raw in f:
            line = raw.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(obj, dict):
                yield obj


def truncate(text: str, limit: int) -> str:
    if limit <= 0:
        return ""
    if len(text) <= limit:
        return text
    return text[: max(0, limit - 1)] + "…"


def main() -> int:
    args = parse_args()
    path = args.path

    if not os.path.isfile(path):
        print(f"Trace file does not exist: {path}.", file=sys.stderr)
        return 2

    counts: dict[str, int] = {}
    events: list[TraceEvent] = []
    partials: list[PartialEvent] = []

    for ev in read_ndjson(path):
        kind = str(ev.get("kind") or "")
        if not kind:
            continue

        counts[kind] = counts.get(kind, 0) + 1

        if args.events:
            t_ms = int(ev.get("t_ms") or 0)
            revision_raw = ev.get("revision")
            revision: Optional[int] = int(revision_raw) if revision_raw is not None else None
            strategy_raw = ev.get("strategy")
            strategy = str(strategy_raw) if strategy_raw is not None else None
            text_raw = ev.get("text")
            text = str(text_raw) if text_raw is not None else None
            details_raw = ev.get("details")
            details = details_raw if isinstance(details_raw, dict) else {}
            events.append(
                TraceEvent(
                    t_ms=t_ms,
                    kind=kind,
                    revision=revision,
                    strategy=strategy,
                    text=text,
                    details=details,
                )
            )

        if kind == "stt_partial":
            t_ms = int(ev.get("t_ms") or 0)
            revision = int(ev.get("revision") or 0)
            strategy = str(ev.get("strategy") or "")
            text = str(ev.get("text") or "")
            partials.append(PartialEvent(t_ms=t_ms, revision=revision, strategy=strategy, text=text))

    # Summary metrics
    strategy_switches = 0
    rewrites = 0
    prefix_appends = 0

    last_text: Optional[str] = None
    last_strategy: Optional[str] = None

    for p in partials:
        if last_strategy is not None and p.strategy and p.strategy != last_strategy:
            strategy_switches += 1

        if last_text is not None:
            if p.text.startswith(last_text):
                prefix_appends += 1
            else:
                rewrites += 1

        last_text = p.text
        last_strategy = p.strategy or last_strategy

    print(f"Trace: {path}")
    print(f"Events: {sum(counts.values())} ({len(counts)} kinds)")
    for kind in sorted(counts.keys()):
        print(f"  {kind}: {counts[kind]}")

    print()
    print("stt_partial summary:")
    print(f"  count: {len(partials)}")
    print(f"  strategy_switches: {strategy_switches}")
    print(f"  rewrites: {rewrites}")
    print(f"  prefix_appends: {prefix_appends}")

    if args.timeline and partials:
        print()
        print("Timeline (stt_partial):")
        last_text = None
        last_strategy = None

        for p in partials:
            change = ""
            if last_strategy is not None and p.strategy and p.strategy != last_strategy:
                change = f"{last_strategy}->{p.strategy}"

            flag = ""
            if last_text is not None:
                flag = "append" if p.text.startswith(last_text) else "REWRITE"

            text = truncate(p.text, args.max_text_len)
            print(
                f"{p.t_ms:6d}ms r{p.revision:<4d} {p.strategy:<13s} {flag:<7s} {change:>20s} | {text}"
            )

            last_text = p.text
            last_strategy = p.strategy or last_strategy

    if args.events and events:
        kinds_filter = {k.strip() for k in args.kinds.split(",") if k.strip()}
        print()
        print("Timeline (events):")

        for e in events:
            if kinds_filter and e.kind not in kinds_filter:
                continue

            seg = e.details.get("segment_id")
            gen = e.details.get("window_generation")
            scheduled = e.details.get("window_job_id_last_scheduled")
            applied = e.details.get("window_job_id_last_applied")
            has_window = e.details.get("live_has_window")

            revision = f"r{e.revision}" if e.revision is not None else "-"
            strategy = e.strategy or "-"

            text = truncate(e.text or "", args.max_text_len)
            print(
                f"{e.t_ms:6d}ms {e.kind:<18s} {revision:<6s} {strategy:<13s} "
                f"seg={seg!s:<3s} gen={gen!s:<3s} sch={scheduled!s:<4s} app={applied!s:<4s} "
                f"win={has_window!s:<5s} | {text}"
            )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
