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
export table, load, applyDelta, runMigrations
export ins, upd, del
export domain Database

use aivi

type DbError = Text

Table A = { name: Text, columns: List Column, rows: List A }

type ColumnType = IntType | BoolType | TimestampType | Varchar Int
type ColumnConstraint = AutoIncrement | NotNull
type ColumnDefault = DefaultBool Bool | DefaultInt Int | DefaultText Text | DefaultNow
type Column = {
  name: Text
  type: ColumnType
  constraints: List ColumnConstraint
  default: Option ColumnDefault
}

type Pred A = A -> Bool
type Patch A = A -> A
type Delta A = Insert A | Update (Pred A) (Patch A) | Delete (Pred A)

type Driver = Sqlite | Postgresql | Mysql
type DbConfig = { driver: Driver, url: Text }

configure : DbConfig -> Effect DbError Unit
configure config = database.configure config

table : Text -> List Column -> Table A
table name columns = database.table name columns

load : Table A -> Effect DbError (List A)
load value = database.load value

applyDelta : Table A -> Delta A -> Effect DbError (Table A)
applyDelta table delta = database.applyDelta table delta

runMigrations : List (Table A) -> Effect DbError Unit
runMigrations tables = database.runMigrations tables

ins : A -> Delta A
ins value = Insert value

upd : Pred A -> Patch A -> Delta A
upd pred patchFn = Update pred patchFn

del : Pred A -> Delta A
del pred = Delete pred

domain Database over Table A = {
  (+) : Table A -> Delta A -> Effect DbError (Table A)
  (+) table delta = applyDelta table delta

  ins = Insert
  upd = Update
  del = Delete
}"#;
