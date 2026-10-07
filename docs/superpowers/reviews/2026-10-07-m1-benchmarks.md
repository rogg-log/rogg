# M1 benchmark results

Machine: Apple M2 Pro, 10 cores, 32 GB RAM
Data: genlog 1 GiB, seed 42 (10316955 lines)
Command: `just bench`

| Benchmark | Mean | Throughput | Spec target | Pass |
|---|---|---|---|---|
| engine/index | 124.2 ms | 8.05 GiB/s | < 2 s | ✓ |
| engine/filter_plain_4 | 270.8 ms | 3.69 GiB/s | < 1.5 s | ✓ |
| engine/filter_regex_2 | 654.2 ms | 1.53 GiB/s | < 3 s | ✓ |
| index heap (memory test) | 41.3 MB | — | < 100 MB | ✓ |

Notes: the file is warm in the page cache after generation (32 GB RAM), so `engine/index` measures indexing, not cold disk reads. Criterion's "change: regressed" lines in the output compare against the earlier 64 MiB smoke run, which shares the benchmark names; they are not real regressions.

## Optimizations applied
none needed
