# Records (kernel)

## 3.1 Record construction

```text
{ l₁ = e₁, l₂ = e₂ }
```


## 3.2 Record projection

```text
e.l
```


## 3.3 Record update (primitive)

```text
update(e, l, f)
```

Semantics:

* apply `f` to field `l` if present
* otherwise insert if allowed by row type

> **This single primitive underlies patching**
