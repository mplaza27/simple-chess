# Startup Guide

## Quick start (one command)

```bash
./startup.sh
```

Then open: **http://localhost:8080**

---

## Manual steps

### 1. Build the WASM bundle

```bash
PATH="$PATH:$HOME/.cargo/bin" trunk build --release
```

Only needed when code changes. The `dist/` folder is reused between runs.

### 2. Serve on localhost:8080

```bash
python3 serve.py
```

Serves `dist/` with gzip compression (WASM ~200 KB instead of 1.2 MB).

---

## Run tests (no browser needed)

```bash
~/.cargo/bin/cargo test
```
