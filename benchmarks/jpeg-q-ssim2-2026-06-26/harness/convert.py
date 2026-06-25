#!/usr/bin/env python3
"""Build Q <-> SSIMULACRA2 <-> bpp conversion ("rosetta") tables from the sweep.

For each encoder (and per content-class), aggregates the per-cell sweep into
monotone anchor curves Q->ssim2 and Q->bpp (with p10/p25/p50/p75/p90 bands), then
exposes all six conversions by interpolating/inverting the monotone medians:
  Q->ssim2, Q->bpp, ssim2->Q, ssim2->bpp, bpp->Q, bpp->ssim2.

Honesty: the medians are central estimates. ssim2<->Q is usable (~±7 IQR); any
direction touching bpp carries a 5-8x content spread, so it is a planning number,
not a per-image value. Conditioning on content-class (and size) tightens it; true
per-image accuracy needs image features (a learned model).

Outputs: results/rosetta_<encoder>[_<class>].csv  +  results/conversion_guide.md
"""
import argparse
import os
import numpy as np
import polars as pl

PCTS = [0.10, 0.25, 0.50, 0.75, 0.90]
QFINE = np.arange(5, 101, 1.0)


def anchors(df: pl.DataFrame) -> pl.DataFrame:
    """Per-q percentiles of ssim2 and bpp (the anchor curve)."""
    aggs = []
    for p in PCTS:
        tag = int(p * 100)
        aggs.append(pl.col("ssim2").quantile(p).alias(f"ssim2_p{tag}"))
        aggs.append(pl.col("bpp").quantile(p).alias(f"bpp_p{tag}"))
    aggs.append(pl.len().alias("n"))
    return df.group_by("q").agg(aggs).sort("q")


def monotone(x):
    """Enforce non-decreasing (medians are ~monotone; clean tiny inversions)."""
    return np.maximum.accumulate(np.asarray(x, dtype=float))


class Rosetta:
    """All six conversions for one encoder/stratum, via monotone median curves."""

    def __init__(self, anc: pl.DataFrame):
        self.q = anc["q"].to_numpy().astype(float)
        self.ssim2 = monotone(anc["ssim2_p50"].to_numpy())
        self.bpp = monotone(anc["bpp_p50"].to_numpy())
        # fine Q grid for convenience lookups
        self.qfine = QFINE
        self.ssim2_fine = np.interp(QFINE, self.q, self.ssim2)
        self.bpp_fine = np.interp(QFINE, self.q, self.bpp)

    def q_to_ssim2(self, q):
        return float(np.interp(q, self.q, self.ssim2))

    def q_to_bpp(self, q):
        return float(np.interp(q, self.q, self.bpp))

    def ssim2_to_q(self, s):
        return float(np.interp(s, self.ssim2, self.q))  # ssim2 monotone in q

    def bpp_to_q(self, b):
        return float(np.interp(b, self.bpp, self.q))  # bpp monotone in q

    def ssim2_to_bpp(self, s):
        return self.q_to_bpp(self.ssim2_to_q(s))

    def bpp_to_ssim2(self, b):
        return self.q_to_ssim2(self.bpp_to_q(b))


def md_table(df: pl.DataFrame, cols=None) -> str:
    if cols:
        df = df.select(cols)
    out = ["| " + " | ".join(df.columns) + " |", "|" + "|".join(["---"] * len(df.columns)) + "|"]
    for row in df.iter_rows():
        out.append("| " + " | ".join(f"{v:.2f}" if isinstance(v, float) else str(v) for v in row) + " |")
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--parquet", default="results/sweep.parquet")
    ap.add_argument("--outdir", default="results")
    args = ap.parse_args()
    df = pl.read_parquet(args.parquet)
    encoders = df["encoder"].unique().sort().to_list()
    classes = df["content_class"].unique().sort().to_list()

    lines = ["# Q ↔ SSIMULACRA2 ↔ bpp conversion guide\n",
             "Central estimates from monotone median curves. Bands show content spread.\n"]

    ros = {}
    for enc in encoders:
        sub = df.filter(pl.col("encoder") == enc)
        anc = anchors(sub)
        anc.write_csv(os.path.join(args.outdir, f"rosetta_{enc}.csv"))
        ros[enc] = Rosetta(anc)
        for cls in classes:
            ca = anchors(sub.filter(pl.col("content_class") == cls))
            ca.write_csv(os.path.join(args.outdir, f"rosetta_{enc}_{cls}.csv"))

        lines.append(f"\n## {enc} — anchor table (Q → ssim2 & bpp, with spread)\n")
        lines.append(md_table(anc, ["q", "ssim2_p25", "ssim2_p50", "ssim2_p75",
                                    "bpp_p10", "bpp_p50", "bpp_p90", "n"]))

    # Worked conversions, both directions, both encoders.
    lines.append("\n## Worked conversions (median; ⚠ = content-bound, wide band)\n")
    lines.append("\n**Pick a quality dial Q → expect:**\n")
    rows = [["Q", *[f"{e} ssim2" for e in encoders], *[f"{e} bpp ⚠" for e in encoders]]]
    for q in [20, 40, 60, 75, 85, 90, 95, 100]:
        rows.append([q] + [round(ros[e].q_to_ssim2(q), 1) for e in encoders]
                    + [round(ros[e].q_to_bpp(q), 2) for e in encoders])
    lines.append(_grid(rows))

    lines.append("\n**Target a SSIMULACRA2 → needed Q (and resulting bpp ⚠):**\n")
    rows = [["target ssim2", *[f"{e} Q" for e in encoders], *[f"{e} bpp ⚠" for e in encoders]]]
    for s in [50, 60, 70, 80, 85, 90]:
        rows.append([s] + [round(ros[e].ssim2_to_q(s)) for e in encoders]
                    + [round(ros[e].ssim2_to_bpp(s), 2) for e in encoders])
    lines.append(_grid(rows))

    lines.append("\n**Have a bpp budget → best Q / ssim2 (⚠ both are rough — bpp is 5–8× content-spread):**\n")
    rows = [["bpp budget", *[f"{e} Q" for e in encoders], *[f"{e} ssim2" for e in encoders]]]
    for b in [0.5, 1.0, 1.5, 2.0, 3.0]:
        rows.append([b] + [round(ros[e].bpp_to_q(b)) for e in encoders]
                    + [round(ros[e].bpp_to_ssim2(b), 1) for e in encoders])
    lines.append(_grid(rows))

    guide = os.path.join(args.outdir, "conversion_guide.md")
    with open(guide, "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"rosetta CSVs + {guide} written ({len(encoders)} encoders, {len(classes)} classes)")
    print("\n".join(lines))


def _grid(rows):
    hdr = rows[0]
    out = ["| " + " | ".join(str(c) for c in hdr) + " |",
           "|" + "|".join(["---"] * len(hdr)) + "|"]
    for r in rows[1:]:
        out.append("| " + " | ".join(str(c) for c in r) + " |")
    return "\n".join(out)


if __name__ == "__main__":
    main()
