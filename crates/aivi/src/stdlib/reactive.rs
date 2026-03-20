pub const MODULE_NAME: &str = "aivi.reactive";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.reactive
export Signal, Disposable, EventHandle
export signal, get, peek, set, update
export derive, combineAll
export watch, on, batch, dispose, event

use aivi

opaque Signal A = { __signalRuntimeId: Int }

Disposable = {
  dispose: Unit -> Unit
}

EventHandle E A = {
  run: Effect E A
  result: Signal (Option A)
  error: Signal (Option E)
  done: Signal Bool
  running: Signal Bool
}

signal : A -> Signal A
signal = initial => reactive.signal initial

get : Signal A -> A
get = sig => reactive.get sig

peek : Signal A -> A
peek = sig => reactive.peek sig

set : Signal A -> A -> Unit
set = sig value => reactive.set sig value

update : Signal A -> (A -> A) -> Unit
update = sig updater => reactive.update sig updater

derive : Signal A -> (A -> B) -> Signal B
derive = sig mapper => reactive.derive sig mapper

combineAll : A -> (B -> C) -> Signal C
combineAll = signals fn => reactive.combineAll signals fn

watch : Signal A -> (A -> R) -> Disposable
watch = sig callback => reactive.watch sig callback

on : Signal A -> (A -> R) -> Disposable
on = watch

batch : (Unit -> A) -> A
batch = callback => reactive.batch callback

dispose : Disposable -> Unit
dispose = disposable => disposable.dispose Unit

eventFrom = action => reactive.eventFrom action

event : { from: Effect E A -> EventHandle E A }
event = {
  from: eventFrom
}
"#;
