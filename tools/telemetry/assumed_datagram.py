#!/usr/bin/env python3
"""Build assumed synthetic datagrams from packed packet tables; never a capture.

Example: python3 tools/telemetry/assumed_datagram.py 2026 0 \
  --set car_motion_data.23.g_force_lateral=-1900 > motion.bin
Values are JSON literals, and field paths omit the packet's header prefix.
"""
import argparse
import json
from pathlib import Path
import struct
import sys

TABLES = json.loads(Path(__file__).with_name('packet_tables.json').read_text())
PACKETS = ['Motion', 'Session', 'Lap', 'Event', 'Participants', 'CarSetup',
           'CarTelemetry', 'CarStatus', 'FinalClassification', 'LobbyInfo',
           'CarDamage', 'SessionHistory', 'TyreSets', 'MotionEx', 'TimeTrial',
           'LapPositions', 'CarTelemetry2']
PRIMITIVES = {'u8': 'B', 'i8': 'b', 'u16': 'H', 'i16': 'h', 'u32': 'I',
              'u64': 'Q', 'f32': 'f', 'f64': 'd'}


def build(packet_format, packet_id, values=None):
    """Return a zero-filled packet with explicit named field overrides."""
    values = dict(values or {})
    schema = TABLES[str(packet_format)]
    values.setdefault('header.packet_format', packet_format)
    values.setdefault('header.packet_version', 1)
    values.setdefault('header.packet_id', packet_id)
    values.setdefault('header.secondary_player_car_index', 255)

    def encode(kind, dims, path):
        if dims:
            return b''.join(encode(kind, dims[1:], f'{path}.{i}') for i in range(dims[0]))
        if kind in PRIMITIVES:
            return struct.pack('<' + PRIMITIVES[kind], values.pop(path, 0))
        return b''.join(encode(f['type'], f['dims'], f"{path}.{f['name']}".lstrip('.'))
                        for f in schema[kind])

    result = encode('Packet' + PACKETS[packet_id] + 'Data', [], '')
    if values:
        raise ValueError(f'Unknown fields: {sorted(values)}')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('packet_format', type=int, choices=[2025, 2026])
    parser.add_argument('packet_id', type=int, choices=range(17))
    parser.add_argument('--set', action='append', default=[], metavar='PATH=JSON')
    args = parser.parse_args()
    values = {}
    for entry in args.set:
        key, value = entry.split('=', 1)
        values[key] = json.loads(value)
    sys.stdout.buffer.write(build(args.packet_format, args.packet_id, values))


if __name__ == '__main__':
    main()
