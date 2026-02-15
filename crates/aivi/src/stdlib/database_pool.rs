pub const MODULE_NAME: &str = "aivi.database.pool";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.database.pool
export Pool, Config, PoolError, PoolStats, BackoffPolicy, QueuePolicy
export Timeout, Closed, HealthFailed, InvalidConfig
export Fixed, Exponential
export Fifo, Lifo
export create, withConn, acquire, release, stats, drain, close

use aivi
use aivi.duration
use aivi.database

PoolError =
  | Timeout
  | Closed
  | HealthFailed
  | InvalidConfig Text

PoolStats = { size: Int, idle: Int, inUse: Int, waiters: Int, closed: Bool }

QueuePolicy = Fifo | Lifo

BackoffPolicy = Fixed Span | Exponential { base: Span, max: Span }

Config Conn = {
  maxSize: Int
  minIdle: Int
  acquireTimeout: Span
  idleTimeout: Option Span
  maxLifetime: Option Span
  healthCheckInterval: Option Span
  backoffPolicy: BackoffPolicy
  queuePolicy: QueuePolicy
  acquire: Unit -> Effect DbError Conn
  release: Conn -> Effect DbError Unit
  healthCheck: Conn -> Effect DbError Bool
}

// Pool is represented as a record of effectful operations.
Pool Conn = {
  acquire: Unit -> Effect DbError (Result PoolError Conn)
  release: Conn -> Effect DbError Unit
  stats: Unit -> Effect DbError PoolStats
  drain: Unit -> Effect DbError Unit
  close: Unit -> Effect DbError Unit
  withConn: (Conn -> Effect DbError A) -> Effect DbError (Result PoolError A)
}

create : Config Conn -> Effect DbError (Result PoolError (Pool Conn))
create = config => database.pool.create config

acquire : Pool Conn -> Effect DbError (Result PoolError Conn)
acquire = pool => pool.acquire Unit

release : Pool Conn -> Conn -> Effect DbError Unit
release = pool conn => pool.release conn

stats : Pool Conn -> Effect DbError PoolStats
stats = pool => pool.stats Unit

drain : Pool Conn -> Effect DbError Unit
drain = pool => pool.drain Unit

close : Pool Conn -> Effect DbError Unit
close = pool => pool.close Unit

withConn : Pool Conn -> (Conn -> Effect DbError A) -> Effect DbError (Result PoolError A)
withConn = pool run => pool.withConn run
"#;
