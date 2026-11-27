#!/usr/bin/env python3
"""Build assets/stars/labels.tsv, the names shown on the sky grid.

Sources:
- bright stars (to magnitude 3.5) with their names: KStars' namedstars.dat
  and starnames.dat (GPL-2.0-or-later), moved by proper motion to 2025.0;
- the Messier objects, and other nebulae and clusters with a common name
  brighter than magnitude 7: OpenNGC as shipped with KStars (OpenNGC.kscat,
  by Mattia Verga, CC-BY-SA-4.0).

One line per object, tab separated: kind (s = star, d = deep sky), name,
right ascension (degrees), declination (degrees), magnitude, size (arc
minutes, 0 for stars).

    python3 tools/make_sky_labels.py /usr/share/kstars assets/stars/labels.tsv
"""
import math
import os
import sqlite3
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from make_star_catalog import read_kstars  # noqa: E402

EPOCH = 2025.0
STAR_MAG = 3.5
DSO_MAG = 7.0


def star_names(path):
    b = open(path, "rb").read()
    o = 124
    nfields = struct.unpack_from("<H", b, o + 3)[0]
    o += 5 + 16 * nfields
    ntrixels = struct.unpack_from("<I", b, o)[0]
    o += 4
    names = []
    for k in range(ntrixels):
        _, off, cnt = struct.unpack_from("<III", b, o + 12 * k)
        for r in range(cnt):
            rec = b[off + 40 * r: off + 40 * r + 40]
            bayer = rec[:8].split(b"\0")[0].decode("latin1").strip()
            long = rec[8:].split(b"\0")[0].decode("latin1").strip()
            names.append(long or bayer)
    return names


def common_name(long_name):
    # "Andromeda Galaxy, PGC 2557, UGC 454, NGC 224" -> "Andromeda Galaxy"
    for part in (long_name or "").split(","):
        p = part.strip()
        if p and not any(p.startswith(c + " ") for c in ("PGC", "UGC", "NGC", "IC", "MCG", "ESO", "CGCG", "Cr", "Mel", "LBN", "Sh2-")):
            return p
    return ""


def main():
    src, out = sys.argv[1], sys.argv[2]
    rows = []
    stars = read_kstars(os.path.join(src, "namedstars.dat"))
    names = star_names(os.path.join(src, "starnames.dat"))
    assert len(stars) == len(names)
    years = EPOCH - 2000.0
    for (ra, de, pmra, pmde, mag), name in zip(stars, names):
        if mag > STAR_MAG or not name:
            continue
        c = max(math.cos(math.radians(de)), 1e-6)
        rows.append(("s", name, (ra + pmra * years / 3.6e6 / c) % 360.0, de + pmde * years / 3.6e6, mag, 0.0))
    db = sqlite3.connect("file:" + os.path.join(src, "OpenNGC.kscat") + "?mode=ro", uri=True)
    seen = set()
    for name, long_name, ra, de, mag, size in db.execute("select name, long_name, ra, dec, magnitude, major_axis from cat where type not in (0, 1, 255)"):  # not stars, not unknown
        common = common_name(long_name)
        messier = name.startswith("M ")
        if not messier and not (common and mag is not None and mag <= DSO_MAG):
            continue
        label = f"{name.replace(' ', '')} {common}".strip() if messier else (common or name)
        key = (round(ra, 2), round(de, 2))
        if key in seen:
            continue
        seen.add(key)
        rows.append(("d", label, ra, de, mag if mag is not None else 99.0, size or 0.0))
    rows.sort(key=lambda r: (r[0], r[4]))
    with open(out, "w", encoding="utf-8") as f:
        for kind, name, ra, de, mag, size in rows:
            f.write(f"{kind}\t{name}\t{ra:.4f}\t{de:.4f}\t{mag:.2f}\t{size:.1f}\n")
    print(f"{sum(r[0] == 's' for r in rows)} stars, {sum(r[0] == 'd' for r in rows)} deep-sky objects written to {out}")


if __name__ == "__main__":
    main()
