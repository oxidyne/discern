#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
    #[default]
    Generic,
    Postgres,
    MySql,
    Sqlite,
}

impl Dialect {
    pub fn is_keyword(self, word: &str) -> bool {
        const COMMON: &[&str] = &[
            "ALTER", "AND", "AS", "ASC", "BY", "CREATE", "DELETE", "DESC", "DISTINCT", "DROP",
            "FROM", "GROUP", "HAVING", "IN", "INSERT", "INTO", "JOIN", "LEFT", "LIMIT", "NOT",
            "NULL", "ON", "OR", "ORDER", "OUTER", "RIGHT", "SELECT", "SET", "TABLE", "UPDATE",
            "VALUES", "WHERE", "WITH", "INNER", "UNION", "ALL", "CASE", "WHEN", "THEN", "ELSE", "END",
        ];
        COMMON.iter().any(|k| k.eq_ignore_ascii_case(word))
    }
}
