# oxidize-db

A SQLite-compatible database engine written in Rust.

> **Status:** Early scaffold — the core architecture is in place and compiles cleanly.
> Full SQL execution, persistent B-tree storage, and WAL recovery are actively being built.

---

## Architecture

```
oxidize-db
├── crates/
│   ├── oxidize-core/          # Engine library
│   │   └── src/
│   │       ├── sql/           # Lexer → Parser → AST  (sqlparser-rs)
│   │       ├── vdbe/          # Virtual machine + code generator
│   │       ├── btree/         # B-tree nodes, cursors, page layout
│   │       ├── pager/         # Page cache (moka LRU) + async disk I/O
│   │       ├── wal/           # Write-ahead log + checkpointing
│   │       └── schema/        # Table/column metadata + type system
│   └── oxidize-cli/           # REPL binary
└── .github/workflows/ci.yml   # GitHub Actions CI
```

---

## Build

```bash
# Requires Rust ≥ 1.70 (stable)
cargo build --all
cargo test --all
cargo clippy --all -- -D warnings
```

## Run the REPL

```bash
# In-memory (no file)
cargo run --bin oxidize-db

# Persistent file
cargo run --bin oxidize-db -- mydb.db
```

Then type SQL:

```
oxidize-db 0.1.0
Connected to: :memory:
Type SQL statements terminated by ';'. Type .quit or ^D to exit.

SELECT 1;
1
SELECT 'hello', 42;
hello | 42
```

---

## Roadmap

| Component | Module | Status |
|-----------|--------|--------|
| SQL parser | `sql/` | ✅ Complete (via sqlparser-rs) |
| Type system & schema catalog | `schema/` | ✅ Scaffolded |
| B-tree (in-memory) | `btree/` | ✅ Scaffolded |
| Page cache (LRU) | `pager/cache` | ✅ Scaffolded |
| Async disk I/O | `pager/io` | ✅ Scaffolded |
| Write-ahead log | `wal/` | ✅ Scaffolded |
| VDBE opcodes | `vdbe/opcodes` | ✅ Defined |
| VDBE execution (scalars) | `vdbe/vm` | ✅ Working |
| VDBE code generation | `vdbe/` | 🔧 Partial (literals only) |
| B-tree persistence | `btree/` + `pager/` | 🔧 In progress |
| Full DML execution | `vdbe/` + `btree/` | 🔜 Planned |
| WAL recovery | `wal/` | 🔜 Planned |
| ACID transactions | all | 🔜 Planned |
| SQLite wire compatibility | — | 🔜 Planned |

---

## Contributing

1. Fork the repo and create a feature branch.
2. Ensure `cargo test --all` and `cargo clippy --all -- -D warnings` pass.
3. Open a pull request with a clear description of the change.

All contributions are welcome — from fixing typos to implementing full B-tree persistence!

---

## License

MIT — see [LICENSE](LICENSE).
