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
| 2026-10-15 | v2-qwen35-4b | Qwen3.5-4B-Q8_0.gguf | small-model profile | 6d3e804 | 3 | 111 | 95.2% [92.3, 97.0] | 95.1% | 95.4% | 3.5 | 2026-10-16-qwen35-4b-v2.json |
| 2026-10-15 | v3-qwen35-4b-rules | Qwen3.5-4B-Q8_0.gguf | small-model profile, no thinking | 0e5caa7 | 3 | 111 | 95.2% [92.3, 97.0] | 95.5% | 94.3% | 3.0 | 2026-10-16-qwen35-4b-v3-rules.json |
| 2026-10-16 | v3-qwen35-4b-think-512 | Qwen3.5-4B-Q8_0.gguf | profile + thinking ≤512 | 0e5caa7 | 3 | 111 | 98.2% [96.1, 99.2] | 98.8% | 96.6% | 7.0 | 2026-10-16-qwen35-4b-v3-think-512.json |
| 2026-10-16 | v3-qwen35-4b-think | Qwen3.5-4B-Q8_0.gguf | profile + thinking, no limit | 0e5caa7 | 3 | 111 | 98.5% [96.5, 99.4] | 98.8% | 97.7% | 7.0 | 2026-10-16-qwen35-4b-v3-think.json |
| 2026-10-16 | v2world-qwen35-4b-think512 | Qwen3.5-4B-Q8_0.gguf | v2 suite · thinking ≤512 | 0e5caa7 | 3 | 84 | 66.3% [60.2, 71.8] | 63.5% | 74.6% | 15.6 | 2026-10-16-v2-qwen35-4b-think512.json |
| 2026-10-16 | v2world-qwen35-4b-nothink | Qwen3.5-4B-Q8_0.gguf | v2 suite · no thinking | 0e5caa7 | 3 | 84 | 57.9% [51.8, 63.9] | 55.6% | 65.1% | 6.0 | 2026-10-16-v2-qwen35-4b-nothink.json |
| 2026-10-16 | v2world-qwen35-9b-think512 | Qwen3.5-9B-Q5_K_M.gguf | v2 suite · thinking ≤512 | 0e5caa7 | 3 | 84 | 72.2% [66.4, 77.4] | 67.7% | 85.7% | 12.1 | 2026-10-16-v2-qwen35-9b-think512.json |
| 2026-10-19 | nemotron3-nano-4b-think512 | NVIDIA-Nemotron-3-Nano-4B-Q8_0.gguf | profile + thinking ≤512 | 0e5caa7 | 3 | 111 | 79.0% [74.3, 83.0] | 79.3% | 78.2% | 5.0 | 2026-10-19-nemotron3-nano-4b-think512.json |
| 2026-10-19 | gemma4-e4b-think512 | gemma-4-E4B-it-Q8_0.gguf | profile + thinking ≤512 | 0e5caa7 | 3 | 111 | 94.9% [92.0, 96.8] | 94.7% | 95.4% | 6.0 | 2026-10-19-gemma4-e4b-think512.json |
| 2026-10-19 | qwen38-4b-distill-think512 | Qwen3.8-4B-Distill-Q8_0.gguf | profile + thinking ≤512 | 0e5caa7 | 3 | 111 | 92.5% [89.2, 94.9] | 92.3% | 93.1% | 5.5 | 2026-10-19-qwen38-4b-distill-think512.json |
| 2026-10-19 | v2world-qwen38-4b-distill-think512 | Qwen3.8-4B-Distill-Q8_0.gguf | v2 suite · thinking ≤512 | 0e5caa7 | 3 | 84 | 54.4% [48.2, 60.4] | 54.0% | 55.6% | 9.3 | 2026-10-19-v2-qwen38-4b-distill-think512.json |
| 2026-10-19 | v2world-gemma4-e4b-think512 | gemma-4-E4B-it-Q8_0.gguf | v2 suite · thinking ≤512 | 0e5caa7 | 3 | 84 | 62.7% [56.6, 68.4] | 60.3% | 69.8% | 8.5 | 2026-10-19-v2-gemma4-e4b-think512.json |
