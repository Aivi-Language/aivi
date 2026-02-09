# Resource Management

AIVI provides a dedicated `Resource` type to manage lifecycles (setup and teardown) in a declarative way. This ensures that resources like files, sockets, and database connections are always reliably released, even in the event of errors or task cancellation.


## 15.1 Defining Resources

Resources are defined using `resource` blocks. The syntax is analogous to generators: you perform setup, `yield` the resource, and then perform cleanup.

The code after `yield` is guaranteed to run when the resource goes out of scope.

```aivi
// Define a reusable resource
managedFile path = resource {
  handle = file.open path   // Acquire
  yield handle              // Provide to user
  handle.close ()           // Release
}
```

This declarative approach hides the complexity of error handling and cancellation checks.


## 15.2 Using Resources

Inside an `effect` block, you use the `<-` binder to acquire a resource. This is similar to the generator binder, but instead of iterating, it scopes the resource to the current block.

```aivi
main = effect {
  // Acquire resource
  f <- managedFile "data.txt"
  
  // Use resource
  content = f.readAll ()
  _ <- print content
} // f is automatically closed here
```

### Multiple Resources

You can acquire multiple resources in sequence. They will be released in reverse order of acquisition (LIFO).

```aivi
copy src dest = effect {
  input  <- managedFile src
  output <- managedFile dest
  
  input.copyTo output
}
```


## 15.3 Ad-hoc cleanup (inline `resource`)

Instead of a `defer` statement, define a small inline `resource` and bind it with `<-`. Cleanup happens automatically when the scope exits.

```aivi
main = effect {
  s <- resource {
    sock = socket.connect "localhost" 8080
    yield sock
    sock.close ()
  }

  _ <- s.send "Hello"
  Unit
}
```

### Guarantees

Cleanup code runs if:
1. The block completes successfully.
2. The block returns an error.
3. The task executing the block is **cancelled**.
