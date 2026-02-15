> 1.2.1 Recursion (module level)
> Local recursion inside { ... } / effect { ... } blocks is a future surface feature; in v0.1, prefer defining recursive helpers at module scope.

what does this mean?

> formatUser = u@{ id, name } => "{id}: {name}"

should also `formatUser = "{id}: {name}"` work, if type def is given?

> { data: { user: { profile: p@{ name } } } } = response

what's the p@ here, why not 
`{ data: { user: { profile: { name } } } } = response`

should we remove `add x y = x + y` shortcut syntax? what are the implications?

rename const `const = x _ => x`

> NOTE:
> Predicates can also perform complex transformations by deconstructing multiple fields: map { name, id } => if id > 10 then name else "no name"

this makes no sense

>  1d = Day 1

This is a bit weird def syntax. What alternatives?

> This requires the domain to be in scope (e.g. use aivi.color (domain Color)), not just the carrier type.

aren't aivi.color auto importing domains?

> use aivi.calendar as Cal
> use vendor.legacy.math (v1_add as add)

do we have tests for this?

> renderCount = { count * 23 sasd, adsad step } =>

lsp server is bad at spotting garbage code...how to fix. it should complain about bad destructors, unknown symbols etc.
