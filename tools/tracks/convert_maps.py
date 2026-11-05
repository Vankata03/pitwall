#!/usr/bin/env python3
"""One-time conversion of windman007/f1-25-pit-wall track maps (spec sections 7 and 18).

Usage: python3 -I tools/tracks/convert_maps.py <path-to-clone> [output.json]

The clone is untrusted data: this script only reads `tracks/maps/<Name>_<id>-<length>.json`
and `LICENSE` as text and JSON, and runs nothing from it. Only the track ID, the track
length and the points are imported. Every other field, `corners` above all (it comes from
the MultiViewer API, which carries no licence), is dropped. Points are written as
`[bucket, x, z]`: bucket n is lap distance n * 8 m, x and z are game world coordinates.

The output is checked in at crates/core/assets/tracks.json. The licence text of the
source `LICENSE` is checked in separately, in THIRD-PARTY-NOTICES.md; this script verifies
that the notice still carries it byte for byte.
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_OUT = ROOT / 'crates' / 'core' / 'assets' / 'tracks.json'
NOTICE = ROOT / 'THIRD-PARTY-NOTICES.md'
FILE = re.compile(r'^[A-Za-z-]+_(\d+)-(\d+)\.json$')
# Spec section 7: the 22 shipped layouts, by game track ID.
EXPECTED = {14, 17, 20, 16, 4, 9, 27, 29, 32, 0, 19, 5, 6, 11, 3, 2, 7, 12, 10, 13, 15, 26}
BUCKET_METRES = 8


def convert(source: Path):
    maps = []
    for path in sorted((source / 'tracks' / 'maps').glob('*_*.json')):
        match = FILE.match(path.name)
        if not match:
            continue
        data = json.loads(path.read_text(encoding='utf-8'))
        track_id, length = data['trackId'], data['trackLength']
        if (str(track_id), str(length)) != match.groups():
            raise SystemExit(f'{path.name}: file name and contents disagree')
        points = []
        for bucket, x, z in data['points']:
            if not (isinstance(bucket, int) and 0 <= bucket * BUCKET_METRES <= length):
                raise SystemExit(f'{path.name}: bucket {bucket} outside the lap')
            points.append([bucket, float(x), float(z)])
        if [p[0] for p in points] != sorted({p[0] for p in points}):
            raise SystemExit(f'{path.name}: buckets are not strictly increasing')
        maps.append({'trackId': track_id, 'trackLength': length, 'points': points})
    found = {m['trackId'] for m in maps}
    if found != EXPECTED or len(maps) != 22:
        raise SystemExit(f'expected the 22 spec layouts, got ids {sorted(found)}')
    maps.sort(key=lambda m: m['trackId'])
    return maps


def check_notice(source: Path):
    licence = (source / 'LICENSE').read_text(encoding='utf-8').strip()
    if not NOTICE.exists() or licence not in NOTICE.read_text(encoding='utf-8'):
        raise SystemExit('THIRD-PARTY-NOTICES.md does not carry the source LICENSE text')


def main():
    if len(sys.argv) not in (2, 3):
        raise SystemExit(__doc__)
    source = Path(sys.argv[1])
    out = Path(sys.argv[2]) if len(sys.argv) == 3 else DEFAULT_OUT
    maps = convert(source)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open('w', encoding='utf-8', newline='\n') as file:
        file.write('[\n')
        for i, m in enumerate(maps):
            points = ',\n'.join('    ' + json.dumps(p) for p in m['points'])
            file.write(f'  {{"trackId": {m["trackId"]}, "trackLength": {m["trackLength"]}, "points": [\n{points}\n  ]}}')
            file.write(',\n' if i + 1 < len(maps) else '\n')
        file.write(']\n')
    print(f'wrote {len(maps)} maps to {out}')
    if NOTICE.exists():
        check_notice(source)


if __name__ == '__main__':
    main()
