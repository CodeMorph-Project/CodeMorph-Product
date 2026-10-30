# Models

Models are external artifacts loaded through the registry. Each artifact has a manifest describing role, source and target language, supported features, architecture, parameter count, tokenizer, context size, version, hashes, backend, artifact format, and protocol version.

Directories use convention:

`models/<role>/manifest.json`

Model sizes such as 10M, 30M, 60M, 100M, and 300M are experiment parameters.

A production process must not package a mock backend as if it were a model. Missing model directories must be reported as missing.