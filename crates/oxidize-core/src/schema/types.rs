use std::fmt;

/// Column type affinities, following SQLite's type affinity rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlType {
    Integer,
    Real,
    Text,
    Blob,
    Null,
}

impl SqlType {
    /// Determine column affinity from a declared type string (SQLite rules).
    pub fn from_declared(declared: &str) -> Self {
        let upper = declared.to_ascii_uppercase();
        if upper.contains("INT") {
            SqlType::Integer
        } else if upper.contains("CHAR") || upper.contains("CLOB") || upper.contains("TEXT") {
            SqlType::Text
        } else if upper.is_empty() || upper.contains("BLOB") {
            SqlType::Blob
        } else if upper.contains("REAL") || upper.contains("FLOA") || upper.contains("DOUB") {
            SqlType::Real
        } else {
            // NUMERIC affinity — treat as Integer for now.
            SqlType::Integer
        }
    }
}

impl fmt::Display for SqlType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SqlType::Integer => write!(f, "INTEGER"),
            SqlType::Real => write!(f, "REAL"),
            SqlType::Text => write!(f, "TEXT"),
            SqlType::Blob => write!(f, "BLOB"),
            SqlType::Null => write!(f, "NULL"),
        }
    }
}

/// A runtime SQL value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
    Null,
}

impl Value {
    pub fn type_of(&self) -> SqlType {
        match self {
            Value::Integer(_) => SqlType::Integer,
            Value::Real(_) => SqlType::Real,
            Value::Text(_) => SqlType::Text,
            Value::Blob(_) => SqlType::Blob,
            Value::Null => SqlType::Null,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(n) => write!(f, "{n}"),
            Value::Real(r) => write!(f, "{r}"),
            Value::Text(s) => write!(f, "{s}"),
            Value::Blob(b) => write!(f, "<blob {} bytes>", b.len()),
            Value::Null => write!(f, "NULL"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_affinity() {
        assert_eq!(SqlType::from_declared("INTEGER"), SqlType::Integer);
        assert_eq!(SqlType::from_declared("VARCHAR(255)"), SqlType::Text);
        assert_eq!(SqlType::from_declared("DOUBLE"), SqlType::Real);
        assert_eq!(SqlType::from_declared("BLOB"), SqlType::Blob);
        assert_eq!(SqlType::from_declared(""), SqlType::Blob);
    }
}
