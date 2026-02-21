# Resources

`resource { … }` blocks desugar into kernel `bracket` calls (see [Kernel §8.7](../kernel/effects.md)).

## Resource acquisition

A basic resource block:

<<< ../snippets/from_md/desugaring/generators/resource_acquisition.aivi{aivi}

desugars to:

```text
bracket ⟦acquire⟧ (λhandle. ⟦cleanup⟧) (λhandle. ⟦body using handle⟧)
```

where `bracket` is the kernel primitive:

```text
bracket : Effect E A → (A → Effect E Unit) → (A → Effect E B) → Effect E B
```

## Resource sequencing

When multiple resources are acquired in a single `resource { … }` block, they desugar to nested `bracket` calls. Inner resources are released before outer resources (stack order):

```text
resource {
  a <- acquire1
  b <- acquire2
  body
}
```

desugars to:

```text
bracket ⟦acquire1⟧ (λa. ⟦release1⟧)
  (λa. bracket ⟦acquire2⟧ (λb. ⟦release2⟧)
    (λb. ⟦body⟧))
```

## Yield-based resource syntax

The `resource { … }` block supports exactly one `yield` expression, which provides the handle to the caller. The code after `yield` runs as cleanup:

```text
resource {
  h <- acquireHandle
  yield h
  releaseHandle h
}
```

desugars to:

```text
bracket (⟦acquireHandle⟧) (λh. ⟦releaseHandle h⟧) (λh. pure h)
```

See [Syntax §15](../syntax/resources.md) for the surface syntax and error semantics.
