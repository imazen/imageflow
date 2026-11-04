#!/usr/bin/env python3
"""Compares transform build and run time between two probe binaries.

    python3 -I bench.py <probe-a> <probe-b> <cmyk.icc> <rgb-lut.jpg> [rounds]

Runs `bench` in alternating processes and prints, per transform, the median across processes
and the ratio b/a, with the interquartile range of the per-round ratios.
"""

import statistics
import subprocess
import sys


def run(exe, profile, depth):
    out = subprocess.run([exe, "bench", profile, depth], check=True, capture_output=True, text=True)
    f = out.stdout.split()
    return float(f[1]), float(f[3])


def spread(ratios):
    q = statistics.quantiles(ratios, n=4)
    return f"{statistics.median(ratios):.3f} (IQR {q[0]:.3f}-{q[2]:.3f})"


def main():
    a, b, cmyk, lut = sys.argv[1:5]
    rounds = int(sys.argv[5]) if len(sys.argv) > 5 else 30
    cases = [
        ("cmyk", cmyk, ["8", "16", "f32"]),
        ("rgb-lut", lut, ["8", "16", "f32"]),
        ("display-p3", "builtin:display-p3", ["8", "16", "f32"]),
        ("gray-2.2", "builtin:gray-2.2", ["8", "16"]),
    ]
    print(f"{rounds} rounds; each process reports the median of 41 builds and of 41 runs over 65,536 pixels")
    print(f"{'transform':<16} {'build us a':>10} {'build us b':>10} {'b/a build':>26} "
          f"{'ns/px a':>9} {'ns/px b':>9} {'b/a run':>26}")
    for name, profile, depths in cases:
        for depth in depths:
            pa, pb = [], []
            for i in range(rounds):
                if i % 2:
                    pb.append(run(b, profile, depth))
                    pa.append(run(a, profile, depth))
                else:
                    pa.append(run(a, profile, depth))
                    pb.append(run(b, profile, depth))
            build = spread([y[0] / x[0] for x, y in zip(pa, pb)])
            runs = spread([y[1] / x[1] for x, y in zip(pa, pb)])
            print(f"{name + ' ' + depth:<16} "
                  f"{statistics.median(x[0] for x in pa) / 1000:>10.1f} "
                  f"{statistics.median(x[0] for x in pb) / 1000:>10.1f} {build:>26} "
                  f"{statistics.median(x[1] for x in pa):>9.3f} "
                  f"{statistics.median(x[1] for x in pb):>9.3f} {runs:>26}", flush=True)


if __name__ == "__main__":
    main()
