#!/usr/bin/env python3
"""Build assets/stars/stars.bin, the star catalogue of the plate solver.

Source: the star data shipped with KStars (namedstars.dat and unnamedstars.dat,
GPL-2.0-or-later; Hipparcos and Tycho-2 stars to magnitude 8, J2000 positions).
Positions are moved by their proper motion to the epoch 2025.0; double entries
are dropped.

Output (little endian): b"PSSTARS1", u32 count, then per star, brightest
first: f32 right ascension (degrees), f32 declination (degrees), i16 V
magnitude × 100.

    python3 tools/make_star_catalog.py /usr/share/kstars assets/stars/stars.bin
"""
import math
import os
import struct
import sys

EPOCH = 2025.0
MAG_LIMIT = 8.0


def read_kstars(path):
    b = open(path, "rb").read()
    o = 124  # text preamble
    nfields = struct.unpack_from("<H", b, o + 3)[0]
    o += 5 + 16 * nfields
    ntrixels = struct.unpack_from("<I", b, o)[0]
    o += 4
    stars = []
    for k in range(ntrixels):
        _, off, cnt = struct.unpack_from("<III", b, o + 12 * k)
        for r in range(cnt):
            ra, de, pmra, pmde, _par, _hd, mag, _bv = struct.unpack_from("<iiiiiihh", b, off + 32 * r)
            stars.append((ra / 1e6 * 15.0, de / 1e5, pmra / 10.0, pmde / 10.0, mag / 100.0))
    return stars


def main():
    src, out = sys.argv[1], sys.argv[2]
    stars = read_kstars(os.path.join(src, "namedstars.dat")) + read_kstars(os.path.join(src, "unnamedstars.dat"))
    years = EPOCH - 2000.0
    seen = set()
    rows = []
    for ra, de, pmra, pmde, mag in stars:
        if mag > MAG_LIMIT:
            continue
        key = (round(ra, 4), round(de, 4))
        if key in seen:
            continue
        seen.add(key)
        # proper motion in mas/yr (RA component already times cos(dec))
        c = max(math.cos(math.radians(de)), 1e-6)
        de2 = de + pmde * years / 3.6e6
        ra2 = (ra + pmra * years / 3.6e6 / c) % 360.0
        rows.append((mag, ra2, de2))
    rows.sort()
    with open(out, "wb") as f:
        f.write(b"PSSTARS1")
        f.write(struct.pack("<I", len(rows)))
        for mag, ra, de in rows:
            f.write(struct.pack("<ffh", ra, de, int(round(mag * 100))))
    print(f"{len(rows)} stars to magnitude {MAG_LIMIT} written to {out}")


if __name__ == "__main__":
    main()
