#!/usr/bin/env python3
"""Fit JPEG quality-dial -> SSIMULACRA2 calibration tables from the sweep CSV.

Reads the sweep CSV emitted by the `sweep` harness, writes the canonical Parquet,
and emits:
  - per (encoder, q) summary: median ssim2 + p25/p75 + std + median bpp + n
  - the Q -> ssim2 anchor tables (libjpeg-turbo and imageflow-moz), markdown
  - evalchroma's chroma-subsampling distribution per q (the content-adaptive var)
  - "does mozjpeg+evalchroma track better": ssim2 spread at fixed q, and the
    RD frontier (ssim2 at matched bytes-per-pixel)

Usage: python3 analyze.py --csv sweep.csv --outdir results/
"""
import argparse
import os
import polars as pl


def md_table(df: pl.DataFrame) -> str:
    cols = df.columns
    out = ["| " + " | ".join(cols) + " |", "|" + "|".join(["---"] * len(cols)) + "|"]
    for row in df.iter_rows():
        out.append("| " + " | ".join(_fmt(v) for v in row) + " |")
    return "\n".join(out)


def _fmt(v):
    if isinstance(v, float):
        return f"{v:.3f}"
    return str(v)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--csv", required=True)
    ap.add_argument("--outdir", default="results")
    args = ap.parse_args()
    os.makedirs(args.outdir, exist_ok=True)

    df = pl.read_csv(args.csv)
    pq = os.path.join(args.outdir, "sweep.parquet")
    df.write_parquet(pq)
    print(f"{len(df):,} rows -> {pq}")
    print(f"encoders: {df['encoder'].unique().to_list()}")
    print(f"content classes: {df['content_class'].unique().to_list()}")
    print(f"q values: {sorted(df['q'].unique().to_list())}")
    print(f"images: {df['image_id'].n_unique()}, size buckets: {df['tgt_w'].n_unique()} widths")

    # --- per (encoder, q) summary ---
    summary = (
        df.group_by(["encoder", "q"])
        .agg(
            pl.col("ssim2").median().alias("ssim2_p50"),
            pl.col("ssim2").quantile(0.25).alias("ssim2_p25"),
            pl.col("ssim2").quantile(0.75).alias("ssim2_p75"),
            pl.col("ssim2").std().alias("ssim2_std"),
            pl.col("bpp").median().alias("bpp_p50"),
            pl.len().alias("n"),
        )
        .sort(["encoder", "q"])
    )
    summary.write_parquet(os.path.join(args.outdir, "summary_by_encoder_q.parquet"))

    lines = ["# JPEG quality dial -> SSIMULACRA2 calibration\n"]
    for enc in df["encoder"].unique().sort():
        sub = summary.filter(pl.col("encoder") == enc).select(
            ["q", "ssim2_p50", "ssim2_p25", "ssim2_p75", "ssim2_std", "bpp_p50", "n"]
        )
        lines.append(f"\n## {enc}: Q -> SSIMULACRA2 (median, with p25/p75 spread)\n")
        lines.append(md_table(sub))

    # --- evalchroma chroma distribution per q (moz only) ---
    moz = df.filter(pl.col("encoder") == "imageflow-moz")
    if len(moz):
        chroma = (
            moz.group_by(["q", "chroma"])
            .agg(pl.len().alias("n"))
            .with_columns((pl.col("n") / pl.col("n").sum().over("q") * 100).alias("pct"))
            .sort(["q", "chroma"])
        )
        pivot = chroma.pivot(values="pct", index="q", on="chroma").sort("q").fill_null(0.0)
        lines.append("\n## imageflow-moz: evalchroma chroma-subsampling % by q (content-adaptive)\n")
        lines.append(md_table(pivot))

    # --- per content-class Q->ssim2 (median) — within-class anchors are tighter ---
    for enc in df["encoder"].unique().sort():
        sub = df.filter(pl.col("encoder") == enc)
        byclass = sub.group_by(["q", "content_class"]).agg(
            pl.col("ssim2").median().alias("s")
        )
        pivot = byclass.pivot(values="s", index="q", on="content_class").sort("q")
        lines.append(f"\n## {enc}: median SSIMULACRA2 by q × content-class\n")
        lines.append(md_table(pivot))

    # --- "does mozjpeg+evalchroma track better?" ---
    # (a) ssim2 spread (std) at fixed q — lower = more predictable mapping.
    # (b) RD: ssim2 at matched bpp bins — higher ssim2 per byte = more efficient.
    lines.append("\n## Does mozjpeg+evalchroma track better?\n")
    spread = (
        summary.group_by("encoder")
        .agg(pl.col("ssim2_std").mean().alias("mean_ssim2_std_over_q"))
        .sort("encoder")
    )
    lines.append("\n**Predictability — mean SSIMULACRA2 std across q (lower = tighter Q→ssim2):**\n")
    lines.append(md_table(spread))

    rd = (
        df.with_columns((pl.col("bpp").log(2)).round(0).alias("bpp_bin"))
        .group_by(["encoder", "bpp_bin"])
        .agg(pl.col("ssim2").median().alias("ssim2_p50"), pl.len().alias("n"))
        .filter(pl.col("n") >= 20)
        .sort(["bpp_bin", "encoder"])
    )
    lines.append("\n**RD frontier — median SSIMULACRA2 by log2(bpp) bin (higher = better RD):**\n")
    lines.append(md_table(rd))

    table_path = os.path.join(args.outdir, "q_to_ssim2_tables.md")
    with open(table_path, "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"tables -> {table_path}")
    print("\n".join(lines[:4]))


if __name__ == "__main__":
    main()
