# Resource Management

AIVI provides a dedicated `Resource` type to manage lifecycles (setup and teardown) in a declarative way. This ensures that resources like files, sockets, and database connections are always reliably released, even in the event of errors or task cancellation.


## 15.1 Defining Resources

Resources are defined using `resource` blocks. The syntax is analogous to generators: you perform setup, `yield` the resource, and then perform cleanup.

The code after `yield` is guaranteed to run when the resource goes out of scope.

<<< ../snippets/from_md/02_syntax/15_resources/block_01.aivi{aivi}

This declarative approach hides the complexity of error handling and cancellation checks.


## 15.2 Using Resources

Inside an `effect` block, you use the `<-` binder to acquire a resource. This is similar to the generator binder, but instead of iterating, it scopes the resource to the current block.

<<< ../snippets/from_md/02_syntax/15_resources/block_02.aivi{aivi}

### Multiple Resources

You can acquire multiple resources in sequence. They will be released in reverse order of acquisition (LIFO).

<<< ../snippets/from_md/02_syntax/15_resources/block_03.aivi{aivi}
