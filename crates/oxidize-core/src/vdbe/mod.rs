pub mod opcodes;
pub mod vm;

pub use opcodes::Opcode;
pub use vm::{Row, VirtualMachine};

use crate::error::OxidizeError;
use crate::sql::ast::Statement;
use crate::Result;

/// Stub code generator: compiles a parsed SQL statement into a VDBE program.
///
/// Currently handles only `SELECT <literal>` queries. Full DML/DDL support
/// requires schema resolution, table scans, and expression evaluation.
pub struct CodeGen;

impl CodeGen {
    pub fn new() -> Self {
        Self
    }

    /// Compile a single SQL statement into a sequence of opcodes.
    pub fn compile(&self, stmt: &Statement) -> Result<Vec<Opcode>> {
        match stmt {
            Statement::Query(query) => self.compile_query(query),
            Statement::CreateTable { .. } => Err(OxidizeError::Unsupported(
                "CREATE TABLE not yet implemented in CodeGen".into(),
            )),
            Statement::Insert { .. } => Err(OxidizeError::Unsupported(
                "INSERT not yet implemented in CodeGen".into(),
            )),
            Statement::Drop { .. } => Err(OxidizeError::Unsupported(
                "DROP not yet implemented in CodeGen".into(),
            )),
            _ => Err(OxidizeError::Unsupported(format!(
                "statement type not yet supported: {stmt}"
            ))),
        }
    }

    fn compile_query(&self, query: &sqlparser::ast::Query) -> Result<Vec<Opcode>> {
        use sqlparser::ast::{Expr, SelectItem, SetExpr, Value as SqlVal};

        let body = match query.body.as_ref() {
            SetExpr::Select(s) => s,
            _ => {
                return Err(OxidizeError::Unsupported(
                    "only simple SELECT bodies are supported".into(),
                ))
            }
        };

        // Only handle `SELECT <literal>, ...` (no FROM clause) for now.
        if !body.from.is_empty() {
            return Err(OxidizeError::Unsupported(
                "SELECT with FROM not yet implemented in CodeGen".into(),
            ));
        }

        let mut program = Vec::new();
        let mut reg = 0usize;
        let start = reg;
        let mut count = 0usize;

        for item in &body.projection {
            match item {
                SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
                    match expr {
                        Expr::Value(v) => match v {
                            SqlVal::Number(n, _) => {
                                if let Ok(i) = n.parse::<i64>() {
                                    program.push(Opcode::Integer { value: i, reg });
                                } else if let Ok(f) = n.parse::<f64>() {
                                    // Store floats as their bit representation in an Integer reg
                                    // for simplicity; full impl would use a Real register type.
                                    let _ = f;
                                    program.push(Opcode::String {
                                        value: n.to_string(),
                                        reg,
                                    });
                                }
                            }
                            SqlVal::SingleQuotedString(s) | SqlVal::DoubleQuotedString(s) => {
                                program.push(Opcode::String {
                                    value: s.clone(),
                                    reg,
                                });
                            }
                            SqlVal::Null => {
                                program.push(Opcode::Null { reg });
                            }
                            _ => {
                                return Err(OxidizeError::Unsupported(
                                    "unsupported literal type in SELECT".into(),
                                ))
                            }
                        },
                        _ => {
                            return Err(OxidizeError::Unsupported(
                                "only literal expressions are supported in SELECT".into(),
                            ))
                        }
                    }
                    reg += 1;
                    count += 1;
                }
                SelectItem::Wildcard(_) => {
                    return Err(OxidizeError::Unsupported(
                        "SELECT * requires a FROM clause".into(),
                    ))
                }
                SelectItem::QualifiedWildcard(_, _) => {
                    return Err(OxidizeError::Unsupported(
                        "qualified wildcard not supported".into(),
                    ))
                }
            }
        }

        program.push(Opcode::ResultRow { start, count });
        program.push(Opcode::Halt);
        Ok(program)
    }
}

impl Default for CodeGen {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql::parse;

    #[test]
    fn test_compile_select_1() {
        let stmts = parse("SELECT 1;").unwrap();
        let cg = CodeGen::new();
        let program = cg.compile(&stmts[0]).unwrap();
        assert!(!program.is_empty());
        assert!(program.contains(&Opcode::Halt));
    }

    #[test]
    fn test_compile_select_string() {
        let stmts = parse("SELECT 'hello';").unwrap();
        let cg = CodeGen::new();
        let program = cg.compile(&stmts[0]).unwrap();
        assert!(program
            .iter()
            .any(|op| matches!(op, Opcode::String { value, .. } if value == "hello")));
    }
}
