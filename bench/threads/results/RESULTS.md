# Threads bench results

One line per committed run (`bench.py run --append`, or `bench.py report --append <file>`). Pass is over all
task-runs with its Wilson 95% interval; dev and holdout are the two splits (README, "Splits"). Seconds are the
median per task-run.

| date | name | model | variant | relay | runs | tasks | pass [95% CI] | dev | holdout | median s | file |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 2026-10-15 | baseline-qwen35-4b | Qwen3.5-4B-Q8_0.gguf | baseline | ff599ba7b1 | 3 | 111 | 74.5% [69.5, 78.9] | 73.2% | 78.2% | 16.6 | 2026-10-15-qwen35-4b-baseline.json |
| 2026-10-15 | final4-default | Qwen3.5-4B-Q8_0.gguf | small-model profile | ff599ba7b1+wip | 2 | 82 | 89.6% [84.0, 93.4] | 89.6% | – | 2.5 | 2026-10-15-qwen35-4b-small-model-profile-dev.json |
| 2026-10-15 | final4-default-holdout | Qwen3.5-4B-Q8_0.gguf | small-model profile | ff599ba7b1+wip | 2 | 29 | 84.5% [73.1, 91.6] | – | 84.5% | 2.5 | 2026-10-15-qwen35-4b-small-model-profile-holdout.json |
| 2026-10-15 | after-qwen35-4b | Qwen3.5-4B-Q8_0.gguf | small-model profile | a24ed13598 | 3 | 111 | 89.8% [86.1, 92.6] | 90.7% | 87.4% | 3.0 | 2026-10-15-qwen35-4b-after.json |
| 2026-10-15 | qwen35-9b | Qwen3.5-9B-Q5_K_M.gguf | small-model profile | a24ed13598 | 3 | 111 | 95.8% [93.1, 97.5] | 96.3% | 94.3% | 2.5 | 2026-10-15-qwen35-9b-after.json |
