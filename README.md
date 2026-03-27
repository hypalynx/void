# void

CLI agent for LLM interaction. Tap the infinite.

An agentic tool that brings language model capabilities to your terminal, letting you
explore emergent possibilities through a clean command-line interface.

## Configuration

Configure void via `~/.void/config.toml` with named profiles for different AI backends:

```toml
[default]
profile = "local"

[profile.local]
host = "127.0.0.1"
port = 7777

[profile.gemini]
host = "generativelanguage.googleapis.com"
port = 443
model = "gemini-3-flash-preview"
path_prefix = "/v1beta/openai"
api_key_env = "GEMINI_API_KEY"

[profile.kimi]
host = "api.fireworks.ai"
port = 443
model = "accounts/fireworks/models/kimi-k2p5"
path_prefix = "/inference"
api_key_env = "FIREWORKS_API_KEY"

[profile.glm]
host = "api.fireworks.ai"
port = 443
model = "accounts/fireworks/models/glm-4p7"
path_prefix = "/inference"
api_key_env = "FIREWORKS_API_KEY"
```

**Usage:**
- `void` — uses default profile (local)
- `void --profile gemini` — use Gemini profile with GEMINI_API_KEY
- `void --profile kimi` — use Kimi K2.5 from Fireworks with FIREWORKS_API_KEY
- `void --profile glm` — use GLM 4.7 from Fireworks with FIREWORKS_API_KEY
- `void --profile gemini --model custom-model` — override model via CLI flags

## TODO

- add default AGENTS.md and ability to pass in your own.
- switching URLs/models (and make it configurable?) and test out
  GLM 5 + Kimi K2
- Ctrl + J for input (newline) and expanding input box, also
  up/down arrow to move up/down input BUT at top/bottom instead
  goes up/down user input history.
- parse/render tables!
- Esc/Ctrl + C should warn if LLM is responding and pressing
  again should close the connection/stop receiving the request..
  if no LLM responding then exits as usual.
- allow config of LLMs/urls/api keys etc + switching
- draw just the lines that can be seen to keep consistent 60 fps

---

## Name Origin

The name *void* draws from primordial mythology—the space of infinite potential before creation.
In this sense, a language model is much the same: vast knowledge compressed into emergent
possibilities, a frontier of what's achievable. Void captures that sense of reaching into
something primal and powerful to pull out what's needed.
