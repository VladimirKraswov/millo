# Millo website

The project website is deliberately small: static HTML/CSS/JavaScript plus a
Python standard-library service. The service owns SQLite persistence and LLM
moderation; the browser never receives moderation credentials or database
access.

## Run locally

```bash
MILLO_SITE_DATABASE=/tmp/millo-feedback.sqlite3 \
MILLO_SITE_PORT=8080 \
python3 site/server.py
```

Open `http://127.0.0.1:8080`. Without an LLM configuration, submitted feedback
is retained with `pending` status and is not published. This fail-closed mode is
intentional.

## Moderation

The adapter uses an OpenAI-compatible `/v1/chat/completions` endpoint:

```bash
export MILLO_MODERATION_URL=http://llm.internal:18080/v1
export MILLO_MODERATION_MODEL=gemma-4-26b-a4b-it-qat-q4_0
export MILLO_MODERATION_API_KEY=optional-secret
```

For a native llama.cpp server, constrained completion avoids model-specific
reasoning wrappers and guarantees the response shape:

```bash
export MILLO_MODERATION_URL=http://llama.internal:8003
export MILLO_MODERATION_MODEL=qwen3.6-27b-coding
export MILLO_MODERATION_MODE=llama-completion
```

Deterministic validation runs before the LLM. The model receives untrusted
feedback as JSON and must return a constrained JSON decision. Invalid output,
timeouts, or an unavailable model produce `pending`, never automatic approval.
The policy allows criticism and bug reports while rejecting spam, threats,
doxxing, harmful wrongdoing, and other prohibited public content.

Only a salted one-way IP fingerprint is retained for abuse control. Raw client
IP addresses are not stored in SQLite.

## Test

```bash
python3 -m unittest discover -s site/tests -v
```

Production deployment and proxy details are documented in
[`docs/WEBSITE_DEPLOYMENT.md`](../docs/WEBSITE_DEPLOYMENT.md).
