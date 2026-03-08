# Resource Management

AIVI provides a dedicated `Resource` type for values that need reliable setup and teardown. Use it for things like files, sockets, database connections, or any other handle that must be released even when work fails or gets cancelled.

## 15.1 The `Resource E A` Type

`Resource E A` is a value that describes how to **acquire** a handle of type `A` and later **release** it around a caller-supplied use site.

```text
Resource E A
```

- `E` is the error type for acquisition failures, such as `FileError` or `SocketError`
- `A` is the type of the acquired handle, such as `Handle` or `Socket`

A `Resource` is **not** a handle itself. It is an inert recipe for obtaining one. The actual use happens after acquisition with `<-`, and the handle exists only within that enclosing scope.

## 15.2 Defining Resources

Define a resource with a `resource` block. The shape is simple: perform setup, `yield` the resource to the caller, then write cleanup after `yield`.

The code after `yield` is guaranteed to run when the resource goes out of scope.

<<< ../snippets/from_md/syntax/resources/defining_resources.aivi{aivi}

Think of `yield` as the handoff point between “make the handle available” and “clean it up later”.

### Rules

- write acquisition before `yield` and cleanup after it; a well-formed resource uses `yield` as its single handoff point
- if `yield` is never reached, such as when acquisition fails, no cleanup runs because there is nothing to release
- the cleanup phase runs as a finalizer and **may perform effects**
- cleanup effects use the same error type `E`; if cleanup itself fails, the error is logged but does not override the original error

## 15.3 Using Resources

Inside a `do Effect { ... }` block, use `<-` to acquire a resource. This binds the handle for the rest of that enclosing scope.

<<< ../snippets/from_md/syntax/resources/using_resources.aivi{aivi}

When the enclosing scope exits—typically the surrounding `do Effect { ... }` block—whether by normal completion, an error in `E`, or cancellation, all acquired resources are released in reverse order.

### Multiple Resources

You can acquire multiple resources in sequence. They are released in reverse order of acquisition (LIFO).

<<< ../snippets/from_md/syntax/resources/multiple_resources.aivi{aivi}

## 15.4 Error Semantics

- if **acquisition** fails, the resource is never yielded and no cleanup runs
- if **use** fails after acquisition, cleanup still runs and the original error propagates afterward
- if **cleanup** fails, the cleanup error is suppressed to diagnostics and the original error, if any, takes priority

All of these guarantees hold for typed errors and for cancellation.

## 15.5 Cancellation

Resources interact with the cancellation system (see [Concurrency](../stdlib/system/concurrency.md)):

- cancellation is checked at `<-` bind points; if a task is cancelled before acquisition, acquisition does not run
- if cancellation arrives **during use** of an acquired resource, cleanup still runs
- cleanup code itself runs in a **cancellation-protected** context and is not interrupted by a second cancellation signal
- this masking is structural, so ordinary finalizer safety does not require explicit `cancellation.mask`

## 15.6 Composability and Nesting

Resources compose naturally:

- a `resource` block can acquire other resources internally
- inner resources are released before the outer resource's cleanup runs
- resources can be returned from functions and passed as values; they stay inert until acquired with `<-`
- higher-level resources can be built by combining lower-level acquisition and cleanup steps
