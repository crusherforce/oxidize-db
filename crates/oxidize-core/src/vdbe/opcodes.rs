/// Opcodes for the Virtual Database Engine (VDBE).
///
/// These loosely mirror SQLite's VDBE instruction set. Each opcode
/// is self-contained; the VM carries a register file and a program counter.
#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    /// Initialise execution; jump to `addr` (usually past any setup).
    Init { addr: usize },

    /// Open a read cursor on table `table` using root page `root_page`.
    OpenRead {
        cursor: usize,
        root_page: u32,
        table: String,
    },

    /// Open a write cursor on table `table` using root page `root_page`.
    OpenWrite {
        cursor: usize,
        root_page: u32,
        table: String,
    },

    /// Move cursor to the first entry; branch to `addr` if empty.
    Rewind { cursor: usize, addr: usize },

    /// Read column `col` from the current cursor row into register `reg`.
    Column {
        cursor: usize,
        col: usize,
        reg: usize,
    },

    /// Emit the values in registers `start..start+count` as a result row.
    ResultRow { start: usize, count: usize },

    /// Advance cursor to the next row; loop back to `addr` if more rows.
    Next { cursor: usize, addr: usize },

    /// Store an integer literal into `reg`.
    Integer { value: i64, reg: usize },

    /// Store a string literal into `reg`.
    String { value: String, reg: usize },

    /// Store NULL into `reg`.
    Null { reg: usize },

    /// Copy `src` register value into `dest`.
    Copy { src: usize, dest: usize },

    /// Add reg[left] + reg[right] and store in reg[dest].
    Add {
        left: usize,
        right: usize,
        dest: usize,
    },

    /// Compare reg[left] with reg[right]; branch to `addr` if equal.
    Eq {
        left: usize,
        right: usize,
        addr: usize,
    },

    /// Unconditional jump to `addr`.
    Goto { addr: usize },

    /// Yield a single register value as output (for expressions like SELECT 1).
    Yield { reg: usize },

    /// Insert (reg[key], reg[data]) into cursor's table.
    Insert {
        cursor: usize,
        key_reg: usize,
        data_reg: usize,
    },

    /// Delete current row from cursor.
    Delete { cursor: usize },

    /// Halt execution (end of program).
    Halt,
}
