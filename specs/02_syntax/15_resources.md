# Resource Management

AIVI ensures that resources (files, sockets, memory) are always reliably released, even in the event of errors or task cancellation.

---

## 15.1 The `bracket` Primitive

The `bracket` function is the fundamental building block for safe resource management. It guarantees that the release operation is executed regardless of the outcome of the use operation.

```text
bracket : Effect ε A -> (A -> Effect ε Unit) -> (A -> Effect ε B) -> Effect ε B
```

```aivi
effect { 
  contents = bracket
    (file.open "data.txt") // Acquire
    (f => f.close ())      // Release
    (f => f.readAll ())    // Use
}
```

---

## 15.2 The `defer` Keyword (LIFO Sugar)

The `defer` keyword provides a more ergonomic way to release resources within an `effect` block. Deferred operations are executed in **Last-In, First-Out (LIFO)** order when the block exits.

```aivi
copyFile = src dest => effect {
  s = file.open src
  defer s.close ()
  
  d = file.create dest
  defer d.close ()
  
  s.copyTo d
}
```

### Guarantees

Deferred operations are guaranteed to run if:
1. The block completes successfully.
2. The block returns an error.
3. The task executing the block is **cancelled**.

This ensures that AIVI code is "safe by default" against leaks.
