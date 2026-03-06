pub const MODULE_NAME: &str = "aivi.net.streams";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.net.streams
export Stream, StreamError
export fromSocket, toSocket, chunks
export fromList, map, filter, take, drop, flatMap, merge, fold

use aivi
use aivi.logic

StreamError = { message: Text }

fromSocket : Connection -> Stream (List Int)
fromSocket = conn => streams.fromSocket conn

toSocket : Connection -> Stream (List Int) -> Effect StreamError Unit
toSocket = conn stream => streams.toSocket conn stream

chunks : Int -> Stream (List Int) -> Stream (List Int)
chunks = size stream => streams.chunks size stream

fromList : List A -> Stream A
fromList = items => streams.fromList items

map : (A -> B) -> Stream A -> Stream B
map = f s => streams.map f s

filter : (A -> Bool) -> Stream A -> Stream A
filter = pred s => streams.filter pred s

take : Int -> Stream A -> Stream A
take = n s => streams.take n s

drop : Int -> Stream A -> Stream A
drop = n s => streams.drop n s

flatMap : (A -> Stream B) -> Stream A -> Stream B
flatMap = f s => streams.flatMap f s

merge : Stream A -> Stream A -> Stream A
merge = left right => streams.merge left right

fold : (B -> A -> B) -> B -> Stream A -> Effect StreamError B
fold = f seed s => streams.fold f seed s

instance Functor (Stream A) = given (A: Any) {
  map: f s => streams.map f s
}

instance Filterable (Stream A) = given (A: Any) {
  filter: pred s => streams.filter pred s
}
"#;
