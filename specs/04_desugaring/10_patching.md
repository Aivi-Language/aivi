# Record patching `<|` (Path : Instruction)

Kernel record primitives:

* `update(e, l, f)` : update/insert field `l` by applying `f` to old value (or a sentinel for missing)
* `delete(e, l)` : remove field `l` (row shrink)

For nested paths, desugar into nested `update`/`delete`.

## Path compilation (dot paths)

| Surface | Desugaring |
| :--- | :--- |
| `r <| { a: v }` | `update ⟦r⟧ "a" (λ_. ⟦v⟧)` (replace/insert) |
| `r <| { a: f }` where `f` is a function | `update ⟦r⟧ "a" ⟦f⟧` (transform) |
| `r <| { a: - }` | `delete ⟦r⟧ "a"` |

Nested:

| Surface | Desugaring |
| :--- | :--- |
| `r <| { a.b: v }` | `update ⟦r⟧ "a" (λa0. update a0 "b" (λ_. ⟦v⟧))` |
| `r <| { a.b: f }` | `update ⟦r⟧ "a" (λa0. update a0 "b" ⟦f⟧)` |
| `r <| { a.b: - }` | `update ⟦r⟧ "a" (λa0. delete a0 "b")` |

## Function-as-data disambiguation `:=`

| Surface | Desugaring |
| :--- | :--- |
| `path: := (λx. e)` | `update carrier path (λ_. (λx. ⟦e⟧))` and mark as “value replacement” (no transform) |

Formally:

* `path: f` (function) → transform
* `path: := f` → replace with function value

## Automatic lifting for patch instructions

Patch instruction `instr` is lifted when the targeted field type is `Option T` or `Result E T`.

Let `L(instr)` be the lifted instruction:

* If field is `T`: apply instruction normally.
* If field is `Option T`: `mapOption instr`
* If field is `Result E T`: `mapResult instr`

Desugaring (conceptual) for transform instruction `f`:

* `Option`: `λopt. case opt of \| Some x -> Some (f x) \| None -> None`
* `Result`: `λres. case res of \| Ok x -> Ok (f x) \| Err e -> Err e`

Replacement (`value`) replaces the whole container unless explicitly targeted deeper (e.g. `.Some.val`).

## Traversals `[*]`

Path segment `items[*].price: f` desugars to `map` over list plus nested patch.

| Surface | Desugaring |
| :--- | :--- |
| `items[*].price: f` | `update r "items" (λxs. map (λit. update it "price" ⟦f⟧) xs)` |

## Predicate traversal `items[pred]`

Predicate segments desugar to **map with conditional update**.

| Surface | Desugaring |
| :--- | :--- |
| `items[pred].price: f` | `update r "items" (λxs. map (λit. case (⟦pred→λ⟧ it) of \| True -> update it "price" ⟦f⟧ \| False -> it) xs)` |

Predicate `pred` uses the unified predicate desugaring table (Section 8).

## Sum-type focus (prisms)

`Ok.value: f` desugars to a constructor check and selective update.

| Surface | Desugaring |
| :--- | :--- |
| `Ok.value: f` | `λres. case res of \| Ok v -> Ok (update v "value" ⟦f⟧) \| _ -> res` (record payload) |
| `Some.val: f` | `λopt. case opt of \| Some v -> Some (update v "val" ⟦f⟧) \| _ -> opt` |

For constructors with direct payload (not record), `value` refers to the payload position.

# Summary: smallest set of kernel primitives assumed

* `λ`, application
* `let`
* `case` + patterns (including `@`)
* ADT constructors
* records + projection + `update` + `delete`
* `fold` (List)
* `Effect` with `bind/pure`
* compile-time elaboration for classes (dictionary passing)
* compile-time rewrite for domains (operator resolution)
