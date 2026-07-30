# CONCH

A portable trust container for agents and humans.

CONCH is a structured digital object format and platform. Every conch carries its own identity, schema, data, permissions, and audit trail — self-describing and self-validating. You don't trust the server that sent it. You trust the object itself.

---

## What is a Conch?

A conch is a JSON object with five required sections:

```json
{
  "meta":        { "id", "version", "created_at", "creator", "conch_version" },
  "schema":      { "version", "fields": { "field_name": { "type", "required", "description" } } },
  "data":        { "field_name": value },
  "permissions": { "read": [...], "write": [...], "admin": [...] },
  "history":     [{ "timestamp", "action", "actor", "diff" }]
}
```

All five sections are required. Data must conform to the schema. Missing fields, wrong types, and undeclared fields are all rejected. The canonical serialized form is deterministic — same object, same bytes, every time, on every machine.

---

## Stack

| Layer    | Technology                          |
| -------- | ----------------------------------- |
| Backend  | Rust + Axum                         |
| Database | PostgreSQL 16                       |
| Events   | Redis 7 + SSE + WebSocket           |
| Frontend | React 18 + TypeScript + Vite        |
| Identity | Ed25519 keypairs (`@noble/ed25519`) |

Backend runs on port `3000` (container) / `3001` (host). Frontend runs on port `5173` and proxies `/api`, `/ws`, `/events` to the backend.

---

## Identity

There are no usernames or passwords. Your Ed25519 public key is your identity. Every request carries an `X-Public-Key` header. The wallet lives in the browser (`frontend/src/lib/wallet.ts`) and never leaves the client.

---

## Getting Started

**Prerequisites:** Docker Desktop, Node.js 18+

**Start the backend:**

```bash
docker compose up -d
```

This starts Postgres (5432), Redis (6379), and the Rust API (3001).

**Start the frontend:**

```bash
cd frontend
npm install
npm run dev
```

Open [http://localhost:5173](http://localhost:5173).

**Rebuild after backend changes:**

```bash
docker compose up -d --build
```

---

## CLI

The `conch` CLI is built into the Docker image. Use it to validate, inspect, and scaffold `.conch` files without touching the web app.

> **Windows users:** run these from PowerShell, not Git Bash. Git Bash mangles `/workspace` paths.

### Commands

**Validate a file** — exits 0 if valid, exits 1 with a full error list if not:

```powershell
docker compose exec api conch validate /workspace/examples/knowledge.conch
```

**Inspect a file** — human-readable breakdown of every section:

```powershell
docker compose exec api conch inspect /workspace/examples/memory.conch
```

**Generate a new conch from a template** — prints canonical JSON to stdout:

```powershell
docker compose exec api conch new knowledge --creator <your-pubkey>
```

### Templates

| Name        | Required fields          | Optional fields                     |
| ----------- | ------------------------ | ----------------------------------- |
| `knowledge` | title, body              | source, tags, confidence            |
| `memory`    | title, observation       | context, tags, importance           |
| `artifact`  | name, description        | language, snippet, tags             |
| `note`      | title, content           | tags                                |

### Dev workflow

```powershell
# 1. Scaffold a new conch
docker compose exec api conch new knowledge --creator abc123 > my.conch

# 2. Edit the file — fill in the empty required fields
#    "title": ""  →  "title": "Your title here"
#    "body": ""   →  "body":  "Your content here"

# 3. Validate your edits
docker compose exec api conch validate /workspace/my.conch

# 4. Repeat until clean, then use the file
```

### Reference examples

Three complete, valid example conches are in the `examples/` directory:

| File                       | Type      | Demonstrates                    |
| -------------------------- | --------- | ------------------------------- |
| `examples/knowledge.conch` | knowledge | Article with confidence score   |
| `examples/memory.conch`    | memory    | Agent observation with context  |
| `examples/artifact.conch`  | artifact  | Code snippet with language tag  |

---

## API Reference

### Core Conch Endpoints

| Method | Path                  | Description                                        |
| ------ | --------------------- | -------------------------------------------------- |
| `POST` | `/api/conch/new`      | Build a fresh ConchObject                          |
| `POST` | `/api/conch/validate` | Validate a JSON string, returns all errors         |
| `POST` | `/api/conch/write`    | Serialize a ConchObject to canonical JSON          |
| `POST` | `/api/conch/parse`    | Parse a raw JSON string into a ConchObject         |

### Conch Storage

| Method   | Path                       | Description          |
| -------- | -------------------------- | -------------------- |
| `GET`    | `/api/conches`             | List conches         |
| `POST`   | `/api/conches`             | Create a conch       |
| `GET`    | `/api/conches/:id`         | Get a conch          |
| `PUT`    | `/api/conches/:id`         | Update a conch       |
| `DELETE` | `/api/conches/:id`         | Delete a conch       |
| `GET`    | `/api/conches/:id/links`   | Get linked conches   |
| `POST`   | `/api/conches/:id/links`   | Link two conches     |

### Other

| Method | Path          | Description                     |
| ------ | ------------- | ------------------------------- |
| `GET`  | `/api/graph`  | All conches + links for graph   |
| `GET`  | `/api/search` | Search conches                  |
| `GET`  | `/health`     | Health check                    |
| `WS`   | `/ws`         | WebSocket for real-time updates |
| `GET`  | `/events`     | SSE stream                      |

---

## Backend Structure

```text
backend/src/
├── conch/
│   ├── types.rs      # ConchObject, ConchMeta, ConchSchema, etc.
│   ├── parser.rs     # parse_conch(json) → ConchObject
│   ├── validator.rs  # validate_conch(&obj) → Result<(), Vec<ConchError>>
│   ├── builder.rs    # ConchBuilder fluent API
│   ├── writer.rs     # write_conch(&obj) → canonical JSON string
│   └── error.rs      # ConchError enum
├── api/mod.rs        # All HTTP handlers
├── db/mod.rs         # Database queries
├── auth/mod.rs       # Ed25519 identity
├── websocket/mod.rs  # WS + SSE handlers
└── main.rs           # Router + server bootstrap
```

---

## Agent Integration (Milestone 4)

The `conch-agent` binary connects a local Ollama LLM to the CONCH format. It reads a `.conch` file, reasons over its contents, and writes a new `.conch` file containing the model's synthesis — no cloud, no API keys, no backend server.

### Prerequisites

Install [Ollama](https://ollama.com) on your machine and pull a model:

```powershell
# Install from https://ollama.com/download, then:
ollama pull llama3.2
```

Verify it is running:

```powershell
ollama list
# Should show: llama3.2   ...
```

> Ollama listens on `localhost:11434` by default. The Docker container reaches it via `host.docker.internal:11434`.

### Running the agent

```powershell
docker compose exec api conch-agent \
  --input  /workspace/examples/memory.conch \
  --output /workspace/examples/synthesis.conch
```

The agent will:
1. Read and validate the input conch
2. Build a structured prompt from the schema + data
3. Send the prompt to llama3.2 via Ollama
4. Write the LLM's insight as a new conch to the output path

**Options:**

| Flag | Default | Description |
| --- | --- | --- |
| `--input` | required | Path to the source `.conch` file |
| `--output` | required | Path where the synthesis conch will be written |
| `--model` | `llama3.2` | Any Ollama model you have pulled |
| `--ollama` | `http://host.docker.internal:11434` | Ollama base URL |
| `--agent-id` | `conch-agent-v1` | Identity stamped as `creator` in the output conch |

**Example output (`synthesis.conch`):**

```json
{
  "meta": { "creator": "conch-agent-v1", "conch_version": "1.0" },
  "data": {
    "title": "Synthesis: Agent Memory Experiment",
    "synthesis": "Shared representations between agents act as an implicit communication channel, enabling 40% efficiency gains without explicit coordination protocols.",
    "source_conch": "<id of the input conch>",
    "model": "llama3.2",
    "tags": ["agent", "synthesis", "local"]
  }
}
```

The output is a fully valid ConchObject — it carries its own ID, creator, schema, permissions, and history entry. Another agent or human can pick it up with no extra context.

> **Windows note:** run all `docker compose exec` commands from PowerShell, not Git Bash. Git Bash mangles `/workspace` paths.

---

## Roadmap

| Milestone              | Status  | Description                                    |
| ---------------------- | ------- | ---------------------------------------------- |
| M1 — Parser            | Done    | Parse, validate, build ConchObjects            |
| M2 — Writer            | Done    | Canonical serialization + round-trip guarantee |
| M3 — Reference Library | Done    | CLI tool, example conches, schema templates    |
| M4 — Agent Integration | Done    | Local LLM agent loop via Ollama                |
| M5 — Storage           | Planned | Store validated ConchObjects in Postgres       |
| M6 — Signatures        | Planned | Ed25519 signing over canonical bytes           |
| M7 — Flesh             | Planned | Private encrypted memory inside a conch        |
| M8 — Pearl             | Planned | Immutable provenance history                   |

---

## The Three Layers

| Layer | Name      | Description                                             |
| ----- | --------- | ------------------------------------------------------- |
| Shell | Public    | Structured meaning — readable by anyone with permission |
| Flesh | Private   | Encrypted memory — only decryptable by the holder       |
| Pearl | Immutable | Provenance trail — cryptographically sealed history     |
