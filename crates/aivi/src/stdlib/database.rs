pub const MODULE_NAME: &str = "aivi.database";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.database
export RelationLink, Relation, ColumnType, ColumnConstraint, ColumnDefault, Column
export IntType, FloatType, BoolType, TimestampType, Varchar
export AutoIncrement, NotNull
export DefaultBool, DefaultInt, DefaultText, DefaultNow
export DbError
export Driver, DbConfig, DbConnection, configure, connect, open, close
export Sqlite, Postgresql, Mysql
export SqliteTuning, MigrationStep, SavepointName, TxAction
export FtsDoc, FtsQuery
export relation, hasMany, belongsTo, runMigrations, runMigrationSql
export runMigrationsOn, runMigrationSqlOn
export configureSqlite, configureSqliteOn
export beginTx, commitTx, rollbackTx, inTransaction
export beginTxOn, commitTxOn, rollbackTxOn, inTransactionOn
export savepoint, releaseSavepoint, rollbackToSavepoint
export savepointOn, releaseSavepointOn, rollbackToSavepointOn
export insert, insertOn
export rows, rowsOn, first, firstOn, count, countOn, exists, existsOn
export delete, deleteOn
export update, updateOn
export upsert, upsertOn
export Query, GroupedQuery, Order, OrderTerm, Agg
export orderBy, limit, offset, distinct, selectMap, groupBy, having
export asc, desc, key, sum, avg, min, max
export ftsDoc, ftsMatchAny, ftsMatchAll

use aivi
use aivi.prelude (panic)
DbError = Text

Patch A = A -> A

RelationLink = {
  name: Text
  target: Text
  many: Bool
}

Relation A = {
  name: Text
  columns: List Column
  links: List RelationLink
  rows: List A
  run: DbConnection -> Effect DbError (List A)
}

Query A = {
  run: DbConnection -> Effect DbError (List A)
}

GroupedQuery K A = {
  runGroups: DbConnection -> Effect DbError (List Unit)
}

OrderTerm A = {
  descending: Bool
}

Order A = OrderTerm A
Agg A B = B

ColumnType = IntType | FloatType | BoolType | TimestampType | Varchar Int
ColumnConstraint = AutoIncrement | NotNull
ColumnDefault = DefaultBool Bool | DefaultInt Int | DefaultText Text | DefaultNow
Column = {
  name: Text
  type: ColumnType
  constraints: List ColumnConstraint
  default: Option ColumnDefault
}

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

withRelationRuntime : { name: Text, columns: List Column, links: List RelationLink, rows: List A } -> Relation A
withRelationRuntime = rel => {
  name: rel.name
  columns: rel.columns
  links: rel.links
  rows: rel.rows
  run: conn => database.loadRelationOn conn rel
}

loweringRequiredQuery : Text -> Query A
loweringRequiredQuery = message => {
  run: _conn => fail message
}

loweringRequiredGrouped : Text -> GroupedQuery K A
loweringRequiredGrouped = message => {
  runGroups: _conn => fail message
}

loweringRequiredPure : Text -> A
loweringRequiredPure = message => panic message

relation : Text -> List Column -> List RelationLink -> Relation A
relation = name columns links =>
  withRelationRuntime { name, columns, links, rows: [] }

hasMany : Text -> Text -> (A -> K) -> (B -> K) -> RelationLink
hasMany = name target _sourceKey _targetKey => { name, target, many: True }

belongsTo : Text -> Text -> (A -> K) -> (B -> K) -> RelationLink
belongsTo = name target _sourceKey _targetKey => { name, target, many: False }

asc : (A -> B) -> OrderTerm A
asc = _key => { descending: False }

desc : (A -> B) -> OrderTerm A
desc = _key => { descending: True }

orderBy : O -> Query A -> Query A
orderBy = _order _query =>
  loweringRequiredQuery "database.orderBy requires SQL-backed lowering"

limit : Int -> Query A -> Query A
limit = n q => {
  run: conn =>
    conn
       |> q.run
       |> List.take n
}

offset : Int -> Query A -> Query A
offset = n q => {
  run: conn =>
    conn
       |> q.run
       |> List.drop n
}

distinct : Query A -> Query A
distinct = q => {
  run: conn =>
    conn
       |> q.run
       |> List.dedup
}

selectMap : M -> Query A -> Query B
selectMap = _mapper _source =>
  loweringRequiredQuery "database.selectMap requires SQL-backed lowering"

selectMap : M -> GroupedQuery K A -> Query B
selectMap = _mapper _source =>
  loweringRequiredQuery "database.grouped selectMap requires SQL-backed lowering"

groupBy : K -> Query A -> GroupedQuery K A
groupBy = _key _query =>
  loweringRequiredGrouped "database.groupBy requires SQL-backed lowering"

having : H -> GroupedQuery K A -> GroupedQuery K A
having = _pred _query =>
  loweringRequiredGrouped "database.having requires SQL-backed lowering"

key : K
key = loweringRequiredPure "database.key requires grouped SQL-backed lowering"

count : Int
count = loweringRequiredPure "database.count aggregate requires grouped SQL-backed lowering"

sum : (A -> N) -> N
sum = _selector =>
  loweringRequiredPure "database.sum requires grouped SQL-backed lowering"

avg : (A -> N) -> Float
avg = _selector =>
  loweringRequiredPure "database.avg requires grouped SQL-backed lowering"

min : (A -> B) -> B
min = _selector =>
  loweringRequiredPure "database.min requires grouped SQL-backed lowering"

max : (A -> B) -> B
max = _selector =>
  loweringRequiredPure "database.max requires grouped SQL-backed lowering"

rows : Query A -> Effect DbError (List A)
rows = q => database.rows q

rowsOn : DbConnection -> Query A -> Effect DbError (List A)
rowsOn = conn q => database.rowsOn conn q

first : Query A -> Effect DbError (Option A)
first = q => database.first q

firstOn : DbConnection -> Query A -> Effect DbError (Option A)
firstOn = conn q => database.firstOn conn q

count : Query A -> Effect DbError Int
count = q => database.count q

countOn : DbConnection -> Query A -> Effect DbError Int
countOn = conn q => database.countOn conn q

exists : Query A -> Effect DbError Bool
exists = q => database.exists q

existsOn : DbConnection -> Query A -> Effect DbError Bool
existsOn = conn q => database.existsOn conn q

configure : DbConfig -> Effect DbError Unit
configure = config => database.configure config

connect : DbConfig -> Effect DbError DbConnection
connect = config => database.connect config

open : DbConfig -> Resource DbError DbConnection
open = config =>
  config
     |> connect @cleanup close #conn

close : DbConnection -> Effect DbError Unit
close = conn => database.close conn

configureSqlite : SqliteTuning -> Effect DbError Unit
configureSqlite = tuning => database.configureSqlite tuning

configureSqliteOn : DbConnection -> SqliteTuning -> Effect DbError Unit
configureSqliteOn = conn tuning => database.configureSqliteOn conn tuning

runMigrations : List (Relation A) -> Effect DbError Unit
runMigrations = relations =>
  database.runMigrations relations

runMigrationsOn : DbConnection -> List (Relation A) -> Effect DbError Unit
runMigrationsOn = conn relations =>
  database.runMigrationsOn conn relations

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
inTransaction = action =>
  Unit
    ~|> (_ => beginTx)
    ?|> (_ => action)
    !|> err => err
       ~|> (_ => rollbackTx)
        |> fail
    ~|> (_ => commitTx)

inTransactionOn : DbConnection -> Effect DbError A -> Effect DbError A
inTransactionOn = conn action =>
  Unit
    ~|> (_ => beginTxOn conn)
    ?|> (_ => action)
    !|> err => err
       ~|> (_ => rollbackTxOn conn)
        |> fail
    ~|> (_ => commitTxOn conn)

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

insert : Relation A -> A -> Effect DbError (Relation A)
insert = rel value => database.insert rel value

insertOn : DbConnection -> Relation A -> A -> Effect DbError (Relation A)
insertOn = conn rel value => database.insertOn conn rel value

delete : Query A -> Effect DbError (Relation A)
delete = query => database.delete query

deleteOn : DbConnection -> Query A -> Effect DbError (Relation A)
deleteOn = conn query => database.deleteOn conn query

update : Query A -> Patch A -> Effect DbError (Relation A)
update = query patchFn => database.update query patchFn

updateOn : DbConnection -> Query A -> Patch A -> Effect DbError (Relation A)
updateOn = conn query patchFn => database.updateOn conn query patchFn

upsert : Query A -> A -> Patch A -> Effect DbError (Relation A)
upsert = query value patchFn => database.upsert query value patchFn

upsertOn : DbConnection -> Query A -> A -> Patch A -> Effect DbError (Relation A)
upsertOn = conn query value patchFn => database.upsertOn conn query value patchFn

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
"#;
