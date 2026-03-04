# oxidize-db: SQLite Internals Dissection & Rebuild Plan

## Context

oxidize-db is an early-stage SQLite-compatible database engine in Rust. The scaffold exists — SQL parsing works, schema catalog exists, VDBE has 18 opcodes, B-tree is in-memory only, pager has async I/O, WAL has no checkpoint. Nothing is wired end-to-end; no real SQL queries can execute against real tables.

This plan dissects every layer of SQLite — file format, slotted pages, B-tree, pager/WAL, VDBE, SQL compiler, query optimizer, transactions — rebuilding each with correctness tests, fuzz targets, and benchmarks. The approach is bottom-up: each phase closes with a testable, non-regressed component before the next begins.

**Three non-negotiable principles:**
1. Correctness before performance. Every disk-touching data structure has documented invariants and tests.
2. No magic numbers. Every byte offset, bit mask, serial type code lives in `constants.rs`.
3. Measurement is the spec. Benchmarks are written in the same PR as implementation.

---

## Phase 0: Infrastructure Foundation

**Goal:** Test harness, benchmark harness, fuzz scaffolding. No database functionality. Prevents all future phases from shipping untestable code.

### Files to Create/Modify
- `crates/oxidize-core/src/constants.rs` — all magic values (page sizes, serial type codes, page type flags, header offsets)
- `crates/oxidize-core/src/error.rs` — extend with `CorruptPage { page_no, reason }`, `VarintOverflow { pos }`, `InvalidSerialType { code }`, `InvalidPageSize { size }`
- `crates/oxidize-core/tests/` — integration test directory
- `crates/oxidize-core/benches/harness.rs` — criterion group structure + trivial latency-floor benchmark
- `fuzz/fuzz_targets/` — cargo-fuzz directory

### Key constants to define
```rust
pub const SQLITE_MAGIC: &[u8; 16] = b"SQLite format 3\0";
pub const PAGE_SIZE_DEFAULT: u32 = 4096;
pub const DB_HEADER_SIZE: usize = 100;
pub const PAGE_TYPE_INTERIOR_INDEX: u8 = 0x02;
pub const PAGE_TYPE_INTERIOR_TABLE: u8 = 0x05;
pub const PAGE_TYPE_LEAF_INDEX: u8 = 0x0a;
pub const PAGE_TYPE_LEAF_TABLE: u8 = 0x0d;
pub const SERIAL_NULL: u64 = 0; // ... through SERIAL_ONE: u64 = 9
```

### Tests
- Smoke test: every public module importable, constants have expected values
- Error display: every error variant formats without panic

### Verification
`cargo test --workspace` green. `cargo bench --no-run` compiles. `cargo fuzz build` compiles.

---

## Phase 1: File Format Primitives

**Goal:** SQLite binary format layer — varint, record format, 100-byte DB header. Pure, deterministic, property-testable. When closed: can parse any valid SQLite file's header and any cell payload.

### Files to Create
- `crates/oxidize-core/src/format/mod.rs`
- `crates/oxidize-core/src/format/varint.rs`
- `crates/oxidize-core/src/format/serial.rs`
- `crates/oxidize-core/src/format/record.rs`
- `crates/oxidize-core/src/format/header.rs`
- `crates/oxidize-core/benches/format.rs`
- `fuzz/fuzz_targets/fuzz_varint.rs`
- `fuzz/fuzz_targets/fuzz_record.rs`

### Key Implementations

**varint.rs** — SQLite varint (not LEB128): high bit set = more bytes follow; 9th byte uses all 8 bits (no sign extension quirk).
```rust
pub fn decode(buf: &[u8], offset: usize) -> Result<(i64, usize), DbError>; // bytes_consumed in [1, 9]
pub fn encode(buf: &mut [u8], offset: usize, value: i64) -> Result<usize, DbError>;
pub fn encoded_len(value: i64) -> usize;
```

**serial.rs** — Serial type codes → Rust types. Critical: `ConstZero` (code 8) and `ConstOne` (code 9) have **zero bytes of payload** — many implementations get this wrong.
```rust
pub enum SerialType { Null, Int(IntWidth), Float, ConstZero, ConstOne, Blob(u32), Text(u32) }
pub enum IntWidth { I8, I16, I24, I32, I48, I64 }
impl Value {
    pub fn serial_type(&self) -> SerialType; // picks most compact encoding
    pub fn write_payload(&self, buf: &mut [u8]) -> Result<usize, DbError>;
    pub fn read_payload(st: &SerialType, buf: &[u8]) -> Result<Self, DbError>;
}
```

**record.rs** — Record = `[header_size_varint][type_varint...][payload...]`. The header_size includes the varint encoding of itself — creates a subtle fixed-point calculation when column count pushes header_size across a varint boundary.
```rust
pub struct Record { pub values: Vec<Value> }
impl Record {
    pub fn encode(&self) -> Result<Vec<u8>, DbError>;
    pub fn decode(buf: &[u8]) -> Result<Self, DbError>;
    pub fn encoded_len(&self) -> usize;
}
```

**header.rs** — 100-byte DB file header. Every field has a named offset constant. Must cross-check `db_size_pages` field (bytes 28-31) against actual file size.
```rust
pub struct DbHeader {
    pub page_size: u32,        // stored as u16; value 1 means 65536
    pub file_format_write: u8, // must be 1 (rollback) or 2 (WAL)
    pub file_change_counter: u32,
    pub db_size_pages: u32,
    pub freelist_trunk: u32,
    pub schema_cookie: u32,
    pub text_encoding: TextEncoding,
    // ... all 100 bytes per spec
}
impl DbHeader {
    pub fn read(buf: &[u8; 100]) -> Result<Self, DbError>;
    pub fn write(&self, buf: &mut [u8; 100]);
    pub fn validate(&self) -> Result<(), DbError>;
    pub fn effective_page_size(&self) -> u32; // handles u16=1 → 65536
}
```

### Tests
- Varint roundtrip: all 1-byte values, boundary values (128, 2^14-1, 2^21-1, i64::MAX, i64::MIN)
- Varint rejects truncated buffers
- `encoded_len` matches actual encode output for 10K random values
- Serial: `from_code(to_code(x)) == x` for all variants; `payload_size == 0` for Null/ConstZero/ConstOne
- Record roundtrip: single NULL, all-types, 255-column (forces multi-byte header_size varint)
- Record decode rejects buffer shorter than declared header_size
- Header roundtrip, validate rejects invalid page sizes, `effective_page_size` returns 65536 when stored=1
- **Proptest:** varint roundtrip for arbitrary i64, record roundtrip for arbitrary value vectors
- **Fuzz:** varint and record decoders never panic on arbitrary bytes
- **Golden-file:** parse Chinook SQLite DB header, assert known field values

### Benchmarks
- `bench_varint_encode/decode_{1,4,9}_byte` — parameterized by value range
- `bench_varint_decode_throughput` — 1M sequential varints from pre-built buffer
- `bench_record_{encode,decode}_{narrow,wide}` — 4 columns vs 64 columns
- `bench_record_roundtrip_narrow` — encode+decode, measure allocator pressure

---

## Phase 2: Page Layer & Slotted Page Engine

**Goal:** In-memory representation of all four SQLite B-tree page types with correct slotted page layout: cell pointer array from start, cell content growing from end. Cell formats for all 4 page types. Insertion, deletion, defragmentation, overflow pages, freelist.

Read SQLite's `btree.c`: `insertCell`, `dropCell`, `defragmentPage` for reference.

### Files to Create/Modify
- `crates/oxidize-core/src/btree/page.rs` — replace existing stub entirely
- `crates/oxidize-core/src/btree/cell.rs` — cell format encoders/decoders
- `crates/oxidize-core/src/btree/overflow.rs` — overflow page chains
- `crates/oxidize-core/src/btree/freelist.rs` — trunk/leaf freelist pages
- `crates/oxidize-core/benches/btree_page.rs`
- `fuzz/fuzz_targets/fuzz_page.rs`

### Key Implementations

**BTreePage** — owns a page-sized buffer, maintains slotted page invariants:
```rust
pub struct BTreePage {
    page_no: u32,
    data: Box<[u8]>,      // exactly page_size bytes
    page_size: u16,
    is_first_page: bool,  // page_no == 1: 100-byte DB header precedes btree header
}
impl BTreePage {
    pub fn header_offset(&self) -> usize; // 100 for page 1, 0 for all others
    pub fn read_header(&self) -> Result<BTreePageHeader, DbError>;
    pub fn write_header(&mut self, h: &BTreePageHeader);
    pub fn cell_offset(&self, i: u16) -> Result<u16, DbError>; // reads cell pointer array
    pub fn cell_data(&self, i: u16) -> Result<&[u8], DbError>;
    pub fn insert_cell(&mut self, i: u16, cell: &[u8]) -> Result<(), DbError>; // may defragment
    pub fn drop_cell(&mut self, i: u16) -> Result<(), DbError>;
    pub fn defragment(&mut self) -> Result<(), DbError>; // compact page, merge freeblocks
    pub fn usable_free_space(&self) -> u16;
    pub fn validate(&self) -> Result<(), DbError>; // full invariant check
}
```

**cell.rs** — Cell format is page-type-specific:
- Table leaf: `[payload_size varint][rowid varint][local_payload][overflow_ptr u32?]`
- Table interior: `[left_child_ptr u32][rowid varint]`
- Index leaf: `[payload_size varint][local_payload][overflow_ptr u32?]`
- Index interior: `[left_child_ptr u32][payload_size varint][local_payload][overflow_ptr u32?]`

```rust
// Critical: local payload size calculation (SQLite spec §2.3.3)
pub fn local_payload_size(total_payload: u64, page_usable_size: u32, is_leaf: bool) -> u32;
```

**overflow.rs** — Overflow page: 4-byte next-page pointer + `page_size - 4` bytes payload.

**freelist.rs** — Trunk pages pointed to by DB header `freelist_trunk`; each trunk contains array of leaf page numbers.

### Tests
- Page new/header roundtrip for all 4 page types
- Insert 1, 100, 1000 cells — all readable
- Drop middle cell — remaining cells intact, free space increased
- Drop all cells — free space equals initial usable space
- Defragment after fragmentation — contiguous free space
- Validate detects overlapping cells
- Insert past capacity → `NeedsSplit` error
- Overflow single-page, overflow chain (3 pages) — read back matches
- `local_payload_size` cross-checked against known SQLite values
- All 4 cell type roundtrips
- Freelist: allocate, exhaust trunk, free onto empty freelist creates trunk
- **Proptest:** random sequence of insert/drop operations, validate() always Ok while not over capacity
- **Golden-file:** parse a real SQLite file's page 2 with `BTreePage::from_raw`, validate() passes

### Benchmarks
- `bench_page_insert_{sequential,random}_100`
- `bench_page_cell_scan_100`
- `bench_page_defragment` — after 50% fragmentation
- `bench_page_validate`
- `bench_local_payload_size_calculation`

---

## Phase 3: Pager & Durability

**Goal:** Full pager stack — disk I/O, page cache (CLOCK-Pro), file locking (5-level protocol), rollback journal (delete mode), WAL mode with wal-index, checkpoint strategies. When closed: pages are durable across crashes.

Implement rollback journal first (simpler, linear), fully test crash recovery, then WAL.

### Files to Create/Modify
- `crates/oxidize-core/src/pager/lock.rs` — 5-level file lock (None/Shared/Reserved/Pending/Exclusive)
- `crates/oxidize-core/src/pager/cache.rs` — replace moka with CLOCK-Pro (scan-resistant; hot/cold page distinction)
- `crates/oxidize-core/src/pager/journal.rs` — rollback journal (28-byte header, atomic commit protocol)
- `crates/oxidize-core/src/pager/wal.rs` — WAL file (32-byte header + frames with CRC32 checksums + salt)
- `crates/oxidize-core/src/pager/wal_index.rs` — in-memory HashMap wal-index (page_no → highest frame_no)
- `crates/oxidize-core/src/pager/mod.rs` — replace stub; integrate all above
- `crates/oxidize-core/benches/pager.rs`

### Key Implementations

**lock.rs** — Valid transitions: None→Shared→Reserved→Pending→Exclusive (cannot skip). WAL mode uses byte-range locks on -shm instead.

**cache.rs** — CLOCK-Pro: distinguishes cold pages (accessed once) from hot pages (accessed multiple times). Cold evicted first. This protects hot pages during full-table scans — critical for realistic workloads.
```rust
pub struct PageCache { /* CLOCK-Pro with hot/cold bits */ }
impl PageCache {
    pub fn hit_rate(&self) -> f64; // for benchmark observability
}
```

**journal.rs** — Atomic commit protocol:
1. Acquire RESERVED lock → write journal header (page_count = -1) → write original pages → fsync journal
2. Acquire EXCLUSIVE lock → write modified pages → fsync database → delete journal

**wal.rs** — Frame format: `[page_no u32][commit_size u32][salt_1 u32][salt_2 u32][checksum_1 u32][checksum_2 u32][page_data]`. Commit frame has nonzero commit_size. Salts must match WAL header salts; checksums validated on every read. On corruption: return last valid frame before corrupt frame.

**wal_index.rs** — Maps page_no → highest frame_no for O(1) lookup without scanning WAL. Readers hold a `read_mark` (WAL frame count at transaction start); look up must respect the mark.
```rust
pub fn lookup(&self, page_no: u32) -> Option<u32>; // returns highest frame <= read_mark
pub fn advance_read_mark(&mut self, new_mark: u32);
pub fn checkpoint_done(&mut self, checkpoint_frame: u32);
```

**mod.rs (Pager)** — Routes: reads check WAL (WAL takes priority) then database file. Writes go to WAL in WAL mode, directly in rollback mode.
```rust
pub struct Pager { /* cache, lock, journal/wal, wal_index, state */ }
pub enum JournalMode { Delete, Wal }
pub enum CheckpointMode { Passive, Full, Restart, Truncate }
impl Pager {
    pub async fn read_page(&mut self, page_no: u32) -> Result<Arc<RwLock<Vec<u8>>>, DbError>;
    pub async fn write_page(&mut self, page_no: u32, data: Vec<u8>) -> Result<(), DbError>;
    pub async fn begin_write(&mut self) -> Result<(), DbError>;
    pub async fn commit(&mut self) -> Result<(), DbError>;
    pub async fn rollback(&mut self) -> Result<(), DbError>;
    pub async fn checkpoint(&mut self, mode: CheckpointMode) -> Result<CheckpointResult, DbError>;
}
```

### Tests
- Journal create + rollback restores all original pages
- Journal commit atomicity (journal deleted after commit)
- WAL write single frame, write multi-page transaction, read back matches
- WAL read latest version: two txns write same page, read returns newest
- WAL validate detects corruption after bit-flip, returns last valid frame before corruption
- Pager read-after-write, commit-persists, rollback-restores
- Cache eviction: fill past capacity, dirty pages flushed
- **Crash simulation:** `FaultInjector` trait, inject fsync error mid-journal → reopen finds consistent state
- WAL reader snapshot isolation: reader sees pre-commit values, post-commit reader sees new values
- PASSIVE checkpoint: WAL frames transferred to db file
- Lock transitions: valid sequence succeeds, invalid/blocked sequence fails

### Benchmarks
- `bench_page_{read,write}_{sequential,random}_{cold,warm}`
- `bench_wal_append_throughput`
- `bench_wal_checkpoint_passive_{1k,100k}` — cost vs WAL size
- `bench_cache_hit_rate_{clock_pro,lru}` — Zipfian access pattern, compare eviction quality

### Verification
1. After simulated crash (journal written, not deleted), reopen triggers recovery, all pages restored
2. Checkpoint oxidize WAL → open resulting database file with rusqlite → readable
3. All 4 checkpoint modes behave per spec

---

## Phase 4: B-Tree Engine

**Goal:** Complete B-tree on top of pager: search, insertion (with overflow + page splitting), deletion (with page merging/redistribution), range cursors. When closed: persistent, correct, measurable.

Implement insertion first, fully test, then deletion. Do not implement them together.

### Files to Create/Modify
- `crates/oxidize-core/src/btree/mod.rs` — replace stub, `BTree` struct
- `crates/oxidize-core/src/btree/node.rs` — replace stub
- `crates/oxidize-core/src/btree/search.rs` — binary search in page cell array
- `crates/oxidize-core/src/btree/cursor.rs` — replace stub, path-stack cursor
- `crates/oxidize-core/src/btree/insert.rs`
- `crates/oxidize-core/src/btree/delete.rs`
- `crates/oxidize-core/src/btree/balance.rs` — 3-sibling balancing (SQLite's approach)
- `crates/oxidize-core/benches/btree.rs`
- `fuzz/fuzz_targets/fuzz_btree.rs`

### Key Implementations

**search.rs** — Binary search within page cell pointer array. Table B-tree: compare rowid integers. Index B-tree: compare Record key tuples lexicographically. This is a hot path.

**cursor.rs** — Cursor holds a **path stack** of `(page_no, cell_index)` from root to current leaf. Moving Next does not re-traverse from root — it walks the path stack.
```rust
pub enum SeekMode { GE, GT, LE, LT }
impl BTreeCursor {
    pub async fn rewind(&mut self) -> Result<(), DbError>;
    pub async fn last(&mut self) -> Result<(), DbError>;
    pub async fn seek(&mut self, rowid: i64, mode: SeekMode) -> Result<bool, DbError>;
    pub async fn next(&mut self) -> Result<bool, DbError>;
    pub async fn prev(&mut self) -> Result<bool, DbError>;
    pub async fn rowid(&self) -> Result<i64, DbError>;
    pub async fn payload(&self) -> Result<Vec<u8>, DbError>; // follows overflow
}
```

**balance.rs** — The SQLite balancing algorithm balances **up to 3 siblings simultaneously** (not just 2), distributing cells evenly to minimize waste. This is the defining quality-of-implementation decision.
```rust
pub async fn balance(pager: &mut Pager, cursor: &mut BTreeCursor, root_page: u32) -> Result<(), DbError>;
pub async fn balance_nonroot(pager: &mut Pager, cursor: &mut BTreeCursor) -> Result<(), DbError>;
pub async fn balance_after_delete(pager: &mut Pager, cursor: &mut BTreeCursor) -> Result<(), DbError>;
pub async fn balance_root(pager: &mut Pager, root_page: u32) -> Result<(), DbError>; // height increase
```

**Invariant checker — called after every test:**
```rust
pub async fn validate_tree(pager: &mut Pager, root_page: u32) -> Result<TreeStats, DbError>;
// Checks: all leaves at same depth, no page appears twice, pointer ordering correct,
//         free space accounting consistent, overflow chains have no cycles
```

### Tests
- Insert 1, 10, 1000, 1M rows (sequential and random) — all readable, scan returns in order
- Insert causes leaf split, causes root split — tree still correct (validate_tree passes)
- Duplicate rowid → error
- SeekGE/GT/LE/LT: exact hit, between entries, past end
- Full table scan, bounded range scan, reverse scan
- Delete only entry, leaf entry, triggers merge, triggers redistribute
- Delete then insert in same slot works
- Index insert + lookup + range scan
- Overflow: insert payload > local size, read back; delete frees overflow chain
- **Proptest:** random sequence of insert/delete/get operations, tree valid after each op
- **Fuzz:** arbitrary op sequences, validate_tree never panics

### Benchmarks
- `bench_insert_{sequential,random}_{10k,100k,1m}`
- `bench_point_lookup_{10k,1m}`
- `bench_range_scan_full_100k`, `bench_range_scan_10pct_100k`
- `bench_delete_{sequential,random}_100k`
- `bench_mixed_rw_100k` — 50/50 reads/writes
- **Comparison baseline (rusqlite):** same workloads via rusqlite for calibration (not competition — for finding structural bugs)

### Verification
1. **Semantic equivalence:** Insert same 100K rows into oxidize-db and SQLite; checkpoint; open oxidize-db file with rusqlite; full scan matches.
2. **Fill factor:** After 1M sequential inserts, `validate_tree` reports avg fill factor > 0.70.
3. **Height bound:** 1M rows at page_size=4096 → tree height ≤ 4.

---

## Phase 5: VDBE / Virtual Machine

**Goal:** Complete opcode set, connect VM to B-tree cursor layer, implement all cursor opcodes.

### Files to Create/Modify
- `crates/oxidize-core/src/vdbe/register.rs` — typed register values with correct NULL/comparison semantics
- `crates/oxidize-core/src/vdbe/opcode.rs` — replace 18-opcode enum with full set
- `crates/oxidize-core/src/vdbe/executor.rs` — replace vm.rs; dispatch loop + cursor management
- `crates/oxidize-core/src/vdbe/cursor.rs` — VDBE-level cursor wrapping BTreeCursor
- `crates/oxidize-core/benches/vdbe.rs`
- `crates/oxidize-core/tests/slt/` — start populating sqllogictest files

### Key Implementations

**register.rs** — NULL propagation, integer/real promotion, TEXT/BLOB comparison must match SQLite exactly. `NULL = NULL` is NOT true. `NULL + anything` is NULL.
```rust
pub enum Register { Null, Integer(i64), Real(f64), Text(Arc<str>), Blob(Arc<[u8]>), Record(Record) }
impl Register {
    pub fn compare(&self, other: &Self) -> std::cmp::Ordering; // SQLite type affinity rules
    pub fn apply_numeric_affinity(&self) -> Self;
    // add/sub/mul/div/rem returning Result<Self, DbError>
}
```

**opcode.rs** — Full set. Key additions over existing 18:
- All cursor ops: `SeekGE/GT/LE/LT`, `SeekRowid`, `Found/NotFound/NoConflict`, `IdxInsert/IdxDelete/IdxRowid`
- Aggregation: `AggStep/AggFinal/AggValue`
- Sorting: `SorterOpen/Insert/Sort/Next/Data` (external sort for ORDER BY)
- Coroutines: `InitCoroutine/Yield/EndCoroutine` (subquery materialization)
- Subroutines: `Gosub/Return`
- Record ops: `MakeRecord`, `Cast`, `Function`
- Logic: `And/Or/Not/IsNull/NotNull`
- All comparison variants: `Eq/Ne/Lt/Le/Gt/Ge` with `jump_if_null` flag
- `OpenEphemeral` (temp in-memory table for GROUP BY/DISTINCT)

**executor.rs** — Direct match on opcode enum (Rust compiles to jump table). Avoid trait object virtual dispatch in hot path.

### Tests
Write VDBE-level tests by constructing programs manually (tests VM in isolation from compiler):
- Halt immediate, single result row, arithmetic, NULL propagation
- Comparison: Eq jumps when equal; Ne with NULL does NOT jump
- Cursor scan: empty table takes empty_jump; 10-row scan returns 10 rows
- Cursor seek/insert/delete via OpenWrite
- AggStep COUNT and SUM
- SorterSort: 100 rows random order → sorted output
- Gosub/Return, coroutine Yield

**sqllogictest integration:**
```
# tests/slt/select/literals.slt
query I
SELECT 1 + 2
----
3

# tests/slt/nulls.slt
query I
SELECT NULL IS NULL
----
1
```

### Benchmarks
- `bench_opcode_dispatch_empty_loop` — tight Goto self loop, measure dispatch overhead
- `bench_arithmetic_chain` — 100 ADD operations
- `bench_result_row_1m` — produce 1M ResultRows from in-memory cursor
- `bench_aggregate_count_100k`
- `bench_sorter_insert_sort_100k` — external sort cost

---

## Phase 6: SQL Compiler / Code Generator

**Goal:** Full AST-to-VDBE compilation for SELECT/INSERT/UPDATE/DELETE/CREATE TABLE/DROP TABLE/CREATE INDEX.

### Files to Create
- `crates/oxidize-core/src/compiler/mod.rs`
- `crates/oxidize-core/src/compiler/context.rs` — register allocator, cursor allocator, jump backpatching
- `crates/oxidize-core/src/compiler/select.rs` — SELECT (WHERE, GROUP BY, ORDER BY, HAVING, LIMIT)
- `crates/oxidize-core/src/compiler/dml.rs` — INSERT, UPDATE, DELETE
- `crates/oxidize-core/src/compiler/ddl.rs` — CREATE/DROP TABLE/INDEX
- `crates/oxidize-core/src/compiler/expr.rs` — expression compilation → VDBE opcodes
- `crates/oxidize-core/src/compiler/join.rs` — nested loop join, index join

### Key Approach: Logical Plan Layer

Introduce a minimal logical plan IR between AST and VDBE. This is the hook the optimizer (Phase 7) will transform:
```rust
pub enum LogicalPlan {
    Scan { table: TableRef, filter: Option<Expr> },
    Project { input: Box<LogicalPlan>, exprs: Vec<NamedExpr> },
    Filter { input: Box<LogicalPlan>, predicate: Expr },
    Join { left: Box<LogicalPlan>, right: Box<LogicalPlan>, condition: Expr, kind: JoinKind },
    Aggregate { input: Box<LogicalPlan>, group_by: Vec<Expr>, aggregates: Vec<AggExpr> },
    Sort { input: Box<LogicalPlan>, keys: Vec<SortKey> },
    Limit { input: Box<LogicalPlan>, count: Expr, offset: Option<Expr> },
    // Insert/Update/Delete/CreateTable/DropTable/CreateIndex variants
}
```

**context.rs:**
```rust
pub struct CompileContext<'a> {
    schema: &'a SchemaManager,
    program: Vec<Opcode>,
    next_reg: u16,
    next_cursor: u8,
    loop_stack: Vec<LoopFrame>, // for backpatching Next/jump targets
}
impl CompileContext {
    pub fn alloc_reg(&mut self) -> u16;
    pub fn emit(&mut self, op: Opcode) -> u32;   // returns addr
    pub fn patch_jump(&mut self, addr: u32, target: u32);
}
```

Implement `EXPLAIN QUERY PLAN` as text rendering of logical plan.

### Tests — sqllogictest suite
```
tests/slt/
  select/{literals, arithmetic, where_clause, order_by, group_by, having, limit_offset, subqueries}.slt
  select/joins/{inner, left, cross}.slt
  dml/{insert, update, delete}.slt
  ddl/{create_table, drop_table, create_index}.slt
  types/{integers, reals, text, nulls, blob}.slt
  aggregates/{count, sum, avg, min_max}.slt
  functions/{string, numeric}.slt
```

### Benchmarks
- Compiler throughput (parse+compile only, no execute): simple SELECT, 2-table JOIN, aggregate
- End-to-end: `bench_e2e_{point_lookup,full_scan,range_scan,insert,join}_100k`

### Verification
Pass all `select/`, `dml/`, `ddl/`, `types/` slt tests. Cross-check: run same .slt files against rusqlite, compare outputs.

---

## Phase 7: Query Optimizer

**Goal:** Cost-based optimizer transforms logical plan before code generation. Implement statistics collection (ANALYZE), cost model, index selection, join ordering (N-Nearest-Neighbors / NGQP), covering index detection, predicate pushdown.

### Files to Create
- `crates/oxidize-core/src/optimizer/mod.rs`
- `crates/oxidize-core/src/optimizer/stats.rs` — table/index statistics (sqlite_stat1 format)
- `crates/oxidize-core/src/optimizer/cost.rs` — cost model: `setup_cost + nRow * per_row_cost` (log space)
- `crates/oxidize-core/src/optimizer/join_order.rs` — N-Nearest-Neighbors algorithm
- `crates/oxidize-core/src/optimizer/rules.rs` — transformation rules

### Key Implementations

**stats.rs** — ANALYZE stores per-table, per-index statistics. `sqlite_stat1` format: `[total_rows, avg_rows_per_distinct_col1_value, avg_per_col1_col2, ...]`
```rust
pub struct IndexStats {
    pub selectivity: Vec<u64>; // selectivity[k] = avg rows with same k-column prefix
}
impl Statistics {
    pub fn analyze(pager: &mut Pager, schema: &SchemaManager) -> Result<Self, DbError>;
    pub fn save_to_catalog(&self, pager: &mut Pager) -> Result<(), DbError>;
}
```

**join_order.rs** — Greedy NNN: at each step, add the table with lowest incremental cost.
For N tables this is O(N²) vs O(N!) brute force, produces optimal plans for N≤10 in practice.
```rust
pub fn optimize_join_order(
    tables: &[TableRef], predicates: &[Expr], stats: &Statistics, schema: &SchemaManager,
) -> Vec<(TableRef, AccessPath)>;

pub enum AccessPath {
    TableScan,
    IndexScan { index_name: Arc<str>, range: Option<IndexRange> },
    IndexSeek { index_name: Arc<str>, eq_values: Vec<Value> },
}
```

**rules.rs:**
- `PushFilterBelowJoin` — predicate pushdown
- `PushFilterBelowProject`
- `EliminateRedundantSort`
- `CoveringIndexDetection` — index-only scan when all needed columns are in index
- `ConstantFolding`
- `NullPropagation`

### Tests
- Optimizer selects index for equality predicate when index exists
- Optimizer chooses table scan when index selectivity too low
- Join order: 2-table, 3-table cases under NNN
- Covering index detected → no table lookup in plan
- Constant folding: `WHERE 1 + 2 > 2` simplified
- Filter pushed below join
- Known query against known schema produces expected EXPLAIN QUERY PLAN text (regression test)

### Benchmarks — TPC-H queries (scaled ~100K rows)
```
// Each paired with rusqlite baseline for calibration
bench_tpch_q1_{oxidize,rusqlite}   // heavy aggregation
bench_tpch_q3_{oxidize,rusqlite}   // 3-table join
bench_tpch_q6_{oxidize,rusqlite}   // scan + filter + aggregate
bench_tpch_q14_{oxidize,rusqlite}  // join + aggregate + CASE
bench_write_100k_{sequential,random}_wal
bench_read_1m_point_lookup
bench_mixed_oltp_10threads         // 8 reader + 2 writer threads
```

### Verification
1. EXPLAIN QUERY PLAN for TPC-H Q6 must show index scan on `l_shipdate` — if it doesn't, selectivity estimation is broken.
2. TPC-H benchmarks: oxidize-db within 5x of rusqlite. >10x indicates structural issue.
3. sqllogictest pass rate ≥ 90% with optimizer enabled.

---

## Phase 8: Transaction System

**Goal:** DEFERRED/IMMEDIATE/EXCLUSIVE modes, savepoints, snapshot isolation via WAL read marks.

### Files to Create
- `crates/oxidize-core/src/transaction/mod.rs`
- `crates/oxidize-core/src/transaction/savepoint.rs`
- `crates/oxidize-core/src/transaction/manager.rs`

### Key Implementations

```rust
pub enum TransactionMode { Deferred, Immediate, Exclusive }

pub struct Transaction {
    mode: TransactionMode,
    read_mark: u32,          // WAL frame count at transaction start
    savepoints: Vec<Savepoint>,
}

pub struct Savepoint {
    name: Arc<str>,
    wal_frame: u32,          // WAL frame count at savepoint creation
}

pub struct TransactionManager {
    active_readers: Vec<u32>, // read_mark values of active readers
}
impl TransactionManager {
    pub fn min_active_read_mark(&self) -> Option<u32>; // checkpoint cannot proceed past this
}
```

### Tests
- DEFERRED acquires RESERVED lock only on first write (not at BEGIN)
- Two IMMEDIATE transactions: second blocks until first commits
- EXCLUSIVE blocks new readers
- Savepoint rollback: changes after SAVEPOINT undone, before survive
- Nested savepoints: ROLLBACK TO outer rolls back both
- Snapshot isolation via WAL: reader started before commit sees pre-commit values
- Checkpoint FULL waits for all readers with old read_mark to finish

### Verification
Run sqllogictest suites for transactions, adapted from SQLite's `trans.test`, `savepoint.test`, `wal.test`.

---

## Phase 9: Performance Characterization Report

**Goal:** Run complete benchmark suite, document performance model, identify bottlenecks.

### Metrics to Capture
- Throughput ratio oxidize/rusqlite per TPC-H query, with hypothesis for any gaps
- Write amplification: bytes written to WAL per byte of user data
- Read amplification: pages read per point lookup at tree heights 2, 3, 4
- Cache effectiveness: CLOCK-Pro vs LRU hit rate on Zipfian workload
- WAL checkpoint cost: frames/second at WAL size 1K/10K/100K frames
- Latency distributions: p50/p95/p99/p999 for each benchmark category

This written report drives targeted optimization in any subsequent phase — based on measurements, not guesses.

---

## Cross-Cutting Rules

1. **No `unwrap()` outside tests.** Every error propagated with `?`.
2. **Async boundary:** Only Pager and WAL are `async`. B-tree, VDBE, compiler are synchronous.
3. **Page 0 is null.** SQLite uses 1-based page numbers. Use `NonZeroU32` for page pointers where possible.
4. **Big-endian on disk.** All multi-byte integers in file format are big-endian. Never use `from_le_bytes` for file format reads.
5. **Schema cookie.** Increment on schema change. Cached schemas invalidated when cookie changes. Implement correctly in Phase 3 even before schema is complex.
6. **Phase gates.** Before starting each phase: `cargo test --workspace` green, `cargo bench` shows no regressions from previous phase, phase verification criterion documented.

---

## Sequencing

```
Phase 0: Infrastructure               (Week 1)
Phase 1: File Format Primitives       (Weeks 2-3)
Phase 2: Page Layer / Slotted Pages   (Weeks 4-6)
Phase 3: Pager & Durability           (Weeks 7-10)
Phase 4: B-Tree Engine                (Weeks 11-15)
Phase 5: VDBE / Virtual Machine       (Weeks 16-18)
Phase 6: SQL Compiler                 (Weeks 19-23)
Phase 7: Query Optimizer              (Weeks 24-28)
Phase 8: Transaction System           (Weeks 29-31)
Phase 9: Benchmark & Characterize     (Weeks 32-33)
```

---

## Critical Files for Implementation

Highest-stakes files — bugs here corrupt databases silently or produce wrong query results:

| File | Risk | Why |
|------|------|-----|
| `format/varint.rs` | High | Touches every page read/write; 9-byte max with special 9th-byte semantics |
| `btree/page.rs` | Very High | Slotted page engine; all B-tree correctness depends on cell pointer array and defragment |
| `pager/wal.rs` | Very High | WAL frame checksums + salt matching = primary durability guarantee; bugs = silent data loss |
| `btree/balance.rs` | High | 3-sibling balancing determines both correctness under splits and fill factor |
| `vdbe/executor.rs` | High | NULL propagation and comparison semantics must match SQLite exactly for sqllogictest |

---

## External References

- SQLite file format: https://www.sqlite.org/fileformat2.html
- SQLite architecture: https://www.sqlite.org/arch.html
- VDBE opcodes: https://www.sqlite.org/opcode.html
- WAL internals: https://www.sqlite.org/wal.html
- NGQP (query planner): https://www.sqlite.org/queryplanner-ng.html
- sqllogictest-rs: https://github.com/risinglightdb/sqllogictest-rs
- *Database Internals* (Petrov) — Chapters 2-5 for B-tree implementation reference
