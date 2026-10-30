# Training

Training uses an independent subsystem, initially optimized for Apple Silicon while remaining portable. Backends include MLX and PyTorch/MPS, with CUDA available later through configuration.

Training configs record architecture, parameter count, tokenizer, dataset version, context length, batch size, gradient accumulation, optimizer, learning rate schedule, precision, seed, and checkpoint interval. Every run stores config, git commit, dataset version, metrics, checkpoints, environment, and seed. Resume is required.