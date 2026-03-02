pub mod types;

pub use types::{SqlType, Value};

use crate::error::OxidizeError;
use crate::Result;
use std::collections::HashMap;

/// Metadata for a single column.
#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub sql_type: SqlType,
    pub nullable: bool,
    pub primary_key: bool,
}

impl Column {
    pub fn new(name: impl Into<String>, sql_type: SqlType) -> Self {
        Self {
            name: name.into(),
            sql_type,
            nullable: true,
            primary_key: false,
        }
    }
}

/// Metadata for a single table.
#[derive(Debug, Clone)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<Column>,
    /// Index into `columns` of the primary key column, if any.
    pub primary_key_index: Option<usize>,
}

impl TableSchema {
    pub fn new(name: impl Into<String>, columns: Vec<Column>) -> Self {
        let primary_key_index = columns.iter().position(|c| c.primary_key);
        Self {
            name: name.into(),
            columns,
            primary_key_index,
        }
    }

    pub fn column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == name)
    }

    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }
}

/// In-memory catalog of all table schemas.
#[derive(Debug, Default)]
pub struct SchemaStore {
    tables: HashMap<String, TableSchema>,
}

impl SchemaStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_table(&mut self, schema: TableSchema) -> Result<()> {
        let name = schema.name.clone();
        if self.tables.contains_key(&name) {
            return Err(OxidizeError::SchemaError(format!(
                "table '{name}' already exists"
            )));
        }
        self.tables.insert(name, schema);
        Ok(())
    }

    pub fn get_table(&self, name: &str) -> Option<&TableSchema> {
        self.tables.get(name)
    }

    pub fn drop_table(&mut self, name: &str) -> Result<()> {
        if self.tables.remove(name).is_none() {
            return Err(OxidizeError::SchemaError(format!(
                "table '{name}' does not exist"
            )));
        }
        Ok(())
    }

    pub fn table_names(&self) -> Vec<&str> {
        self.tables.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get_table() {
        let mut store = SchemaStore::new();
        let schema = TableSchema::new(
            "users",
            vec![
                Column::new("id", SqlType::Integer),
                Column::new("name", SqlType::Text),
            ],
        );
        store.create_table(schema).unwrap();
        let t = store.get_table("users").unwrap();
        assert_eq!(t.columns.len(), 2);
        assert_eq!(t.column("name").unwrap().sql_type, SqlType::Text);
    }

    #[test]
    fn test_duplicate_table_error() {
        let mut store = SchemaStore::new();
        let schema = TableSchema::new("foo", vec![Column::new("x", SqlType::Integer)]);
        store.create_table(schema.clone()).unwrap();
        let result = store.create_table(schema);
        assert!(result.is_err());
    }
}
