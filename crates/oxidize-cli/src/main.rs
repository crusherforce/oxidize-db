use clap::Parser;
use oxidize_core::sql;
use oxidize_core::vdbe::{CodeGen, VirtualMachine};
use std::io::{self, BufRead, Write};

/// oxidize-db — a SQLite-compatible database engine written in Rust
#[derive(Parser, Debug)]
#[command(name = "oxidize-db", version, about, long_about = None)]
struct Args {
    /// Database file to open or create (omit for in-memory / REPL mode)
    db_file: Option<String>,
}

fn main() {
    let args = Args::parse();

    let db_label = args.db_file.as_deref().unwrap_or(":memory:");

    eprintln!("oxidize-db {}", env!("CARGO_PKG_VERSION"));
    eprintln!("Connected to: {db_label}");
    eprintln!("Type SQL statements terminated by ';'. Type .quit or ^D to exit.\n");

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut buffer = String::new();

    let cg = CodeGen::new();
    let mut vm = VirtualMachine::new();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == ".quit" || trimmed == ".exit" {
            break;
        }

        buffer.push_str(trimmed);

        // Execute when the buffer contains a complete statement (ends with ';').
        if buffer.trim_end().ends_with(';') {
            let sql = buffer.trim().to_string();
            buffer.clear();

            match sql::parse(&sql) {
                Err(e) => {
                    eprintln!("Parse error: {e}");
                    continue;
                }
                Ok(stmts) => {
                    for stmt in &stmts {
                        // Attempt to compile and execute.
                        match cg.compile(stmt) {
                            Err(e) => {
                                // Compilation not yet supported for this statement type —
                                // fall back to printing the AST.
                                writeln!(out, "-- AST (execution not yet supported: {e}) --").ok();
                                writeln!(out, "{stmt:#?}").ok();
                            }
                            Ok(program) => match vm.execute(&program) {
                                Err(e) => {
                                    eprintln!("Runtime error: {e}");
                                }
                                Ok(rows) => {
                                    if rows.is_empty() {
                                        writeln!(out, "(no rows)").ok();
                                    } else {
                                        for row in &rows {
                                            let cols: Vec<String> =
                                                row.iter().map(|v| v.to_string()).collect();
                                            writeln!(out, "{}", cols.join(" | ")).ok();
                                        }
                                    }
                                }
                            },
                        }
                    }
                }
            }
        } else {
            // Multi-line input: add a space and keep reading.
            buffer.push(' ');
        }
    }
}
