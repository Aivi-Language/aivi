use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Mutex;

const META_TABLE: &str = "aivi_tables";
const QUERY_STORAGE_PREFIX: &str = "__aivi_query_storage_";
pub const EMPTY_ROWS_JSON: &str = "{\"t\":\"List\",\"v\":[]}";

pub fn query_storage_name(logical_name: &str) -> String {
    format!("{QUERY_STORAGE_PREFIX}{logical_name}")
}

pub type LoadTableRow = (i64, String, String);

#[derive(Clone, Copy, Debug)]
pub enum Driver {
    Sqlite,
    Postgresql,
    Mysql,
}

#[derive(Clone, Debug)]
pub enum QueryCell {
    Null,
    Int(i64),
    Float(f64),
    Bool(bool),
    Text(String),
}

#[derive(Clone, Copy, Debug)]
pub enum QueryColumnType {
    Int,
    Bool,
    Float,
    Text,
    Timestamp,
}

#[derive(Clone, Debug)]
pub struct QueryColumn {
    pub name: String,
    pub column_type: QueryColumnType,
    pub not_null: bool,
}

#[derive(Clone, Debug)]
pub struct QueryRow {
    pub row_ordinal: i64,
    pub values: Vec<QueryCell>,
}

#[derive(Clone, Debug)]
pub struct QueryTable {
    pub name: String,
    pub columns: Vec<QueryColumn>,
    pub rows: Vec<QueryRow>,
}

type DbResp<T> = mpsc::Sender<Result<T, String>>;

enum DbRequest {
    OpenConnection {
        driver: Driver,
        url: String,
        resp: DbResp<u64>,
    },
    CloseConnection {
        connection_id: u64,
        resp: DbResp<()>,
    },
    EnsureSchema {
        connection_id: u64,
        resp: DbResp<()>,
    },
    LoadTable {
        connection_id: u64,
        name: String,
        resp: DbResp<Option<LoadTableRow>>,
    },
    MigrateTable {
        connection_id: u64,
        name: String,
        columns_json: String,
        resp: DbResp<()>,
    },
    CompareAndSwapRows {
        connection_id: u64,
        name: String,
        expected_rev: i64,
        columns_json: String,
        rows_json: String,
        resp: DbResp<i64>,
    },
    SqliteConfigure {
        connection_id: u64,
        wal: bool,
        busy_timeout_ms: i64,
        resp: DbResp<()>,
    },
    BeginTransaction {
        connection_id: u64,
        resp: DbResp<()>,
    },
    CommitTransaction {
        connection_id: u64,
        resp: DbResp<()>,
    },
    RollbackTransaction {
        connection_id: u64,
        resp: DbResp<()>,
    },
    Savepoint {
        connection_id: u64,
        name: String,
        resp: DbResp<()>,
    },
    ReleaseSavepoint {
        connection_id: u64,
        name: String,
        resp: DbResp<()>,
    },
    RollbackToSavepoint {
        connection_id: u64,
        name: String,
        resp: DbResp<()>,
    },
    RunMigrationSql {
        connection_id: u64,
        statements: Vec<String>,
        resp: DbResp<()>,
    },
    SyncQueryTable {
        connection_id: u64,
        table: QueryTable,
        resp: DbResp<()>,
    },
    QuerySql {
        connection_id: u64,
        sql: String,
        resp: DbResp<Vec<Vec<QueryCell>>>,
    },
}

#[derive(Clone)]
pub struct DbConnection {
    handle: DbHandle,
    connection_id: u64,
}

impl DbConnection {
    fn request<T>(
        &self,
        req: impl FnOnce(u64, mpsc::Sender<Result<T, String>>) -> DbRequest,
    ) -> Result<T, String> {
        self.handle.request(|resp| req(self.connection_id, resp))
    }

    pub fn close(&self) -> Result<(), String> {
        self.request(|connection_id, resp| DbRequest::CloseConnection {
            connection_id,
            resp,
        })
    }

    pub fn ensure_schema(&self) -> Result<(), String> {
        self.request(|connection_id, resp| DbRequest::EnsureSchema {
            connection_id,
            resp,
        })
    }

    pub fn load_table(&self, name: String) -> Result<Option<LoadTableRow>, String> {
        self.request(|connection_id, resp| DbRequest::LoadTable {
            connection_id,
            name,
            resp,
        })
    }

    pub fn migrate_table(&self, name: String, columns_json: String) -> Result<(), String> {
        self.request(|connection_id, resp| DbRequest::MigrateTable {
            connection_id,
            name,
            columns_json,
            resp,
        })
    }

    pub fn compare_and_swap_rows(
        &self,
        name: String,
        expected_rev: i64,
        columns_json: String,
        rows_json: String,
    ) -> Result<i64, String> {
        self.request(|connection_id, resp| DbRequest::CompareAndSwapRows {
            connection_id,
            name,
            expected_rev,
            columns_json,
            rows_json,
            resp,
        })
    }

    pub fn sqlite_configure(&self, wal: bool, busy_timeout_ms: i64) -> Result<(), String> {
        self.request(|connection_id, resp| DbRequest::SqliteConfigure {
            connection_id,
            wal,
            busy_timeout_ms,
            resp,
        })
    }

    pub fn begin_transaction(&self) -> Result<(), String> {
        self.request(|connection_id, resp| DbRequest::BeginTransaction {
            connection_id,
            resp,
        })
    }

    pub fn commit_transaction(&self) -> Result<(), String> {
        self.request(|connection_id, resp| DbRequest::CommitTransaction {
            connection_id,
            resp,
        })
    }

    pub fn rollback_transaction(&self) -> Result<(), String> {
        self.request(|connection_id, resp| DbRequest::RollbackTransaction {
            connection_id,
            resp,
        })
    }

    pub fn savepoint(&self, name: String) -> Result<(), String> {
        self.request(|connection_id, resp| DbRequest::Savepoint {
            connection_id,
            name,
            resp,
        })
    }

    pub fn release_savepoint(&self, name: String) -> Result<(), String> {
        self.request(|connection_id, resp| DbRequest::ReleaseSavepoint {
            connection_id,
            name,
            resp,
        })
    }

    pub fn rollback_to_savepoint(&self, name: String) -> Result<(), String> {
        self.request(|connection_id, resp| DbRequest::RollbackToSavepoint {
            connection_id,
            name,
            resp,
        })
    }

    pub fn run_migration_sql(&self, statements: Vec<String>) -> Result<(), String> {
        self.request(|connection_id, resp| DbRequest::RunMigrationSql {
            connection_id,
            statements,
            resp,
        })
    }

    pub fn sync_query_table(&self, table: QueryTable) -> Result<(), String> {
        self.request(|connection_id, resp| DbRequest::SyncQueryTable {
            connection_id,
            table,
            resp,
        })
    }

    pub fn query_sql(&self, sql: String) -> Result<Vec<Vec<QueryCell>>, String> {
        self.request(|connection_id, resp| DbRequest::QuerySql {
            connection_id,
            sql,
            resp,
        })
    }
}

#[derive(Clone)]
pub struct DbHandle {
    tx: mpsc::Sender<DbRequest>,
}

impl DbHandle {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<DbRequest>();
        std::thread::spawn(move || db_worker(rx));
        Self { tx }
    }

    fn request<T>(
        &self,
        req: impl FnOnce(mpsc::Sender<Result<T, String>>) -> DbRequest,
    ) -> Result<T, String> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.tx
            .send(req(resp_tx))
            .map_err(|_| "database backend worker stopped".to_string())?;
        resp_rx
            .recv()
            .map_err(|_| "database backend worker stopped".to_string())?
    }

    pub fn connect(&self, driver: Driver, url: String) -> Result<DbConnection, String> {
        let connection_id = self.request(|resp| DbRequest::OpenConnection { driver, url, resp })?;
        Ok(DbConnection {
            handle: self.clone(),
            connection_id,
        })
    }
}

impl Default for DbHandle {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DatabaseState {
    default_connection: Mutex<Option<DbConnection>>,
    handle: DbHandle,
}

impl DatabaseState {
    pub fn new() -> Self {
        Self {
            default_connection: Mutex::new(None),
            handle: DbHandle::new(),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.default_connection
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    pub fn configure(&self, driver: Driver, url: String) -> Result<(), String> {
        let connection = self.connect(driver, url)?;
        let previous = {
            let mut guard = self
                .default_connection
                .lock()
                .map_err(|_| "database state poisoned".to_string())?;
            guard.replace(connection)
        };
        if let Some(previous) = previous {
            previous.close()?;
        }
        Ok(())
    }

    pub fn connect(&self, driver: Driver, url: String) -> Result<DbConnection, String> {
        let connection = self.handle.connect(driver, url)?;
        connection.ensure_schema()?;
        Ok(connection)
    }

    pub fn default_connection(&self) -> Result<Option<DbConnection>, String> {
        self.default_connection
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| "database state poisoned".to_string())
    }

    pub fn handle(&self) -> &DbHandle {
        &self.handle
    }
}

impl Default for DatabaseState {
    fn default() -> Self {
        Self::new()
    }
}

fn db_worker(rx: mpsc::Receiver<DbRequest>) {
    use mysql::prelude::*;
    use rusqlite::OptionalExtension;

    enum Backend {
        Sqlite(rusqlite::Connection),
        Postgresql(Box<postgres::Client>),
        Mysql(mysql::Conn),
    }

    fn backend_err(ctx: &str, err: impl std::fmt::Display) -> String {
        format!("{ctx}: {err}")
    }

    fn is_sql_identifier(name: &str) -> bool {
        !name.is_empty()
            && name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    }

    fn require_sql_identifier(name: &str, ctx: &str) -> Result<(), String> {
        if is_sql_identifier(name) {
            Ok(())
        } else {
            Err(format!("{ctx}: invalid SQL identifier '{name}'"))
        }
    }

    fn escape_sql_text(value: &str) -> String {
        value.replace('\'', "''")
    }

    fn sql_type_name(column_type: QueryColumnType) -> &'static str {
        match column_type {
            QueryColumnType::Int => "BIGINT",
            QueryColumnType::Bool => "BOOLEAN",
            QueryColumnType::Float => "DOUBLE PRECISION",
            QueryColumnType::Text | QueryColumnType::Timestamp => "TEXT",
        }
    }

    fn sql_literal(cell: &QueryCell) -> String {
        match cell {
            QueryCell::Null => "NULL".to_string(),
            QueryCell::Int(value) => value.to_string(),
            QueryCell::Float(value) => value.to_string(),
            QueryCell::Bool(value) => {
                if *value {
                    "TRUE".to_string()
                } else {
                    "FALSE".to_string()
                }
            }
            QueryCell::Text(value) => format!("'{}'", escape_sql_text(value)),
        }
    }

    fn query_storage_identifier(name: &str, ctx: &str) -> Result<String, String> {
        require_sql_identifier(name, ctx)?;
        let storage_name = query_storage_name(name);
        require_sql_identifier(&storage_name, ctx)?;
        Ok(storage_name)
    }

    fn create_query_storage_table_sql(table: &QueryTable) -> Result<String, String> {
        require_sql_identifier(&table.name, "database.query.create_table")?;
        let storage_name = query_storage_identifier(&table.name, "database.query.create_table")?;
        let mut parts = vec!["__aivi_ord BIGINT NOT NULL".to_string()];
        for column in &table.columns {
            require_sql_identifier(&column.name, "database.query.create_table")?;
            let mut part = format!("{} {}", column.name, sql_type_name(column.column_type));
            if column.not_null {
                part.push_str(" NOT NULL");
            }
            parts.push(part);
        }
        Ok(format!(
            "CREATE TABLE {} ({})",
            storage_name,
            parts.join(", ")
        ))
    }

    fn create_query_view_sql(table: &QueryTable) -> Result<String, String> {
        require_sql_identifier(&table.name, "database.query.create_view")?;
        let storage_name = query_storage_identifier(&table.name, "database.query.create_view")?;
        let mut select_columns = Vec::with_capacity(table.columns.len());
        for column in &table.columns {
            require_sql_identifier(&column.name, "database.query.create_view")?;
            select_columns.push(column.name.clone());
        }
        Ok(format!(
            "CREATE VIEW {} AS SELECT {} FROM {}",
            table.name,
            select_columns.join(", "),
            storage_name
        ))
    }

    fn insert_row_sql(table: &QueryTable, row: &QueryRow) -> Result<String, String> {
        require_sql_identifier(&table.name, "database.query.insert")?;
        let storage_name = query_storage_identifier(&table.name, "database.query.insert")?;
        if row.values.len() != table.columns.len() {
            return Err(format!(
                "database.query.insert: row for '{}' has {} values but schema has {} columns",
                table.name,
                row.values.len(),
                table.columns.len()
            ));
        }
        let mut column_names = vec!["__aivi_ord".to_string()];
        let mut value_parts = vec![row.row_ordinal.to_string()];
        for (column, value) in table.columns.iter().zip(row.values.iter()) {
            require_sql_identifier(&column.name, "database.query.insert")?;
            column_names.push(column.name.clone());
            value_parts.push(sql_literal(value));
        }
        Ok(format!(
            "INSERT INTO {} ({}) VALUES ({})",
            storage_name,
            column_names.join(", "),
            value_parts.join(", ")
        ))
    }

    fn sqlite_cell_from_value_ref(value: rusqlite::types::ValueRef<'_>) -> QueryCell {
        match value {
            rusqlite::types::ValueRef::Null => QueryCell::Null,
            rusqlite::types::ValueRef::Integer(value) => QueryCell::Int(value),
            rusqlite::types::ValueRef::Real(value) => QueryCell::Float(value),
            rusqlite::types::ValueRef::Text(value) => {
                QueryCell::Text(String::from_utf8_lossy(value).into_owned())
            }
            rusqlite::types::ValueRef::Blob(value) => {
                QueryCell::Text(String::from_utf8_lossy(value).into_owned())
            }
        }
    }

    fn postgres_cell_from_row(row: &postgres::Row, index: usize) -> Result<QueryCell, String> {
        use postgres::types::Type;

        let ty = row.columns()[index].type_();
        if *ty == Type::BOOL {
            let value: Option<bool> = row
                .try_get(index)
                .map_err(|e| backend_err("postgres.query_row.bool", e))?;
            return Ok(value.map(QueryCell::Bool).unwrap_or(QueryCell::Null));
        }
        if *ty == Type::INT2 {
            let value: Option<i16> = row
                .try_get(index)
                .map_err(|e| backend_err("postgres.query_row.int2", e))?;
            return Ok(value
                .map(|value| QueryCell::Int(value as i64))
                .unwrap_or(QueryCell::Null));
        }
        if *ty == Type::INT4 {
            let value: Option<i32> = row
                .try_get(index)
                .map_err(|e| backend_err("postgres.query_row.int4", e))?;
            return Ok(value
                .map(|value| QueryCell::Int(value as i64))
                .unwrap_or(QueryCell::Null));
        }
        if *ty == Type::INT8 {
            let value: Option<i64> = row
                .try_get(index)
                .map_err(|e| backend_err("postgres.query_row.int8", e))?;
            return Ok(value.map(QueryCell::Int).unwrap_or(QueryCell::Null));
        }
        if *ty == Type::FLOAT4 {
            let value: Option<f32> = row
                .try_get(index)
                .map_err(|e| backend_err("postgres.query_row.float4", e))?;
            return Ok(value
                .map(|value| QueryCell::Float(value as f64))
                .unwrap_or(QueryCell::Null));
        }
        if *ty == Type::FLOAT8 {
            let value: Option<f64> = row
                .try_get(index)
                .map_err(|e| backend_err("postgres.query_row.float8", e))?;
            return Ok(value.map(QueryCell::Float).unwrap_or(QueryCell::Null));
        }

        let value: Option<String> = row
            .try_get(index)
            .map_err(|e| backend_err("postgres.query_row.text", e))?;
        Ok(value.map(QueryCell::Text).unwrap_or(QueryCell::Null))
    }

    fn mysql_cell_from_value(value: mysql::Value) -> QueryCell {
        match value {
            mysql::Value::NULL => QueryCell::Null,
            mysql::Value::Int(value) => QueryCell::Int(value),
            mysql::Value::UInt(value) => QueryCell::Int(value as i64),
            mysql::Value::Float(value) => QueryCell::Float(value as f64),
            mysql::Value::Double(value) => QueryCell::Float(value),
            mysql::Value::Bytes(value) => {
                QueryCell::Text(String::from_utf8_lossy(&value).into_owned())
            }
            mysql::Value::Date(year, month, day, hour, min, sec, micros) => {
                QueryCell::Text(format!(
                    "{year:04}-{month:02}-{day:02} {hour:02}:{min:02}:{sec:02}.{:06}",
                    micros
                ))
            }
            mysql::Value::Time(_, days, hours, mins, secs, micros) => QueryCell::Text(format!(
                "{days}:{hours:02}:{mins:02}:{secs:02}.{:06}",
                micros
            )),
        }
    }

    impl Backend {
        fn drop_logical_query_object(&mut self, name: &str) -> Result<(), String> {
            require_sql_identifier(name, "database.query.drop_logical")?;
            match self {
                Backend::Sqlite(conn) => {
                    let kind: Option<String> = conn
                        .query_row(
                            "SELECT type FROM sqlite_master WHERE name = ?1 LIMIT 1",
                            [name],
                            |row| row.get(0),
                        )
                        .optional()
                        .map_err(|e| backend_err("sqlite.query.drop_logical.lookup", e))?;
                    match kind.as_deref() {
                        Some("table") => conn
                            .execute_batch(&format!("DROP TABLE IF EXISTS {name}"))
                            .map_err(|e| backend_err("sqlite.query.drop_logical.table", e)),
                        Some("view") => conn
                            .execute_batch(&format!("DROP VIEW IF EXISTS {name}"))
                            .map_err(|e| backend_err("sqlite.query.drop_logical.view", e)),
                        Some(other) => Err(format!(
                            "database.query.drop_logical: unsupported sqlite object type '{other}'"
                        )),
                        None => Ok(()),
                    }
                }
                Backend::Postgresql(client) => {
                    let row = client
                        .query_opt(
                            "SELECT CASE \
                                WHEN c.relkind IN ('r', 'p') THEN 'table' \
                                WHEN c.relkind = 'v' THEN 'view' \
                                ELSE c.relkind::text \
                             END \
                             FROM pg_class c \
                             JOIN pg_namespace n ON n.oid = c.relnamespace \
                             WHERE c.relname = $1 AND n.nspname = ANY(current_schemas(true)) \
                             LIMIT 1",
                            &[&name],
                        )
                        .map_err(|e| backend_err("postgres.query.drop_logical.lookup", e))?;
                    match row.map(|row| row.get::<usize, String>(0)).as_deref() {
                        Some("table") => client
                            .batch_execute(&format!("DROP TABLE IF EXISTS {name}"))
                            .map_err(|e| backend_err("postgres.query.drop_logical.table", e)),
                        Some("view") => client
                            .batch_execute(&format!("DROP VIEW IF EXISTS {name}"))
                            .map_err(|e| backend_err("postgres.query.drop_logical.view", e)),
                        Some(other) => Err(format!(
                            "database.query.drop_logical: unsupported postgres object type '{other}'"
                        )),
                        None => Ok(()),
                    }
                }
                Backend::Mysql(conn) => {
                    let kind: Option<String> = conn
                        .exec_first(
                            "SELECT TABLE_TYPE FROM information_schema.TABLES \
                             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? \
                             LIMIT 1",
                            (name,),
                        )
                        .map_err(|e| backend_err("mysql.query.drop_logical.lookup", e))?;
                    match kind.as_deref() {
                        Some("BASE TABLE") => conn
                            .query_drop(format!("DROP TABLE IF EXISTS {name}"))
                            .map_err(|e| backend_err("mysql.query.drop_logical.table", e)),
                        Some("VIEW") => conn
                            .query_drop(format!("DROP VIEW IF EXISTS {name}"))
                            .map_err(|e| backend_err("mysql.query.drop_logical.view", e)),
                        Some(other) => Err(format!(
                            "database.query.drop_logical: unsupported mysql object type '{other}'"
                        )),
                        None => Ok(()),
                    }
                }
            }
        }

        fn drop_query_storage_table(&mut self, logical_name: &str) -> Result<(), String> {
            let storage_name =
                query_storage_identifier(logical_name, "database.query.drop_storage")?;
            self.execute_statement(&format!("DROP TABLE IF EXISTS {storage_name}"))
        }

        fn ensure_schema(&mut self) -> Result<(), String> {
            match self {
                Backend::Sqlite(conn) => {
                    conn.execute(
                        &format!(
                            "CREATE TABLE IF NOT EXISTS {META_TABLE} (\
                                name TEXT PRIMARY KEY,\
                                rev INTEGER NOT NULL DEFAULT 0,\
                                columns_json TEXT NOT NULL,\
                                rows_json TEXT NOT NULL\
                            )"
                        ),
                        [],
                    )
                    .map_err(|e| backend_err("sqlite.ensure_schema", e))?;
                    Ok(())
                }
                Backend::Postgresql(client) => {
                    client
                        .execute(
                            &format!(
                                "CREATE TABLE IF NOT EXISTS {META_TABLE} (\
                                    name TEXT PRIMARY KEY,\
                                    rev BIGINT NOT NULL DEFAULT 0,\
                                    columns_json TEXT NOT NULL,\
                                    rows_json TEXT NOT NULL\
                                )"
                            ),
                            &[],
                        )
                        .map_err(|e| backend_err("postgres.ensure_schema", e))?;
                    Ok(())
                }
                Backend::Mysql(conn) => {
                    conn.query_drop(format!(
                        "CREATE TABLE IF NOT EXISTS {META_TABLE} (\
                            name VARCHAR(255) PRIMARY KEY,\
                            rev BIGINT NOT NULL DEFAULT 0,\
                            columns_json LONGTEXT NOT NULL,\
                            rows_json LONGTEXT NOT NULL\
                        )"
                    ))
                    .map_err(|e| backend_err("mysql.ensure_schema", e))?;
                    Ok(())
                }
            }
        }

        fn load_table(&mut self, name: &str) -> Result<Option<(i64, String, String)>, String> {
            match self {
                Backend::Sqlite(conn) => {
                    let mut stmt = conn
                        .prepare(&format!(
                            "SELECT rev, columns_json, rows_json FROM {META_TABLE} WHERE name = ?1"
                        ))
                        .map_err(|e| backend_err("sqlite.load_table.prepare", e))?;
                    let row = stmt
                        .query_row([name], |row| {
                            let rev: i64 = row.get(0)?;
                            let columns_json: String = row.get(1)?;
                            let rows_json: String = row.get(2)?;
                            Ok((rev, columns_json, rows_json))
                        })
                        .optional()
                        .map_err(|e| backend_err("sqlite.load_table.query_row", e))?;
                    Ok(row)
                }
                Backend::Postgresql(client) => {
                    let row = client
                        .query_opt(
                            &format!(
                                "SELECT rev, columns_json, rows_json FROM {META_TABLE} WHERE name = $1"
                            ),
                            &[&name],
                        )
                        .map_err(|e| backend_err("postgres.load_table.query_opt", e))?;
                    Ok(row.map(|row| {
                        let rev: i64 = row.get::<usize, i64>(0);
                        let columns_json: String = row.get::<usize, String>(1);
                        let rows_json: String = row.get::<usize, String>(2);
                        (rev, columns_json, rows_json)
                    }))
                }
                Backend::Mysql(conn) => {
                    let row: Option<(i64, String, String)> = conn
                        .exec_first(
                            format!(
                                "SELECT rev, columns_json, rows_json FROM {META_TABLE} WHERE name = ?"
                            ),
                            (name,),
                        )
                        .map_err(|e| backend_err("mysql.load_table.exec_first", e))?;
                    Ok(row)
                }
            }
        }

        fn migrate_table(&mut self, name: &str, columns_json: &str) -> Result<(), String> {
            match self {
                Backend::Sqlite(conn) => {
                    conn.execute(
                        &format!(
                            "INSERT INTO {META_TABLE} (name, rev, columns_json, rows_json) VALUES (?1, 0, ?2, '{EMPTY_ROWS_JSON}') \
                             ON CONFLICT(name) DO UPDATE SET columns_json = excluded.columns_json"
                        ),
                        [name, columns_json],
                    )
                    .map_err(|e| backend_err("sqlite.migrate_table", e))?;
                    Ok(())
                }
                Backend::Postgresql(client) => {
                    client
                        .execute(
                            &format!(
                                "INSERT INTO {META_TABLE} (name, rev, columns_json, rows_json) VALUES ($1, 0, $2, '{EMPTY_ROWS_JSON}') \
                                 ON CONFLICT (name) DO UPDATE SET columns_json = EXCLUDED.columns_json"
                            ),
                            &[&name, &columns_json],
                        )
                        .map_err(|e| backend_err("postgres.migrate_table", e))?;
                    Ok(())
                }
                Backend::Mysql(conn) => {
                    conn.exec_drop(
                        format!(
                            "INSERT INTO {META_TABLE} (name, rev, columns_json, rows_json) VALUES (?, 0, ?, '{EMPTY_ROWS_JSON}') \
                             ON DUPLICATE KEY UPDATE columns_json = VALUES(columns_json)"
                        ),
                        (name, columns_json),
                    )
                    .map_err(|e| backend_err("mysql.migrate_table", e))?;
                    Ok(())
                }
            }
        }

        fn compare_and_swap_rows(
            &mut self,
            name: &str,
            expected_rev: i64,
            columns_json: &str,
            rows_json: &str,
        ) -> Result<i64, String> {
            match self {
                Backend::Sqlite(conn) => {
                    let changed = conn
                        .execute(
                            &format!(
                                "UPDATE {META_TABLE} SET columns_json = ?1, rows_json = ?2, rev = rev + 1 \
                                 WHERE name = ?3 AND rev = ?4"
                            ),
                            rusqlite::params![columns_json, rows_json, name, expected_rev],
                        )
                        .map_err(|e| backend_err("sqlite.cas_rows", e))?;
                    if changed == 0 {
                        return Err("concurrent write detected; retry".to_string());
                    }
                    let new_rev: i64 = conn
                        .query_row(
                            &format!("SELECT rev FROM {META_TABLE} WHERE name = ?1"),
                            [name],
                            |row| row.get(0),
                        )
                        .map_err(|e| backend_err("sqlite.cas_rows.read_rev", e))?;
                    Ok(new_rev)
                }
                Backend::Postgresql(client) => {
                    let changed = client
                        .execute(
                            &format!(
                                "UPDATE {META_TABLE} SET columns_json = $1, rows_json = $2, rev = rev + 1 \
                                 WHERE name = $3 AND rev = $4"
                            ),
                            &[&columns_json, &rows_json, &name, &expected_rev],
                        )
                        .map_err(|e| backend_err("postgres.cas_rows", e))?;
                    if changed == 0 {
                        return Err("concurrent write detected; retry".to_string());
                    }
                    let row = client
                        .query_one(
                            &format!("SELECT rev FROM {META_TABLE} WHERE name = $1"),
                            &[&name],
                        )
                        .map_err(|e| backend_err("postgres.cas_rows.read_rev", e))?;
                    Ok(row.get::<usize, i64>(0))
                }
                Backend::Mysql(conn) => {
                    conn.exec_drop(
                        format!(
                            "UPDATE {META_TABLE} SET columns_json = ?, rows_json = ?, rev = rev + 1 \
                             WHERE name = ? AND rev = ?"
                        ),
                        (columns_json, rows_json, name, expected_rev),
                    )
                    .map_err(|e| backend_err("mysql.cas_rows", e))?;
                    let changed = conn.affected_rows().try_into().unwrap_or(0usize);
                    if changed == 0 {
                        return Err("concurrent write detected; retry".to_string());
                    }
                    let row: Option<i64> = conn
                        .exec_first(
                            format!("SELECT rev FROM {META_TABLE} WHERE name = ?"),
                            (name,),
                        )
                        .map_err(|e| backend_err("mysql.cas_rows.read_rev", e))?;
                    row.ok_or_else(|| "missing table after update".to_string())
                }
            }
        }

        fn sqlite_configure(&mut self, wal: bool, busy_timeout_ms: i64) -> Result<(), String> {
            match self {
                Backend::Sqlite(conn) => {
                    let journal_mode = if wal { "WAL" } else { "DELETE" };
                    conn.pragma_update(None, "journal_mode", journal_mode)
                        .map_err(|e| backend_err("sqlite.configure.journal_mode", e))?;
                    conn.busy_timeout(std::time::Duration::from_millis(
                        busy_timeout_ms.max(0) as u64
                    ))
                    .map_err(|e| backend_err("sqlite.configure.busy_timeout", e))?;
                    Ok(())
                }
                _ => Ok(()),
            }
        }

        fn execute_statement(&mut self, statement: &str) -> Result<(), String> {
            match self {
                Backend::Sqlite(conn) => conn
                    .execute_batch(statement)
                    .map_err(|e| backend_err("sqlite.execute", e)),
                Backend::Postgresql(client) => client
                    .batch_execute(statement)
                    .map_err(|e| backend_err("postgres.execute", e)),
                Backend::Mysql(conn) => conn
                    .query_drop(statement)
                    .map_err(|e| backend_err("mysql.execute", e)),
            }
        }

        fn begin_transaction(&mut self) -> Result<(), String> {
            self.execute_statement("BEGIN TRANSACTION")
        }

        fn commit_transaction(&mut self) -> Result<(), String> {
            self.execute_statement("COMMIT")
        }

        fn rollback_transaction(&mut self) -> Result<(), String> {
            self.execute_statement("ROLLBACK")
        }

        fn savepoint(&mut self, name: &str) -> Result<(), String> {
            self.execute_statement(&format!("SAVEPOINT {name}"))
        }

        fn release_savepoint(&mut self, name: &str) -> Result<(), String> {
            self.execute_statement(&format!("RELEASE SAVEPOINT {name}"))
        }

        fn rollback_to_savepoint(&mut self, name: &str) -> Result<(), String> {
            self.execute_statement(&format!("ROLLBACK TO SAVEPOINT {name}"))
        }

        fn run_migration_sql(&mut self, statements: &[String]) -> Result<(), String> {
            for statement in statements {
                let trimmed = statement.trim();
                if trimmed.is_empty() {
                    continue;
                }
                self.execute_statement(trimmed)?;
            }
            Ok(())
        }

        fn sync_query_table(&mut self, table: &QueryTable) -> Result<(), String> {
            self.drop_logical_query_object(&table.name)?;
            self.drop_query_storage_table(&table.name)?;
            self.execute_statement(&create_query_storage_table_sql(table)?)?;
            for row in &table.rows {
                self.execute_statement(&insert_row_sql(table, row)?)?;
            }
            self.execute_statement(&create_query_view_sql(table)?)?;
            Ok(())
        }

        fn query_sql(&mut self, sql: &str) -> Result<Vec<Vec<QueryCell>>, String> {
            match self {
                Backend::Sqlite(conn) => {
                    let mut stmt = conn
                        .prepare(sql)
                        .map_err(|e| backend_err("sqlite.query.prepare", e))?;
                    let column_count = stmt.column_count();
                    let mut rows = stmt
                        .query([])
                        .map_err(|e| backend_err("sqlite.query.query", e))?;
                    let mut out = Vec::new();
                    while let Some(row) = rows
                        .next()
                        .map_err(|e| backend_err("sqlite.query.next", e))?
                    {
                        let mut result_row = Vec::with_capacity(column_count);
                        for index in 0..column_count {
                            let value = row
                                .get_ref(index)
                                .map_err(|e| backend_err("sqlite.query.get_ref", e))?;
                            result_row.push(sqlite_cell_from_value_ref(value));
                        }
                        out.push(result_row);
                    }
                    Ok(out)
                }
                Backend::Postgresql(client) => {
                    let rows = client
                        .query(sql, &[])
                        .map_err(|e| backend_err("postgres.query", e))?;
                    let mut out = Vec::with_capacity(rows.len());
                    for row in rows {
                        let mut result_row = Vec::with_capacity(row.len());
                        for index in 0..row.len() {
                            result_row.push(postgres_cell_from_row(&row, index)?);
                        }
                        out.push(result_row);
                    }
                    Ok(out)
                }
                Backend::Mysql(conn) => {
                    let rows: Vec<mysql::Row> =
                        conn.query(sql).map_err(|e| backend_err("mysql.query", e))?;
                    let mut out = Vec::with_capacity(rows.len());
                    for row in rows {
                        let values = row.unwrap();
                        out.push(values.into_iter().map(mysql_cell_from_value).collect());
                    }
                    Ok(out)
                }
            }
        }
    }

    fn backend_mut(
        backends: &mut HashMap<u64, Backend>,
        connection_id: u64,
    ) -> Result<&mut Backend, String> {
        backends
            .get_mut(&connection_id)
            .ok_or_else(|| format!("database connection {connection_id} is not open"))
    }

    let mut next_connection_id: u64 = 1;
    let mut backends: HashMap<u64, Backend> = HashMap::new();

    for req in rx {
        match req {
            DbRequest::OpenConnection { driver, url, resp } => {
                let result = (|| -> Result<u64, String> {
                    let backend = match driver {
                        Driver::Sqlite => {
                            let conn = rusqlite::Connection::open(url)
                                .map_err(|e| backend_err("sqlite.open", e))?;
                            Backend::Sqlite(conn)
                        }
                        Driver::Postgresql => {
                            let client = postgres::Client::connect(&url, postgres::NoTls)
                                .map_err(|e| backend_err("postgres.connect", e))?;
                            Backend::Postgresql(Box::new(client))
                        }
                        Driver::Mysql => {
                            let opts = mysql::Opts::from_url(&url)
                                .map_err(|e| backend_err("mysql.parse_url", e))?;
                            let conn = mysql::Conn::new(opts)
                                .map_err(|e| backend_err("mysql.connect", e))?;
                            Backend::Mysql(conn)
                        }
                    };
                    let connection_id = next_connection_id;
                    next_connection_id = next_connection_id.saturating_add(1);
                    backends.insert(connection_id, backend);
                    Ok(connection_id)
                })();
                let _ = resp.send(result);
            }
            DbRequest::CloseConnection {
                connection_id,
                resp,
            } => {
                let result = if backends.remove(&connection_id).is_some() {
                    Ok(())
                } else {
                    Err(format!("database connection {connection_id} is not open"))
                };
                let _ = resp.send(result);
            }
            DbRequest::EnsureSchema {
                connection_id,
                resp,
            } => {
                let result = backend_mut(&mut backends, connection_id)
                    .and_then(|backend| backend.ensure_schema());
                let _ = resp.send(result);
            }
            DbRequest::LoadTable {
                connection_id,
                name,
                resp,
            } => {
                let result = backend_mut(&mut backends, connection_id)
                    .and_then(|backend| backend.load_table(&name));
                let _ = resp.send(result);
            }
            DbRequest::MigrateTable {
                connection_id,
                name,
                columns_json,
                resp,
            } => {
                let result = backend_mut(&mut backends, connection_id)
                    .and_then(|backend| backend.migrate_table(&name, &columns_json));
                let _ = resp.send(result);
            }
            DbRequest::CompareAndSwapRows {
                connection_id,
                name,
                expected_rev,
                columns_json,
                rows_json,
                resp,
            } => {
                let result = backend_mut(&mut backends, connection_id).and_then(|backend| {
                    backend.compare_and_swap_rows(&name, expected_rev, &columns_json, &rows_json)
                });
                let _ = resp.send(result);
            }
            DbRequest::SqliteConfigure {
                connection_id,
                wal,
                busy_timeout_ms,
                resp,
            } => {
                let result = backend_mut(&mut backends, connection_id)
                    .and_then(|backend| backend.sqlite_configure(wal, busy_timeout_ms));
                let _ = resp.send(result);
            }
            DbRequest::BeginTransaction {
                connection_id,
                resp,
            } => {
                let result = backend_mut(&mut backends, connection_id)
                    .and_then(|backend| backend.begin_transaction());
                let _ = resp.send(result);
            }
            DbRequest::CommitTransaction {
                connection_id,
                resp,
            } => {
                let result = backend_mut(&mut backends, connection_id)
                    .and_then(|backend| backend.commit_transaction());
                let _ = resp.send(result);
            }
            DbRequest::RollbackTransaction {
                connection_id,
                resp,
            } => {
                let result = backend_mut(&mut backends, connection_id)
                    .and_then(|backend| backend.rollback_transaction());
                let _ = resp.send(result);
            }
            DbRequest::Savepoint {
                connection_id,
                name,
                resp,
            } => {
                let result = backend_mut(&mut backends, connection_id)
                    .and_then(|backend| backend.savepoint(&name));
                let _ = resp.send(result);
            }
            DbRequest::ReleaseSavepoint {
                connection_id,
                name,
                resp,
            } => {
                let result = backend_mut(&mut backends, connection_id)
                    .and_then(|backend| backend.release_savepoint(&name));
                let _ = resp.send(result);
            }
            DbRequest::RollbackToSavepoint {
                connection_id,
                name,
                resp,
            } => {
                let result = backend_mut(&mut backends, connection_id)
                    .and_then(|backend| backend.rollback_to_savepoint(&name));
                let _ = resp.send(result);
            }
            DbRequest::RunMigrationSql {
                connection_id,
                statements,
                resp,
            } => {
                let result = backend_mut(&mut backends, connection_id)
                    .and_then(|backend| backend.run_migration_sql(&statements));
                let _ = resp.send(result);
            }
            DbRequest::SyncQueryTable {
                connection_id,
                table,
                resp,
            } => {
                let result = backend_mut(&mut backends, connection_id)
                    .and_then(|backend| backend.sync_query_table(&table));
                let _ = resp.send(result);
            }
            DbRequest::QuerySql {
                connection_id,
                sql,
                resp,
            } => {
                let result = backend_mut(&mut backends, connection_id)
                    .and_then(|backend| backend.query_sql(&sql));
                let _ = resp.send(result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_without_active_transaction_returns_error() {
        let state = DatabaseState::new();
        let connection = state
            .connect(Driver::Sqlite, ":memory:".to_string())
            .expect("sqlite connection");
        let err = connection
            .commit_transaction()
            .expect_err("commit without transaction should fail");
        assert!(
            err.contains("transaction"),
            "expected transaction error, got: {err}"
        );
        connection.close().expect("close connection");
    }

    #[test]
    fn sync_query_table_exposes_only_declared_columns() {
        let state = DatabaseState::new();
        let connection = state
            .connect(Driver::Sqlite, ":memory:".to_string())
            .expect("sqlite connection");

        connection
            .sync_query_table(QueryTable {
                name: "products".to_string(),
                columns: vec![
                    QueryColumn {
                        name: "id".to_string(),
                        column_type: QueryColumnType::Int,
                        not_null: true,
                    },
                    QueryColumn {
                        name: "name".to_string(),
                        column_type: QueryColumnType::Text,
                        not_null: true,
                    },
                ],
                rows: vec![
                    QueryRow {
                        row_ordinal: 0,
                        values: vec![QueryCell::Int(1), QueryCell::Text("Widget".to_string())],
                    },
                    QueryRow {
                        row_ordinal: 1,
                        values: vec![QueryCell::Int(2), QueryCell::Text("Gadget".to_string())],
                    },
                ],
            })
            .expect("sync query table");

        let logical_columns = connection
            .query_sql("PRAGMA table_info(products)".to_string())
            .expect("query logical columns");
        let logical_column_names: Vec<String> = logical_columns
            .iter()
            .map(|row| match row.get(1) {
                Some(QueryCell::Text(name)) => name.clone(),
                other => panic!("expected PRAGMA column name text, got {other:?}"),
            })
            .collect();
        assert_eq!(
            logical_column_names,
            vec!["id".to_string(), "name".to_string()]
        );

        let storage_table = query_storage_name("products");
        let storage_columns = connection
            .query_sql(format!("PRAGMA table_info({storage_table})"))
            .expect("query storage columns");
        let storage_column_names: Vec<String> = storage_columns
            .iter()
            .map(|row| match row.get(1) {
                Some(QueryCell::Text(name)) => name.clone(),
                other => panic!("expected storage column name text, got {other:?}"),
            })
            .collect();
        assert_eq!(
            storage_column_names,
            vec![
                "__aivi_ord".to_string(),
                "id".to_string(),
                "name".to_string()
            ]
        );

        let rows = connection
            .query_sql("SELECT id, name FROM products ORDER BY id".to_string())
            .expect("query logical view rows");
        assert_eq!(rows.len(), 2);
        assert!(matches!(
            rows[0].as_slice(),
            [QueryCell::Int(1), QueryCell::Text(name)] if name == "Widget"
        ));
        assert!(matches!(
            rows[1].as_slice(),
            [QueryCell::Int(2), QueryCell::Text(name)] if name == "Gadget"
        ));

        connection.close().expect("close connection");
    }
}
