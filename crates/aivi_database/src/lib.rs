use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Mutex;

const META_TABLE: &str = "aivi_tables";
pub const EMPTY_ROWS_JSON: &str = "{\"t\":\"List\",\"v\":[]}";

pub type LoadTableRow = (i64, String, String);

#[derive(Clone, Copy, Debug)]
pub enum Driver {
    Sqlite,
    Postgresql,
    Mysql,
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

    impl Backend {
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
}
