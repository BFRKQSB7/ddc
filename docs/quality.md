# ddc quality metrics

How ddc measures decompilation quality, and where the current version
stands. All numbers are reproducible with `tests/scripts/quality-report.sh`.

## The metrics

**E1 — compile errors (the golden gate).** The decompiled tree is
compiled with the platform `javac` (android.jar + optional libraries,
the same classpath across versions). The error count is the primary
quality metric: decompiled code must compile. Three families are
tracked inside it:

- `env:missing-package` — SDK classes outside the public android.jar
  (org.conscrypt, coil, …). Not fixable from DEX; excluded from
  comparisons by the `ddc` column.
- phantom classes — R8 deleted the class but kept references to it;
  the DEX has no definition to emit. A corpus floor.
- everything else — ddc's own output quality. This is what releases
  must not regress and what quality rounds drive down.

**S1 — output lines.** Total emitted `.java` lines across the corpus.
Lower is better for the same error count: register-noise copies,
`x = e; return x` tails, `while(it.hasNext())` loops and compiler
artifacts are semantic-preservingly collapsed by the cure
source-level pass.

**S2 — structural simplification** (sampled per file by cure's
`--stats`): AST nodes and decision points (cyclomatic complexity
proxies) before/after — measures real structure work, immune to
formatting.

**D1 — determinism.** Two full runs must produce byte-identical
trees (0-diff). Every release is checked.

**P1 — wall time.** End-to-end decompile time per corpus, measured
cold, twice, median. The cure pass costs roughly the decompile
itself on large corpora (~2×); `--fast` skips the simplification
engine (formatting only) for roughly half the wall time.

## Where we stand (2026-10-11, four-corpus battery)

The four public APK corpora every release is gated on:

| corpus | files | errors (E1) | vs v0.1.27 | lines (S1) | vs raw |
|---|---|---|---|---|---|
| reqable | 5,579 | **317** | −66 | 607,038 | **−15.1%** |
| weibo | 110,525 | **3,622** | −66 | 15,472,863 | −16.0% |
| lark | 118,502 | **1,034** | −5 | 17,321,592 | −16.8% |
| weixin | 239,600 | **3,674** | −8 | 21,777,774 | −13.7% |
| **total** | **474,206** | **8,647** | **−148** | **55.2M** | **−15.3%** |

(v0.1.27 = the last released baseline measured on the identical
corpora and classpath; raw = the source-level pass disabled — the
isolation measurement, taken before the raw mode was folded out of
the CLI surface.)

Sampled structural simplification (reqable, cure stats): 28,184
edits, AST nodes −4.1%, decision points −4.5%, parse failures 0/5,579.

## Version-over-version (E1, same corpora + methodology)

| state | errors | note |
|---|---|---|
| v0.1.27 (no cure) | 8,795 | last release, identical corpora |
| + cure integration | 8,748 | additive source-level pass (7 cure correctness guards) |
| + 4 IR passes retired | 8,725 | trailing-return/ternary/final-dead/xor → cure absorbs (parity, −360 LoC in ddc) |
| + obscured-member escape | **8,647** | JLS 6.4.2 static-import escape (reqable cannot-find:变量 103 → 5) |

Total this arc: **−148 errors (−1.7%), −15.3% output lines, ddc
source −360 LoC**. Longer arc on the same gate (earlier corpora mix):
26,672 (2026-09) → 8,777 (v0.1.25) → 8,647.

## Known floors (not counted as fixable)

- `env:missing-package` — 1,380 across the battery.
- phantom classes — R8 deleted-class references (weibo b2×84 class,
  weixin ~1.5k cannot-find:类 dominated by this).
- local-vs-import simple-name capture (weixin +2) — the escape
  machinery cannot see local variables at the render point.

## Performance (P1)

| corpus | decompile (default) | `--fast` |
|---|---|---|
| reqable (36 MB) | 0.9 s | 0.5 s |
| weibo (185 MB) | 23.4 s | 11.5 s |

The cure pass is the dominant post-decompile cost (parse →
fixed-point simplify → print per file, inside the class worker
threads). `--fast` is the latency-sensitive mode.

## Reproducing

```sh
tests/scripts/quality-report.sh <apk>...   # E1 + S1 per corpus
tests/scripts/errtax.py <sweep-dir>        # error family breakdown
```

Two-run 0-diff and `cargo test` are part of every release gate.
