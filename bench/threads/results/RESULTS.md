# Threads bench results

One line per committed run (`bench.py run --append`, or `bench.py report --append <file>`). Pass is over all
task-runs with its Wilson 95% interval; dev and holdout are the two splits (README, "Splits"). Seconds are the
median per task-run.

| date | name | model | variant | relay | runs | tasks | pass [95% CI] | dev | holdout | median s | file |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 2026-10-15 | baseline-qwen35-4b | Qwen3.5-4B-Q8_0.gguf | baseline | ff599ba7b1 | 3 | 111 | 74.5% [69.5, 78.9] | 73.2% | 78.2% | 16.6 | 2026-10-15-qwen35-4b-baseline.json |
| 2026-10-15 | final4-default | Qwen3.5-4B-Q8_0.gguf | small-model profile | ff599ba7b1+wip | 2 | 82 | 89.6% [84.0, 93.4] | 89.6% | – | 2.5 | 2026-10-15-qwen35-4b-small-model-profile-dev.json |
| 2026-10-15 | final4-default-holdout | Qwen3.5-4B-Q8_0.gguf | small-model profile | ff599ba7b1+wip | 2 | 29 | 84.5% [73.1, 91.6] | – | 84.5% | 2.5 | 2026-10-15-qwen35-4b-small-model-profile-holdout.json |
