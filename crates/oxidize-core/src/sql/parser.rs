use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::Parser;

use super::ast::Statement;
use crate::error::OxidizeError;
use crate::Result;

/// Parse one or more SQL statements from a string.
pub fn parse_sql(sql: &str) -> Result<Vec<Statement>> {
    let dialect = SQLiteDialect {};
    Parser::parse_sql(&dialect, sql).map_err(|e| OxidizeError::ParseError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_select() {
        let stmts = parse_sql("SELECT 1;").expect("should parse");
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn test_parse_create_table() {
        let sql = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);";
        let stmts = parse_sql(sql).expect("should parse");
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn test_parse_insert() {
        let sql = "INSERT INTO users (id, name) VALUES (1, 'Alice');";
        let stmts = parse_sql(sql).expect("should parse");
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn test_parse_error() {
        let result = parse_sql("NOT VALID SQL @@@@");
        assert!(result.is_err());
    }
}
