pub const MODULE_NAME: &str = "aivi.concurrency";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.concurrency
export Scope, ChannelError
export par, race, scope
export make, makeBounded, send, recv, close
export fold, forEach
export sleep, timeoutWith, retry

use aivi

Scope = Unit
ChannelError = Closed

par : Effect e a -> Effect e b -> Effect e (a, b)
par = left right => concurrent.par left right

race : Effect e a -> Effect e a -> Effect e a
race = left right => concurrent.race left right

scope : (Scope -> Effect e a) -> Effect e a
scope = run => concurrent.scope (run Unit)

sleep : Int -> Effect Text Unit
sleep = concurrent.sleep

timeoutWith : Int -> Text -> Effect Text a -> Effect Text a
timeoutWith = concurrent.timeoutWith

retry : Int -> Effect e a -> Effect e a
retry = concurrent.retry

make : a -> Effect e (Sender a, Receiver a)
make = _sample => channel.make Unit

makeBounded : Int -> Effect e (Sender a, Receiver a)
makeBounded = capacity => channel.makeBounded capacity

send : Sender a -> a -> Effect e Unit
send = sender value => channel.send sender value

recv : Receiver a -> Effect e (Result a ChannelError)
recv = receiver => channel.recv receiver

close : Sender a -> Effect e Unit
close = sender => channel.close sender

fold : s -> (s -> a -> Effect e s) -> Receiver a -> Effect e s
fold = init => fn => receiver => do Effect {
  loop state = init => {
    result <- channel.recv receiver
    result match
      | Err _ => pure state
      | Ok value => do Effect {
          next <- fn state value
          recurse next
        }
  }
}

forEach : Receiver a -> (a -> Effect e Unit) -> Effect e Unit
forEach = receiver => fn => do Effect {
  loop _ = Unit => {
    result <- channel.recv receiver
    result match
      | Err _ => pure Unit
      | Ok value => do Effect {
          _ <- fn value
          recurse Unit
        }
  }
}
"#;
