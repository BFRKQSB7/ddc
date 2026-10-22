# Benchmarks

[English] | [简体中文](zh-CN/benchmarks.md)

All numbers from the author's machine (Apple Silicon, 6P+12E cores),
measured after a cooldown, release profile with
`target-cpu=native`. The benchmark harness is kept out of the published tree (local APK paths); the tables
above are the recorded results.

Recorded 2026-10-22 with v0.1.33: one run per cell after a short settle; the previous table was recorded on an earlier (pre-cure) version; full and `--fast` are now separate columns. Peak RSS is `/usr/bin/time -l`.

## Full decompile

Seven real-world APKs — reqable 3.2.23 (34MB), Telegram (62MB), WhatsApp
(139MB), weibo 16.9.1 (226MB, 20 dex), weixin 8.0.78 (268MB), lark 8.0.2
(398MB, 59 dex), qq 9.3.65 (374MB) — wall time, peak RSS via `/usr/bin/time -l`,
measured after a cooldown:

| APK | Size | Full decompile | `--fast` | Peak RSS (full) |
|---|---|---|---|---|
| reqable | 34 MB | **0.83s** | 0.50s | 280 MB |
| Telegram | 62 MB | **6.91s** | 4.08s | 924 MB |
| WhatsApp | 139 MB | **24.98s** | 11.41s | 1067 MB |
| weibo | 226 MB | **21.03s** | 10.87s | 1380 MB |
| weixin | 268 MB | **40.21s** | 25.29s | 1497 MB |
| lark | 398 MB | **26.29s** | 14.61s | 2406 MB |
| qq | 374 MB | **57.00s** | 31.96s | 2511 MB |

**Compile validation — read the scope before quoting this number**:
every `.java` of all seven APKs (909,689 files) passes `javac`'s parse
gate (`-XDshould-stop.ifNoError=PARSE -XDshould-stop.ifError=PARSE`)
with **zero syntax errors**. That gate stops at the parser: it does NOT
type-check, resolve symbols, or guarantee the output compiles against a
classpath.

**Semantic quality is measured separately — a javac battery with the
full Android classpath** (android-37 jar + optional libraries) over four
corpora, counting ddc-attributable diagnostics. That battery is the
driver of the post-0.1.4 fix rounds and has gone **11,345 → 7,452
errors (−34%) from v0.1.19 to v0.1.22**, including a multi-handler
exception-routing bug present since the first release (every catch of a
multi-handler method ran the first handler's code — invisible to any
parse gate) and three javac-blind value-loss defects found by reading
the output against the disassembly. Semantic quality still trails jadx
on type recovery (erased generics, register-typed locals in R8 monster
methods) — see "Known limitations" in the README.

## Every query subcommand, same seven APKs

Measured after a cooldown; cells are wall time / peak RSS. Query targets:
`strings -f <package> --with-locations`; `findrefs string <package>`;
`findrefs method onCreate`; `hierarchy`/`disasm`/`getclass` use each
app's launcher class (Telegram's `LaunchActivity` is an exceptionally
large class):

| Command | reqable | Telegram | WhatsApp | weibo | weixin | lark | qq |
|---|---|---|---|---|---|---|---|
| `info` | 0.24s / 55MB | 0.41s / 117MB | 0.85s / 316MB | 1.32s / 569MB | 1.61s / 602MB | 2.46s / 969MB | 2.30s / 1145MB |
| `listclasses` | 0.02s / 25MB | 0.03s / 89MB | 0.04s / 96MB | 0.14s / 337MB | 0.16s / 371MB | 0.18s / 251MB | 0.24s / 311MB |
| `manifest` | 0.00s / 12MB | 0.00s / 19MB | 0.00s / 26MB | 0.01s / 48MB | 0.01s / 37MB | 0.01s / 34MB | 0.01s / 54MB |
| `mainactivity` | 0.02s / 20MB | 0.06s / 72MB | 0.08s / 136MB | 0.17s / 199MB | 0.18s / 199MB | 0.22s / 226MB | 0.31s / 300MB |
| `res` | 0.00s / 12MB | 0.00s / 20MB | 0.00s / 27MB | 0.02s / 56MB | 0.01s / 37MB | 0.01s / 36MB | 0.01s / 57MB |
| `largest` | 0.02s / 28MB | 0.05s / 71MB | 0.13s / 195MB | 0.32s / 363MB | 0.30s / 346MB | 0.46s / 519MB | 0.66s / 631MB |
| `strings` | 0.02s / 20MB | 0.04s / 55MB | 0.12s / 117MB | 0.23s / 177MB | 0.28s / 198MB | 0.18s / 192MB | 0.49s / 295MB |
| `findrefs-string` | 0.02s / 31MB | 0.03s / 74MB | 0.04s / 203MB | 0.06s / 397MB | 0.08s / 425MB | 0.04s / 322MB | 0.10s / 849MB |
| `findrefs-method` | 0.02s / 31MB | 0.05s / 81MB | 0.04s / 212MB | 0.06s / 374MB | 0.07s / 448MB | 0.07s / 581MB | 0.11s / 917MB |
| `members` | 0.01s / 19MB | 0.03s / 55MB | 0.05s / 118MB | 0.11s / 174MB | 0.11s / 180MB | 0.14s / 218MB | 0.21s / 285MB |
| `hierarchy` | 0.01s / 20MB | 0.03s / 55MB | 0.05s / 127MB | 0.10s / 174MB | 0.11s / 177MB | 0.15s / 194MB | 0.21s / 294MB |
| `disasm` | 0.01s / 19MB | 0.03s / 58MB | 0.05s / 124MB | 0.10s / 175MB | 0.11s / 182MB | 0.14s / 193MB | 0.20s / 281MB |
| `getclass` | 0.02s / 33MB | 0.21s / 131MB | 0.18s / 256MB | 0.41s / 515MB | 0.50s / 527MB | 0.70s / 830MB | 1.30s / 1200MB |

## Methodology notes

- **Serial runs after a cooldown.** Same-machine variance reaches ±25% (E-core
  scheduling + memory bandwidth contention); a single run proves nothing.
- **`/usr/bin/time`'s user time includes all threads** — a "busy" worker
  is not CPU (write-back-pressured threads tick wall time without cycles).
  Look at user+sys ÷ effective cores vs wall; the gap is scheduling/blocking.
- **Wall-clock critical paths migrate**: first CPU, then disk writes, then
  deadline idling on timed-out classes. Re-profile every round; don't keep
  optimizing the old hotspot.
- `DDC_PHASES=1` (phase timings, cumulative — difference them) and
  `DDC_BUCKETS=1` (per-class by block count) answer "which methods eat
  CPU" more directly than stack sampling; `sample <pid> 3 -file` early and
  late (different phases, different hotspots) when you need line-level
  attribution. RSS curves via `ps -o rss= -p $PID` every 5s locate the
  allocation phase (lark: 733MB at t=5s → 2086MB at t=10s = materialize
  peak, before any retirement fires). `/usr/bin/time -l` reports RSS in
  **bytes, not KB**.

## vs. ASC (earlier APK snapshot, same machine, alternating 3-round medians)

Five real APKs (62–353 MB), progressive queries, stdout discarded;
correctness cross-checked: 107/107 hits coincide:

| Query | ddc | ASC | Ratio |
|---|---|---|---|
| findrefs string (lark 353MB) | **0.32s** | 0.61s | 1.9× |
| findrefs string (weixin 268MB) | **0.22s** | 0.52s | 2.4× |
| findrefs string (weibo 226MB) | **0.19s** | 0.44s | 2.3× |
| findrefs type/method (all five) | **0.09–0.31s** | 0.29–0.66s | 2–3× |
| getclass (lark/weixin/weibo) | 0.18–0.42s | **0.10–0.12s** | ASC wins |
| getclass (Telegram, big class) | **0.05s** | 0.11s | ddc wins |
| findrefs memory (lark) | ~820MB | ~170MB | ASC leaner |

The getclass split: ASC bit-stream-probes the zip to inflate only the
target dex (a lower floor on huge multi-dex APKs); ddc parses all image
headers (~0.2s floor) then materializes lazily — it overtakes when the
class itself is expensive, loses to the floor when it isn't. On findrefs
scan domains ddc is uniformly faster (pipeline: parse waves → bounded
channel → scan threads consume-and-drop; `Arc`-shared APK bytes eliminate
compression copies).
