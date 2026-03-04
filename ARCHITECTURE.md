# oxidize-db Codebase Orientation Guide

## Context
This document explains the oxidize-db codebase for a developer new to the project.
oxidize-db is a SQLite-compatible database engine in Rust (early scaffold stage).
GitHub: https://github.com/crusherforce/oxidize-db

---

## Directory Layout

```
/root/oxidize-db/
├── Cargo.toml                          ← Workspace root; declares members + shared deps
├── crates/
│   ├── oxidize-core/                   ← Library crate: all engine code
│   │   └── src/
│   │       ├── lib.rs                  ← Module declarations + Result<T> alias
│   │       ├── error.rs                ← OxidizeError enum (thiserror)
│   │       ├── sql/                    ← SQL parsing layer
│   │       │   ├── mod.rs              ← Public: parse(sql) → Vec<Statement>
│   │       │   ├── ast.rs              ← Re-exports from sqlparser::ast
│   │       │   └── parser.rs           ← Thin wrapper around sqlparser v0.53
│   │       ├── vdbe/                   ← Virtual machine (query executor)
│   │       │   ├── mod.rs              ← CodeGen: compile(stmt) → Vec<Opcode>
│   │       │   ├── opcodes.rs          ← Opcode enum (18 variants)
│   │       │   └── vm.rs               ← VirtualMachine: execute(program) → Vec<Row>
│   │       ├── btree/                  ← B-tree storage engine
│   │       │   ├── mod.rs              ← BTree: get/insert/delete/scan
│   │       │   ├── node.rs             ← BTreeNode: keys/values/children
│   │       │   ├── cursor.rs           ← Cursor: sequential scan iterator
│   │       │   └── page.rs             ← PageHeader + PageType (binary format)
│   │       ├── pager/                  ← Page cache + disk I/O
│   │       │   ├── mod.rs              ← Pager: cache-first read/write (PAGE_SIZE=4096)
│   │       │   ├── io.rs               ← FileIo: async tokio file read/write
│   │       │   └── cache.rs            ← PageCache: moka LRU (u32 → Vec<u8>)
│   │       ├── wal/
│   │       │   └── mod.rs              ← Wal: append/replay/checkpoint
│   │       └── schema/
│   │           ├── mod.rs              ← Column, TableSchema, SchemaStore (catalog)
│   │           └── types.rs            ← SqlType enum + Value enum
│   └── oxidize-cli/                    ← Binary crate: REPL
│       └── src/main.rs                 ← CLI entry point (clap + REPL loop)
└── .github/workflows/ci.yml            ← CI: build + test + clippy + fmt
```

---

## Key Types Cheat Sheet

| Type | File | Purpose |
|------|------|---------|
| `OxidizeError` | `error.rs` | Single error enum for all crate errors |
| `Result<T>` | `lib.rs` | Alias for `std::result::Result<T, OxidizeError>` |
| `Statement` | `sql/ast.rs` | Re-export of `sqlparser::ast::Statement` |
| `Value` | `schema/types.rs` | Runtime SQL value: `Integer(i64)`, `Real(f64)`, `Text(String)`, `Blob(Vec<u8>)`, `Null` |
| `SqlType` | `schema/types.rs` | Type affinity: `Integer`, `Real`, `Text`, `Blob`, `Null` |
| `Opcode` | `vdbe/opcodes.rs` | VDBE instruction set (18 variants) |
| `VirtualMachine` | `vdbe/vm.rs` | Executes `Vec<Opcode>`, holds 256-register file |
| `CodeGen` | `vdbe/mod.rs` | Compiles `Statement` → `Vec<Opcode>` |
| `BTreeNode` | `btree/node.rs` | Node: `keys: Vec<Vec<u8>>`, `values: Vec<Vec<u8>>`, `children: Vec<u32>` |
| `Cursor` | `btree/cursor.rs` | Iterator over B-tree slots |
| `PageHeader` | `btree/page.rs` | 12-byte page header (big-endian, SQLite layout) |
| `Pager` | `pager/mod.rs` | Page manager: LRU cache + async file I/O |
| `Wal` | `wal/mod.rs` | Write-ahead log entries (`tx_id`, `page_no`, `data: [u8; 4096]`) |
| `SchemaStore` | `schema/mod.rs` | In-memory `HashMap<String, TableSchema>` catalog |

---

## Data Flow: `SELECT 1;` End-to-End

```
User types: SELECT 1;
     │
     ▼
[main.rs — REPL]
  stdin lines → buffer until ';' → sql = "SELECT 1;"
     │
     ▼
[sql/parser.rs — parse()]
  sqlparser::Parser::parse_sql(&SQLiteDialect{}, sql)
  → Ok(vec![ Statement::Query(...) ])
     │
     ▼
[vdbe/mod.rs — CodeGen::compile()]
  Statement::Query → compile_query()
  → projection = [UnnamedExpr(Value(Number("1")))]
  → program = [
      Integer { value: 1, reg: 0 },
      ResultRow { start: 0, count: 1 },
      Halt,
    ]
     │
     ▼
[vdbe/vm.rs — VirtualMachine::execute()]
  pc=0: Integer{1,0}  → registers[0] = Value::Integer(1)
  pc=1: ResultRow{0,1}→ output.push([ Value::Integer(1) ])
  pc=2: Halt          → break
  → Ok(vec![ vec![ Value::Integer(1) ] ])
     │
     ▼
[main.rs — output]
  rows[0].iter().map(|v| v.to_string()).join(" | ")
  → prints: "1"
```

---

## Module Status: What Works vs What's Stubbed

### Fully Working
- **`sql/`** — Parses all standard SQL (full SQLite dialect via sqlparser)
- **`schema/types.rs`** — Type affinity + Value enum + Display
- **`schema/mod.rs`** — In-memory table catalog (SchemaStore) — not yet wired to VDBE
- **`pager/io.rs`** — Async page read/write (tokio); `PAGE_SIZE = 4096`
- **`pager/cache.rs`** — LRU page cache (moka, 256 pages default)
- **`pager/mod.rs`** — Coordinates cache + I/O
- **`btree/page.rs`** — PageHeader binary serialization (byteorder BigEndian)
- **`btree/node.rs`** — Leaf insert, search, split detection, `split_leaf()`
- **`btree/cursor.rs`** — Single-node scan, seek, advance
- **`vdbe/vm.rs`** — Scalar opcodes: `Integer`, `String`, `Null`, `Copy`, `Add`, `Eq`, `Goto`, `Yield`, `ResultRow`, `Halt`
- **REPL** (`main.rs`) — Reads SQL, parses, compiles, executes, prints

### Stubbed / Incomplete
| Component | Status | Key TODO |
|-----------|--------|----------|
| `btree/mod.rs` — persistence | Stub | Split promotion not persisted; `_pager: Option<Pager>` unused |
| `btree/cursor.rs` — multi-node | Stub | Only navigates single in-memory node |
| `wal/mod.rs` — checkpoint | Stub | `checkpoint()` only clears Vec; doesn't write to DB file |
| `vdbe/vm.rs` — cursor ops | Stub | `OpenRead`, `OpenWrite`, `Rewind`, `Column`, `Next`, `Insert`, `Delete` → all return `Unsupported` |
| `vdbe/mod.rs` (CodeGen) | Partial | `SELECT` literals only; no `FROM`, no `CREATE TABLE`, no `INSERT` |
| Schema ↔ VDBE | Not wired | `SchemaStore` exists but CodeGen doesn't consult it |

---

## The 22 Unit Tests

Run with: `cargo test --all`

| Module | Test | What it checks |
|--------|------|----------------|
| `sql/parser` | `test_parse_select` | `SELECT 1;` parses to 1 statement |
| `sql/parser` | `test_parse_create_table` | `CREATE TABLE` with constraints |
| `sql/parser` | `test_parse_insert` | `INSERT INTO ... VALUES` |
| `sql/parser` | `test_parse_error` | Invalid SQL returns `ParseError` |
| `btree/page` | `test_round_trip_header` | Serialize → deserialize `PageHeader` |
| `btree/node` | `test_insert_and_search` | Insert key, retrieve value |
| `btree/node` | `test_insert_sorted` | Keys stay sorted after insertions |
| `btree/cursor` | `test_advance` | Sequential slot traversal |
| `btree/cursor` | `test_seek` | Seek to target key |
| `btree` | `test_insert_get` | BTree.insert + BTree.get |
| `btree` | `test_delete` | BTree.delete removes key |
| `btree` | `test_delete_not_found` | BTree.delete returns `KeyNotFound` |
| `wal` | `test_append_and_replay` | begin_tx → append → replay |
| `wal` | `test_checkpoint_clears_entries` | checkpoint() clears WAL |
| `schema` | `test_create_and_get_table` | Create table, find column |
| `schema` | `test_duplicate_table_error` | Duplicate table → SchemaError |
| `schema/types` | `test_affinity` | Type affinity detection rules |
| `vdbe/vm` | `test_integer_yield` | Integer → Yield → row output |
| `vdbe/vm` | `test_add` | Add two integers |
| `vdbe/vm` | `test_result_row` | Multi-register result row |
| `vdbe` | `test_compile_select_1` | `SELECT 1` compiles with Halt |
| `vdbe` | `test_compile_select_string` | `SELECT 'hello'` emits String opcode |

---

## Dependencies (Key ones)

| Crate | Version | Used for |
|-------|---------|---------|
| `sqlparser` | 0.53 | Full SQL parsing with SQLite dialect |
| `thiserror` | 1 | Derive macros for `OxidizeError` |
| `byteorder` | 1 | Big-endian binary I/O for page headers |
| `moka` | 0.12 (sync) | LRU page cache |
| `tokio` | 1 (full) | Async file I/O in pager |
| `clap` | 4 (derive) | CLI argument parsing |

---

## How to Navigate the Code

**Starting point:** `crates/oxidize-core/src/lib.rs` — lists all 7 modules.

**To understand execution:** Read `vdbe/vm.rs:execute()` — it's the core loop.

**To understand storage format:** Read `btree/page.rs` (binary format) then `pager/mod.rs` (how pages are managed).

**To extend the engine:** The natural next steps are in this order:
1. Wire `SchemaStore` into `CodeGen` (schema lookup during compilation)
2. Implement `OpenRead`/`Column`/`Next`/`Rewind` in `vm.rs` using `BTree::scan()`
3. Connect `BTree` to `Pager` for persistence (node serialization)
4. Complete WAL `checkpoint()` to flush entries via Pager
5. Add `CREATE TABLE` and `INSERT` to CodeGen

**Quick smoke test:**
```bash
cargo build --all
echo "SELECT 1; SELECT 'hello', 42;" | ./target/debug/oxidize-db
# Output:
# 1
# hello | 42
```
