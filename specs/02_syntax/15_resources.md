# Resource Management

AIVI provides a dedicated `Resource` type to manage lifecycles (setup and teardown) in a declarative way. This ensures that resources like files, sockets, and database connections are always reliably released, even in the event of errors or task cancellation.


## 15.1 The `Resource E A` Type

`Resource E A` is a value that describes how to **acquire** a handle of type `A`, **use** it, and **release** it. Like `Effect E A`, it carries an error type `E` representing what can go wrong during acquisition.

```text
Resource E A
```

- `E`   the error type for acquisition failures (e.g. `FileError`, `SocketError`).
- `A`   the type of the acquired handle (e.g. `Handle`, `Socket`).

A `Resource` is **not** a handle itself   it is a *recipe* for obtaining one. The handle only exists within the scope where the resource is acquired.


## 15.2 Defining Resources

Resources are defined using `resource` blocks. The syntax is analogous to generators: you perform setup, `yield` the resource, and then perform cleanup.

The code after `yield` is guaranteed to run when the resource goes out of scope.

<<< ../snippets/from_md/02_syntax/15_resources/block_01.aivi{aivi}

This declarative approach hides the complexity of error handling and cancellation checks.

### Rules

- A `resource` block must contain exactly **one** `yield` statement. This separates the acquisition phase (before `yield`) from the cleanup phase (after `yield`).
- If `yield` is never reached (e.g. acquisition fails with an error), no cleanup code runs   there is nothing to clean up.
- The cleanup phase runs as a finalizer and **may perform effects** (e.g. closing a file handle, flushing a buffer). Cleanup effects use the same error type `E`; if cleanup itself fails, the error is logged but does not override the original error.


## 15.3 Using Resources

Inside an `effect` block, you use the `<-` binder to acquire a resource. This scopes the resource handle to the enclosing block.

<<< ../snippets/from_md/02_syntax/15_resources/block_02.aivi{aivi}

When the `effect` block exits   whether by normal completion, an error in `E`, or **cancellation**   all acquired resources are released in reverse order.

### Multiple Resources

You can acquire multiple resources in sequence. They will be released in reverse order of acquisition (LIFO).

<<< ../snippets/from_md/02_syntax/15_resources/block_03.aivi{aivi}


## 15.4 Error Semantics

- If **acquisition** fails (the code before `yield` raises `E`), the resource is never yielded and no cleanup runs.
- If **use** fails (the code after `<-` acquisition raises an error), cleanup runs normally. The original error propagates after cleanup completes.
- If **cleanup** fails, the cleanup error is suppressed (logged to diagnostics). The original error (if any) takes priority.

All guarantees hold regardless of whether the failure is a typed error (`E`) or a cancellation signal.


## 15.5 Cancellation

Resources interact with the cancellation system (see [Concurrency](../06_runtime/01_concurrency.md)):

- Cancellation is checked at `<-` bind points. If a task is cancelled before a resource is acquired, acquisition does not run.
- If cancellation arrives **during use** of an acquired resource, cleanup still runs. The resource block's finalizer is registered at acquisition time and cannot be skipped.
- Cleanup code itself runs in a **cancellation-protected** context   it will not be interrupted by a second cancellation signal.


## 15.6 Composability and Nesting

Resources compose naturally:

- A `resource` block can acquire other resources internally. Inner resources are released before the outer resource's cleanup runs.
- Resources can be returned from functions and passed as values   they are inert descriptions until acquired with `<-`.
- You can build higher-level resources from lower-level ones by combining acquisition and cleanup steps.
