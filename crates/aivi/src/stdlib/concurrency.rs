pub const MODULE_NAME: &str = "aivi.concurrency";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.concurrency
export Scope, ChannelError
export par, scope
export make, send, recv, close

use aivi

Scope = Unit
ChannelError = Closed

par : Effect e a -> Effect e b -> Effect e (a, b)
par = left right => concurrent.par left right

scope : (Scope -> Effect e a) -> Effect e a
scope = run => concurrent.scope (run Unit)

make : A -> Effect e (Sender A, Receiver A)
make = sample => channel.make sample

send : Sender A -> A -> Effect e Unit
send = sender value => channel.send sender value

recv : Receiver A -> Effect e (Result A ChannelError)
recv = receiver => channel.recv receiver

close : Sender A -> Effect e Unit
close = sender => channel.close sender
"#;
