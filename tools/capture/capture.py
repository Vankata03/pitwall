#!/usr/bin/env python3
"""Capture raw F1 25 UDP telemetry to a file, unchanged, for later analysis.

Usage:  python capture.py <label> [--port 20777] [--out captures]
Stop with Ctrl+C. Python 3.8+, standard library only.

File layout (little-endian):
  magic  b"PWCAP1\n"
  u32    metadata length, then that many bytes of UTF-8 JSON
  records until end of file:
    u64  nanoseconds since capture start
    u16  datagram length
    ...  datagram bytes
A crash loses at most the last second of records; a truncated last record is
expected and readers should ignore it.
"""
import argparse
import json
import re
import socket
import struct
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

MAGIC = b"PWCAP1\n"
RECORD = struct.Struct("<QH")
# m_packetFormat, m_gameYear, major, minor, m_packetVersion, m_packetId, m_sessionUID
HEADER = struct.Struct("<HBBBBBQ")
STATUS_SECONDS = 2.0


def main():
    parser = argparse.ArgumentParser(description="Capture raw F1 25 UDP telemetry.")
    parser.add_argument("label", help="what this capture holds, e.g. 2026-monza-practice")
    parser.add_argument("--port", type=int, default=20777)
    parser.add_argument("--out", type=Path, default=Path("captures"))
    args = parser.parse_args()

    if not re.fullmatch(r"[A-Za-z0-9_-]+", args.label):
        parser.error("label may use only letters, digits, - and _")

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 16 * 1024 * 1024)
    sock.bind(("0.0.0.0", args.port))
    sock.settimeout(0.25)

    started = datetime.now(timezone.utc)
    args.out.mkdir(parents=True, exist_ok=True)
    path = args.out / f"{started:%Y-%m-%d_%H%M%S}_{args.label}.pwcap"
    meta = json.dumps(
        {"label": args.label, "started_utc": started.isoformat(), "port": args.port}
    ).encode()

    print(f"Listening on UDP {args.port}, writing {path}. Ctrl+C to stop.")
    count = size = window = 0
    last_header = None
    origin = time.perf_counter_ns()
    next_status = next_flush = time.monotonic() + STATUS_SECONDS

    with open(path, "wb") as out:
        out.write(MAGIC + struct.pack("<I", len(meta)) + meta)
        try:
            while True:
                try:
                    data = sock.recv(65535)
                except socket.timeout:
                    data = None
                if data is not None:
                    elapsed = time.perf_counter_ns() - origin
                    out.write(RECORD.pack(elapsed, len(data)))
                    out.write(data)
                    count += 1
                    window += 1
                    size += len(data)
                    if len(data) >= HEADER.size:
                        last_header = data
                now = time.monotonic()
                if now >= next_flush:
                    out.flush()
                    next_flush = now + 1.0
                if now >= next_status:
                    print(status(count, size, window, last_header))
                    window = 0
                    next_status = now + STATUS_SECONDS
        except KeyboardInterrupt:
            pass
    print(f"Stopped. {count} datagrams, {size / 1e6:.1f} MB in {path}")


def status(count, size, window, last_header):
    rate = window / STATUS_SECONDS
    line = f"{count} datagrams, {size / 1e6:.1f} MB, {rate:.0f}/s"
    if window == 0:
        return f"{line} (no packets)"
    packet_format, year, major, minor, _, _, uid = HEADER.unpack_from(last_header)
    return f"{line}, format {packet_format}, year {year}, v{major}.{minor}, session {uid:016x}"


if __name__ == "__main__":
    sys.exit(main())
