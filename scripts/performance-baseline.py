#!/usr/bin/env python3
"""Windows/macOS baseline recorder, using only the Python standard library.

No input automation: interactive scenarios require an operator on a synthetic
desktop. Reports keep missing observations separate from successful samples.
"""
import argparse
import ctypes
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import time
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[1]
EVENT = re.compile(r"\b(capture|search|startup)_latency\s+(.*)")


def distribution(values):
    ordered = sorted(values)
    if not ordered:
        return {"count": 0, "p50": None, "p95": None, "max": None}
    return {"count": len(ordered), "p50": ordered[math.ceil(len(ordered) * .5) - 1],
            "p95": ordered[math.ceil(len(ordered) * .95) - 1], "max": ordered[-1]}


def summarize_logs(paths):
    requests, stages = {}, {}
    dropped = warnings = errors = 0
    for path in paths:
        # Each recorder owns a new profile directory; rotation keeps that
        # namespace, while restarted processes must use different directories.
        run = str(Path(path).resolve().parent)
        for line in Path(path).read_text(encoding="utf-8", errors="replace").splitlines():
            lost = re.search(r"dropped (\d+) log lines", line)
            dropped += int(lost[1]) if lost else 0
            warnings += " WARN [" in line
            errors += " ERROR [" in line
            match = EVENT.search(line)
            if not match:
                continue
            kind = match[1]
            fields = dict(re.findall(r"(\w+)=([^\s]+)", match[2]))
            if not {"id", "stage", "elapsed_us"} <= fields.keys():
                raise ValueError(f"Malformed timing event in {path}")
            elapsed = int(fields["elapsed_us"])
            if elapsed < 0:
                raise ValueError("Negative elapsed time")
            key = (run, kind, fields["id"])
            request = requests.setdefault(key, {})
            stage = fields["stage"]
            # Retain the first observation for repeated startup Ready events
            # and one native visibility observation per display.
            observations = request.setdefault(stage, {})
            monitor = fields.get("monitor", "all")
            observations[monitor] = min(observations.get(monitor, math.inf), elapsed / 1000)
            if "monitors" in fields:
                request["expected_monitors"] = int(fields["monitors"])
    outcomes = {kind: {"started": 0, "complete": 0, "incomplete": 0,
                       "latency_ms": []} for kind in ("search", "capture")}
    starts = {"search": "query_submitted", "capture": "capture_requested"}
    for (_, kind, _), request in requests.items():
        for stage, values in request.items():
            if stage != "expected_monitors":
                stages.setdefault(f"{kind}.{stage}", []).append(max(values.values()))
        if kind not in starts or starts[kind] not in request:
            continue
        result = outcomes[kind]
        result["started"] += 1
        if kind == "search":
            terminal = request.get("results_painted", {})
        else:
            terminal = request.get("mask_native_visible", {})
            expected = request.get("expected_monitors", 0)
            if expected <= 0 or len(terminal) != expected:
                terminal = {}
        if terminal:
            result["complete"] += 1
            result["latency_ms"].append(max(terminal.values()))
        else:
            result["incomplete"] += 1
    for result in outcomes.values():
        result["distribution_ms"] = distribution(result["latency_ms"])
    return {"requests": outcomes, "stages_ms": {k: distribution(v) for k, v in stages.items()},
            "dropped_log_lines": dropped, "warning_lines": warnings, "error_lines": errors,
            "timing_data_valid": dropped == 0,
            "limitations": "Incomplete includes cancellation, failure and observation timeout; never infer success from missing events. Search paint is CPU paint; capture is native visibility after hidden paint, neither is physical screen visibility. Zero samples are unmeasured. Startup starts at run(), not OS process creation."}


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def write_json(path, value):
    with Path(path).open("x", encoding="utf-8") as stream:
        json.dump(value, stream, ensure_ascii=False, indent=2, allow_nan=False)
        stream.write("\n")


def new_target(path):
    path = Path(path).absolute()
    target = (ROOT / "target").resolve()
    # Resolve symlinks/junctions before checking the boundary. Never clean up
    # directories: even an interrupted run remains evidence.
    if not path.resolve().is_relative_to(target) or path.exists() or path.resolve() == target:
        raise ValueError("Use a new directory below the workspace target directory")
    path.mkdir(parents=True, exist_ok=False)
    return path


def ps_cpu_seconds(value):
    days, clock = (value.split("-", 1) if "-" in value else ("0", value))
    total = 0.0
    for part in clock.split(":"):
        total = total * 60 + float(part)
    return int(days) * 86400 + total


class Sampler:
    def __init__(self, pid):
        self.pid = pid
        self.handle = None
        if sys.platform == "win32":
            from ctypes import wintypes as w
            self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
            self.psapi = ctypes.WinDLL("psapi", use_last_error=True)
            self.kernel.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
            self.kernel.OpenProcess.restype = w.HANDLE
            self.kernel.CloseHandle.argtypes = [w.HANDLE]
            self.kernel.GetProcessTimes.argtypes = [w.HANDLE] + [ctypes.POINTER(w.FILETIME)] * 4
            size = ctypes.c_size_t

            class Memory(ctypes.Structure):
                _fields_ = [("cb", w.DWORD), ("faults", w.DWORD)] + [(name, size) for name in (
                    "peak_ws", "working_set", "peak_paged", "paged", "peak_nonpaged",
                    "nonpaged", "pagefile", "peak_pagefile", "private")]

            self.Memory = Memory
            self.psapi.GetProcessMemoryInfo.argtypes = [w.HANDLE, ctypes.POINTER(Memory), w.DWORD]
            self.handle = self.kernel.OpenProcess(0x0400 | 0x0010, False, pid)
            if not self.handle:
                raise ctypes.WinError(ctypes.get_last_error())

    def sample(self):
        if self.handle:
            from ctypes import wintypes as w
            stamps = [w.FILETIME() for _ in range(4)]
            if not self.kernel.GetProcessTimes(self.handle, *[ctypes.byref(v) for v in stamps]):
                raise ctypes.WinError(ctypes.get_last_error())
            memory = self.Memory()
            memory.cb = ctypes.sizeof(memory)
            if not self.psapi.GetProcessMemoryInfo(self.handle, ctypes.byref(memory), memory.cb):
                raise ctypes.WinError(ctypes.get_last_error())
            cpu = sum((v.dwHighDateTime << 32) + v.dwLowDateTime for v in stamps[2:]) / 1e7
            return {"cpu_seconds": cpu, "resident_bytes": memory.working_set,
                    "private_bytes": memory.private, "lifetime_peak_resident_bytes": memory.peak_ws}
        output = subprocess.check_output(
            ["ps", "-p", str(self.pid), "-o", "time=", "-o", "rss="], text=True, timeout=5).split()
        if len(output) != 2:
            raise RuntimeError("Cannot read process counters")
        return {"cpu_seconds": ps_cpu_seconds(output[0]), "resident_bytes": int(output[1]) * 1024,
                "private_bytes": None, "lifetime_peak_resident_bytes": None}

    def close(self):
        if self.handle:
            self.kernel.CloseHandle(self.handle)
            self.handle = None


def git_output(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def record(args):
    if sys.platform not in ("win32", "darwin"):
        raise ValueError("Only Windows and macOS are supported")
    indexed = args.scenario in ("idle-index", "search")
    if indexed and not args.synthetic_machine:
        raise ValueError("Indexing enumerates system volumes. Run on a dedicated synthetic machine with --synthetic-machine")
    binary = Path(args.executable).resolve(strict=True)
    env = {k: v for k, v in os.environ.items()
           if k not in ("ROTOR_DATA_DIR", "ROTOR_RESOURCE_DIR", "ROTOR_CAPTURE_TIMING", "ROTOR_PERF_TIMING")}
    identity = json.loads(subprocess.check_output([str(binary), "--build-info"], env=env, text=True, timeout=15))
    if identity["production"] or identity["identifier"] != "cc.fluctus.rotor.dev":
        raise ValueError("Use a development binary")
    directory = new_target(args.output)
    command = [str(binary), "--background", "--no-elevate", "--data-dir", str(directory / "profile")]
    if not indexed:
        command.append("--no-index")
    if args.scenario in ("idle", "idle-index"):
        command.append("--no-hotkeys")
    env["ROTOR_PERF_TIMING"] = "1"
    source_diff = subprocess.check_output(["git", "diff", "HEAD", "--binary"], cwd=ROOT)
    (directory / "source.diff").write_bytes(source_diff)
    (directory / "recorder.py").write_bytes(Path(__file__).read_bytes())
    report = {"schema_version": 1, "utc": datetime.now(timezone.utc).isoformat(),
              "build": identity, "binary_sha256": sha256(binary), "git_head": git_output("rev-parse", "HEAD"),
              "git_status": git_output("status", "--short"),
              "tracked_diff_sha256": hashlib.sha256(source_diff).hexdigest(),
              "recorder_sha256": sha256(__file__), "command": command,
              "os": platform.platform(), "architecture": platform.machine(), "cpu": platform.processor(),
              "logical_processors": os.cpu_count(), "display_config": args.display_config,
              "workload": args.workload, "scenario": args.scenario,
              "stabilization_seconds": args.stabilization, "sample_seconds": args.seconds,
              "interval_seconds": args.interval, "samples": [], "failures": [],
              "limitations": "Main process only; sampled peaks may miss short allocations; GPU memory unavailable. macOS ps reports RSS, not private memory. Interactive operations are operator driven. Fresh profile is not a cold filesystem cache. Ending this run terminates only its child, not graceful shutdown acceptance."}
    write_json(directory / "metadata.json", {k: v for k, v in report.items() if k != "samples"})
    process = sampler = None
    begin = time.monotonic()
    try:
        with (directory / "stdout.txt").open("x", encoding="utf-8") as stdout, \
                (directory / "stderr.txt").open("x", encoding="utf-8") as stderr, \
                (directory / "samples.jsonl").open("x", encoding="utf-8") as raw:
            process = subprocess.Popen(command, env=env, cwd=binary.parent, stdout=stdout, stderr=stderr,
                                       creationflags=subprocess.CREATE_NO_WINDOW if sys.platform == "win32" else 0)
            sampler = Sampler(process.pid)
            print(f"Recording {args.scenario}, pid={process.pid}, evidence={directory}", flush=True)
            while time.monotonic() - begin < args.stabilization + args.seconds:
                if process.poll() is not None:
                    raise RuntimeError(f"Process exited early: {process.returncode}")
                row = sampler.sample()
                row["seconds"] = time.monotonic() - begin
                row["phase"] = "stabilization" if row["seconds"] < args.stabilization else "sample"
                report["samples"].append(row)
                raw.write(json.dumps(row) + "\n")
                raw.flush()
                time.sleep(args.interval)
    except (Exception, KeyboardInterrupt) as error:
        report["failures"].append(str(error) or type(error).__name__)
    finally:
        if sampler:
            sampler.close()
        if process and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=10)
    rows = [row for row in report["samples"] if row["phase"] == "sample"]
    report["resources"] = {key: distribution([row[key] for row in rows if row[key] is not None])
                           for key in ("resident_bytes", "private_bytes", "lifetime_peak_resident_bytes")}
    report["single_core_cpu_percent"] = (100 * (rows[-1]["cpu_seconds"] - rows[0]["cpu_seconds"]) /
                                          (rows[-1]["seconds"] - rows[0]["seconds"])) if len(rows) > 1 else None
    logs = sorted((directory / "profile").glob("rotor*.log"))
    report["timing"] = summarize_logs(logs)
    if not logs:
        report["failures"].append("No application log produced")
    write_json(directory / "baseline.json", report)
    print(f"Report: {directory / 'baseline.json'}", flush=True)
    return 1 if report["failures"] else 0


def positive(value):
    result = float(value)
    if not math.isfinite(result) or result <= 0:
        raise argparse.ArgumentTypeError("Must be finite and positive")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    collect = commands.add_parser("record")
    collect.add_argument("--executable", required=True)
    collect.add_argument("--output", required=True)
    collect.add_argument("--scenario", choices=("idle", "idle-hotkeys", "idle-index", "search", "capture"), required=True)
    collect.add_argument("--synthetic-machine", action="store_true")
    collect.add_argument("--display-config", required=True, help="Monitor resolutions/scales or explicit unmeasured status")
    collect.add_argument("--workload", required=True, help="Fixture size, storage/hardware, operations and cache state")
    collect.add_argument("--stabilization", type=positive, default=60)
    collect.add_argument("--seconds", type=positive, default=120)
    collect.add_argument("--interval", type=positive, default=.1)
    summary = commands.add_parser("summarize")
    summary.add_argument("logs", nargs="+")
    summary.add_argument("--output", required=True)
    fixture = commands.add_parser("fixture")
    fixture.add_argument("--output", required=True)
    fixture.add_argument("--files", type=int, choices=(1000, 10000, 100000), default=1000)
    args = parser.parse_args()
    if args.command == "record":
        return record(args)
    if args.command == "summarize":
        write_json(args.output, summarize_logs(args.logs))
    else:
        directory = new_target(args.output)
        for index in range(args.files):
            folder = directory / f"group-{index // 1000:03}" / f"batch-{index // 100:04}"
            folder.mkdir(parents=True, exist_ok=True)
            (folder / f"rotor-fixture-{index:06}.txt").write_bytes(b"synthetic\n")
        write_json(directory / "fixture.json", {"files": args.files, "depth": 2, "bytes_per_file": 10,
                                               "query": "rotor-fixture", "contains_user_data": False})
    return 0


if __name__ == "__main__":
    sys.exit(main())
