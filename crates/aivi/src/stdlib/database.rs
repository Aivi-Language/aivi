pub const MODULE_NAME: &str = "aivi.database";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.database
export Table, ColumnType, ColumnConstraint, ColumnDefault, Column
export IntType, BoolType, TimestampType, Varchar
export AutoIncrement, NotNull
export DefaultBool, DefaultInt, DefaultText, DefaultNow
export Pred, Patch, Delta, DbError
export Driver, DbConfig, configure
export Sqlite, Postgresql, Mysql
export SqliteTuning, MigrationStep, SavepointName, TxAction
export FtsDoc, FtsQuery
export table, load, applyDelta, applyDeltas, runMigrations, runMigrationSql
export configureSqlite
export beginTx, commitTx, rollbackTx, inTransaction
export savepoint, releaseSavepoint, rollbackToSavepoint
export chunkDeltas, ftsDoc, ftsMatchAny, ftsMatchAll
export ins, upd, del
export domain Database

use aivi

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
Delta A = Insert A | Update (Pred A) (Patch A) | Delete (Pred A)

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

configure : DbConfig -> Effect DbError Unit
configure = config => database.configure config

configureSqlite : SqliteTuning -> Effect DbError Unit
configureSqlite = tuning => database.configureSqlite tuning

table : Text -> List Column -> Table A
table = name columns => database.table name columns

load : Table A -> Effect DbError (List A)
load = value => database.load value

applyDelta : Table A -> Delta A -> Effect DbError (Table A)
applyDelta = table delta => database.applyDelta table delta

applyDeltas : Table A -> List (Delta A) -> Effect DbError (Table A)
applyDeltas = table deltas => deltas match
  | [] => pure table
  | [d, ...rest] => do Effect {
      next <- applyDelta table d
      applyDeltas next rest
    }

runMigrations : List (Table A) -> Effect DbError Unit
runMigrations = tables => database.runMigrations tables

runMigrationSql : List MigrationStep -> Effect DbError Unit
runMigrationSql = steps =>
  database.runMigrationSql (map steps (step => step.sql))

beginTx : Effect DbError Unit
beginTx = database.beginTx

commitTx : Effect DbError Unit
commitTx = database.commitTx

rollbackTx : Effect DbError Unit
rollbackTx = database.rollbackTx

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

savepoint : SavepointName -> Effect DbError Unit
savepoint = name => database.savepoint name

releaseSavepoint : SavepointName -> Effect DbError Unit
releaseSavepoint = name => database.releaseSavepoint name

rollbackToSavepoint : SavepointName -> Effect DbError Unit
rollbackToSavepoint = name => database.rollbackToSavepoint name

chunkDeltas : Int -> List (Delta A) -> List (List (Delta A))
chunkDeltas = size deltas =>
  if size <= 0 then [deltas] else chunkDeltasGo size deltas [] []

chunkDeltasGo : Int -> List (Delta A) -> List (Delta A) -> List (List (Delta A)) -> List (List (Delta A))
chunkDeltasGo = size remaining current acc => remaining match
  | [] => current match
    | [] => List.reverse acc
    | _ => List.reverse [List.reverse current, ...acc]
  | [d, ...rest] =>
      if List.length current >= size
      then chunkDeltasGo size remaining [] [List.reverse current, ...acc]
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

domain Database over Table A = {
  (+) : Table A -> Delta A -> Effect DbError (Table A)
  (+) = table delta => applyDelta table delta

  ins = Insert
  upd = Update
  del = Delete
}
"#;
