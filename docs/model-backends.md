# Model backends

English · [Русский](#настройка-модели)

## Local CodeMorph artifact

Use `--model-dir PATH` to select an installed translator with a
`manifest.json`. For automatic discovery, set `CODEMORPH_MODELS_ROOT` to
the registry directory containing `translator/manifest.json`. Local neural
inference needs the Python runtime and PyTorch. Set `CODEMORPH_PYTHON` and
`CODEMORPH_PYTHONPATH` to override discovery.

## OpenAI-compatible endpoint

Copy the profile into the local registry and edit its endpoint and model ID.

```sh
mkdir -p models
cp -R config/cloud-model.example models/openai
export CODEMORPH_ALLOW_EXTERNAL=1
# Set OPENAI_API_KEY in your shell or secret manager.
./target/debug/codemorph migrate ./cpp --output ./migration-output   --model-dir models/openai --full
```

The template uses the OpenAI Chat Completions endpoint. For another compatible
provider, change `chat_completions_url`, `model` and `api_key_env`. For a
local Ollama server, use `http://127.0.0.1:11434/v1/chat/completions`, select
an installed model, and set `api_key_env` to an empty string. Hosted endpoints
use HTTPS. The cloud runtime uses only the Python standard library.
Set optional generation parameters in `options` according to the selected model.

The opt-in authorizes sending input source to the configured endpoint. Review
the endpoint and its data policy before sending proprietary code. Credentials
come from the named environment variable.

## Настройка модели

Для локальной модели CodeMorph передайте `--model-dir PATH` с каталогом,
содержащим `manifest.json`. Для автопоиска укажите корень реестра переменной
`CODEMORPH_MODELS_ROOT`, внутри него ожидается `translator/manifest.json`.
Для локального инференса нужны Python и PyTorch.

Для OpenAI скопируйте `config/cloud-model.example` в `models/openai`,
задайте имя модели в `cloud_config.json`, установите переменные
`OPENAI_API_KEY` и `CODEMORPH_ALLOW_EXTERNAL=1` и запустите команду выше.
Для другого провайдера измените URL его совместимого Chat Completions API
и имя переменной ключа. Для локального Ollama используйте
`http://127.0.0.1:11434/v1/chat/completions`, имя установленной модели и
пустое значение `api_key_env`. Облачному бэкенду достаточно стандартной
библиотеки Python.

Включение этого режима разрешает отправку исходного кода по указанному адресу.
Перед отправкой закрытых исходников проверьте адрес и правила обработки данных.
