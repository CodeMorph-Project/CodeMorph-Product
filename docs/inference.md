# Inference

Inference requests are structured objects handled by a `ModelBackend`. Backends may be local model, MLX, PyTorch, llama.cpp-compatible, external teacher API, or test backend.

Every generation stores model ID, model version, template version, generation parameters, input hash, outputs, timestamp, latency, and token counts when available. Output generation is cached by model version, prompt version, input hash, and generation config.