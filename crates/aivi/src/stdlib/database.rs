pub const MODULE_NAME: &str = "aivi.database";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.database
export Table, ColumnType, ColumnConstraint, ColumnDefault, Column
export IntType, BoolType, TimestampType, Varchar
export AutoIncrement, NotNull
export DefaultBool, DefaultInt, DefaultText, DefaultNow
export Pred, Patch, Delta, DbError
export Driver, DbConfig, DbConnection, configure, connect, open, close
export Sqlite, Postgresql, Mysql
export SqliteTuning, MigrationStep, SavepointName, TxAction
export FtsDoc, FtsQuery
export table, load, query, applyDelta, applyDeltas, runMigrations, runMigrationSql
export loadOn, queryOn, applyDeltaOn, applyDeltasOn, runMigrationsOn, runMigrationSqlOn
export configureSqlite, configureSqliteOn
export beginTx, commitTx, rollbackTx, inTransaction
export beginTxOn, commitTxOn, rollbackTxOn, inTransactionOn
export savepoint, releaseSavepoint, rollbackToSavepoint
export savepointOn, releaseSavepointOn, rollbackToSavepointOn
export chunkDeltas, ftsDoc, ftsMatchAny, ftsMatchAll
export ins, upd, del, ups
export insert, insertOn
export deleteWhere, deleteWhereOn
export updateWhere, updateWhereOn
export upsert, upsertOn
export domain Database
export Query, queryOf, queryChain, emptyQuery, from, where_, guard_, select, runQueryOn, runQuery
export orderBy, limit, offset, count, exists

use aivi
use aivi.list (length, reverse)

DbError = Text

Table A = { name: Text, columns: List Column, rows: List A }

ColumnType = IntType | BoolType | TimestampType | Varchar Int
ColumnConstraint = AutoIncrement | NotNull
ColumnDefault = DefaultBool Bool | DefaultInt Int | DefaultText Text | DefaultNow
Column = {
  name: Text
  type: ColumnType
  constraints: List ColumnConstraint
  default: Option ColumnDefault
}

Pred A = A -> Bool
Patch A = A -> A
Delta A = Insert A | Update (Pred A) (Patch A) | Delete (Pred A) | Upsert (Pred A) A (Patch A)

Driver = Sqlite | Postgresql | Mysql
DbConfig = { driver: Driver, url: Text }

SqliteTuning = {
  wal: Bool
  busyTimeoutMs: Int
}

MigrationStep = {
  id: Text
  sql: Text
}

SavepointName = Text
TxAction A =
  | TxContinue A
  | TxCommit A
  | TxRollback DbError

FtsDoc = {
  docId: Text
  terms: List Text
}

FtsQuery = {
  expression: Text
  matchMode: Text
}

// ---------------------------------------------------------------------------
// Query DSL — MVP
//
// `Query A` is an in-memory query over a `DbConnection`.  It is not true SQL
// pushdown; predicates and projections run in the AIVI runtime after loading
// rows from the underlying store.  Future phases may lower `do Query` blocks
// to SQL WHERE/SELECT clauses.
//
// Use `do Query { ... }` notation to compose queries; call `runQueryOn conn q`
// to execute.  The `do Query` block desugars using `queryChain`/`queryOf`
// (rather than the generic monad `chain`/`of`) so the types resolve through
// the helpers below.
//
// Example:
//
//   activeNames : Query Text
//   activeNames = do Query {
//     user <- from userTable
//     guard_ user.active
//     queryOf user.name
//   }
//
//   main = do Effect {
//     conn    <- connect { driver: Sqlite, url: ":memory:" }
//     names   <- runQueryOn conn activeNames
//     ...
//   }
// ---------------------------------------------------------------------------

Query A = { run: DbConnection -> Effect DbError (List A) }

emptyQuery : Query A
emptyQuery = { run: _conn => pure [] }

queryOf : A -> Query A
queryOf = value => { run: _conn => pure [value] }

queryBindAll : DbConnection -> List A -> (A -> Query B) -> Effect DbError (List B)
queryBindAll = conn xs f => xs match
  | []           => pure []
  | [x, ...rest] => do Effect {
      ys <- (f x).run conn
      zs <- queryBindAll conn rest f
      pure [...ys, ...zs]
    }

queryChain : (A -> Query B) -> Query A -> Query B
queryChain = f q => { run: conn => do Effect {
  xs <- q.run conn
  queryBindAll conn xs f
}}

from : Table A -> Query A
from = tbl => { run: conn => loadOn conn tbl }

where_ : (A -> Bool) -> Query A -> Query A
where_ = pred q => { run: conn => do Effect {
  xs <- q.run conn
  pure (List.filter pred xs)
}}

guard_ : Bool -> Query Unit
guard_ = cond => if cond then queryOf Unit else emptyQuery

select : (A -> B) -> Query A -> Query B
select = f q => { run: conn => do Effect {
  xs <- q.run conn
  pure (List.map f xs)
}}

runQueryOn : DbConnection -> Query A -> Effect DbError (List A)
runQueryOn = conn q => q.run conn

runQuery : Query A -> Effect DbError (List A)
runQuery = q => database.runQuery q

orderBy : (A -> B) -> Query A -> Query A
orderBy = key q => { run: conn => do Effect {
  xs <- q.run conn
  pure (List.sortBy key xs)
}}

limit : Int -> Query A -> Query A
limit = n q => { run: conn => do Effect {
  xs <- q.run conn
  pure (List.take n xs)
}}

offset : Int -> Query A -> Query A
offset = n q => { run: conn => do Effect {
  xs <- q.run conn
  pure (List.drop n xs)
}}

count : Query A -> Query Int
count = q => { run: conn => do Effect {
  xs <- q.run conn
  pure [List.length xs]
}}

exists : Query A -> Query Bool
exists = q => { run: conn => do Effect {
  xs <- q.run conn
  xs match
    | [] => pure [False]
    | _  => pure [True]
}}

configure : DbConfig -> Effect DbError Unit
configure = config => database.configure config

connect : DbConfig -> Effect DbError DbConnection
connect = config => database.connect config

open : DbConfig -> Resource DbError DbConnection
open = config => resource {
  conn <- connect config
  yield conn
  _ <- close conn
}

close : DbConnection -> Effect DbError Unit
close = conn => database.close conn

configureSqlite : SqliteTuning -> Effect DbError Unit
configureSqlite = tuning => database.configureSqlite tuning

configureSqliteOn : DbConnection -> SqliteTuning -> Effect DbError Unit
configureSqliteOn = conn tuning => database.configureSqliteOn conn tuning

table : Text -> List Column -> Table A
table = name columns => database.table name columns

load : Table A -> Effect DbError (List A)
load = value => database.load value

loadOn : DbConnection -> Table A -> Effect DbError (List A)
loadOn = conn value => database.loadOn conn value

query : Table A -> (A -> Bool) -> Effect DbError (List A)
query = tbl pred => do Effect {
  rows <- load tbl
  pure (List.filter pred rows)
}

queryOn : DbConnection -> Table A -> (A -> Bool) -> Effect DbError (List A)
queryOn = conn tbl pred => do Effect {
  rows <- loadOn conn tbl
  pure (List.filter pred rows)
}

applyDelta : Table A -> Delta A -> Effect DbError (Table A)
applyDelta = table delta => database.applyDelta table delta

applyDeltaOn : DbConnection -> Table A -> Delta A -> Effect DbError (Table A)
applyDeltaOn = conn table delta => database.applyDeltaOn conn table delta

applyDeltas : Table A -> List (Delta A) -> Effect DbError (Table A)
applyDeltas = table deltas => deltas match
  | [] => pure table
  | [d, ...rest] => do Effect {
      next <- applyDelta table d
      applyDeltas next rest
    }

applyDeltasOn : DbConnection -> Table A -> List (Delta A) -> Effect DbError (Table A)
applyDeltasOn = conn table deltas => deltas match
  | [] => pure table
  | [d, ...rest] => do Effect {
      next <- applyDeltaOn conn table d
      applyDeltasOn conn next rest
    }

runMigrations : List (Table A) -> Effect DbError Unit
runMigrations = tables => database.runMigrations tables

runMigrationsOn : DbConnection -> List (Table A) -> Effect DbError Unit
runMigrationsOn = conn tables => database.runMigrationsOn conn tables

collectSql : List MigrationStep -> List Text
collectSql = steps => steps match
  | [] => []
  | [step, ...rest] => [step.sql, ...collectSql rest]

runMigrationSql : List MigrationStep -> Effect DbError Unit
runMigrationSql = steps =>
  database.runMigrationSql (collectSql steps)

runMigrationSqlOn : DbConnection -> List MigrationStep -> Effect DbError Unit
runMigrationSqlOn = conn steps =>
  database.runMigrationSqlOn conn (collectSql steps)

beginTx : Effect DbError Unit
beginTx = database.beginTx Unit

beginTxOn : DbConnection -> Effect DbError Unit
beginTxOn = conn => database.beginTxOn conn

commitTx : Effect DbError Unit
commitTx = database.commitTx Unit

commitTxOn : DbConnection -> Effect DbError Unit
commitTxOn = conn => database.commitTxOn conn

rollbackTx : Effect DbError Unit
rollbackTx = database.rollbackTx Unit

rollbackTxOn : DbConnection -> Effect DbError Unit
rollbackTxOn = conn => database.rollbackTxOn conn

inTransaction : Effect DbError A -> Effect DbError A
inTransaction = action => do Effect {
  beginTx
  result <- attempt action
  result match
    | Ok value => do Effect {
        commitTx
        pure value
      }
    | Err err => do Effect {
        rollbackTx
        fail err
      }
}

inTransactionOn : DbConnection -> Effect DbError A -> Effect DbError A
inTransactionOn = conn action => do Effect {
  beginTxOn conn
  result <- attempt action
  result match
    | Ok value => do Effect {
        commitTxOn conn
        pure value
      }
    | Err err => do Effect {
        rollbackTxOn conn
        fail err
      }
}

savepoint : SavepointName -> Effect DbError Unit
savepoint = name => database.savepoint name

savepointOn : DbConnection -> SavepointName -> Effect DbError Unit
savepointOn = conn name => database.savepointOn conn name

releaseSavepoint : SavepointName -> Effect DbError Unit
releaseSavepoint = name => database.releaseSavepoint name

releaseSavepointOn : DbConnection -> SavepointName -> Effect DbError Unit
releaseSavepointOn = conn name => database.releaseSavepointOn conn name

rollbackToSavepoint : SavepointName -> Effect DbError Unit
rollbackToSavepoint = name => database.rollbackToSavepoint name

rollbackToSavepointOn : DbConnection -> SavepointName -> Effect DbError Unit
rollbackToSavepointOn = conn name => database.rollbackToSavepointOn conn name

chunkDeltas : Int -> List (Delta A) -> List (List (Delta A))
chunkDeltas = size deltas =>
  if size <= 0 then [deltas] else chunkDeltasGo size deltas [] []

chunkFinalize : List (Delta A) -> List (List (Delta A)) -> List (List (Delta A))
chunkFinalize = current acc => current match
  | [] => reverse acc
  | _  => reverse [reverse current, ...acc]

chunkDeltasGo : Int -> List (Delta A) -> List (Delta A) -> List (List (Delta A)) -> List (List (Delta A))
chunkDeltasGo = size remaining current acc => remaining match
  | [] => chunkFinalize current acc
  | [d, ...rest] =>
      if length current >= size
      then chunkDeltasGo size remaining [] [reverse current, ...acc]
      else chunkDeltasGo size rest [d, ...current] acc

ftsDoc : Text -> List Text -> FtsDoc
ftsDoc = docId terms => { docId, terms }

joinTerms : List Text -> Text
joinTerms = terms => terms match
  | [] => ""
  | [x] => x
  | [x, ...xs] => text.concat [x, " ", joinTerms xs]

joinTermsWithOr : List Text -> Text
joinTermsWithOr = terms => terms match
  | [] => ""
  | [x] => x
  | [x, ...xs] => text.concat [x, " OR ", joinTermsWithOr xs]

ftsMatchAny : List Text -> FtsQuery
ftsMatchAny = terms => { expression: joinTermsWithOr terms, matchMode: "any" }

ftsMatchAll : List Text -> FtsQuery
ftsMatchAll = terms => { expression: joinTerms terms, matchMode: "all" }

ins : A -> Delta A
ins = value => Insert value

upd : Pred A -> Patch A -> Delta A
upd = pred patchFn => Update pred patchFn

del : Pred A -> Delta A
del = pred => Delete pred

ups : Pred A -> A -> Patch A -> Delta A
ups = pred value patchFn => Upsert pred value patchFn

insert : Table A -> A -> Effect DbError (Table A)
insert = table value => applyDelta table (ins value)

insertOn : DbConnection -> Table A -> A -> Effect DbError (Table A)
insertOn = conn table value => applyDeltaOn conn table (ins value)

deleteWhere : Table A -> (A -> Bool) -> Effect DbError (Table A)
deleteWhere = table pred => applyDelta table (del pred)

deleteWhereOn : DbConnection -> Table A -> (A -> Bool) -> Effect DbError (Table A)
deleteWhereOn = conn table pred => applyDeltaOn conn table (del pred)

updateWhere : Table A -> (A -> Bool) -> Patch A -> Effect DbError (Table A)
updateWhere = table pred patchFn => applyDelta table (upd pred patchFn)

updateWhereOn : DbConnection -> Table A -> (A -> Bool) -> Patch A -> Effect DbError (Table A)
updateWhereOn = conn table pred patchFn => applyDeltaOn conn table (upd pred patchFn)

upsert : Table A -> (A -> Bool) -> A -> Patch A -> Effect DbError (Table A)
upsert = table pred value patchFn => applyDelta table (ups pred value patchFn)

upsertOn : DbConnection -> Table A -> (A -> Bool) -> A -> Patch A -> Effect DbError (Table A)
upsertOn = conn table pred value patchFn => applyDeltaOn conn table (ups pred value patchFn)

domain Database over Table A = {
  (+) : Table A -> Delta A -> Effect DbError (Table A)
  (+) = table delta => applyDelta table delta

  ins = Insert
  upd = Update
  del = Delete
  ups = Upsert
}
"#;
