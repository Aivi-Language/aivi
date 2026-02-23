pub const MODULE_NAME: &str = "aivi.logic";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.logic
export Setoid, Ord
export Semigroup, Monoid, Group
export Semigroupoid, Category
export Functor, Apply, Applicative, Chain, Monad
export Foldable, Traversable
export Bifunctor, Profunctor

use aivi

// 1. Equality and Ordering

class Setoid A = {
  equals: A -> A -> Bool
}

class Ord A = Setoid {
  lte: A -> A -> Bool
}

// 2. Monoids and Semigroups

class Semigroup A = {
  concat: A -> A -> A
}

class Monoid A = Semigroup {
  empty: A
}

class Group A = Monoid {
  invert: A -> A
}

// 3. Categories

class Semigroupoid (F * *) = {
  compose: F B C -> F A B -> F A C
}

class Category (F * *) = Semigroupoid {
  id: F A A
}

// 4. Functional Mappings

class Functor (F *) = {
  map: (A -> B) -> F A -> F B
}

class Apply (F *) = Functor {
  ap: F (A -> B) -> F A -> F B
}

class Applicative (F *) = Apply {
  of: A -> F A
}

class Chain (F *) = Apply {
  chain: (A -> F B) -> F A -> F B
}

class Monad (M *) = Applicative, Chain {}

// 5. Folds and Traversals

class Foldable (F *) = {
  reduce: (B -> A -> B) -> B -> F A -> B
}

class Traversable (T *) = Functor, Foldable {
  traverse: (A -> F B) -> T A -> F (T B)
}

// 6. Higher-Order Mappings

class Bifunctor (F * *) = {
  bimap: (A -> C) -> (B -> D) -> F A B -> F C D
}

class Profunctor (F * *) = {
  promap: (A -> B) -> (C -> D) -> F B C -> F A D
}

// ------------------------------------------------------------
// Core ADT instances
// ------------------------------------------------------------

// Option

instance Functor (Option *) = {
  map: f opt =>
    opt match
      | None   => None
      | Some x => Some (f x)
}

instance Apply (Option *) = {
  ap: fOpt opt =>
    (fOpt, opt) match
      | (Some f, Some x) => Some (f x)
      | _                => None
}

instance Applicative (Option *) = {
  of: Some
}

instance Chain (Option *) = {
  chain: f opt =>
    opt match
      | None   => None
      | Some x => f x
}

instance Monad (Option *) = {}

// Result

instance Functor (Result E *) = {
  map: f res =>
    res match
      | Ok x  => Ok (f x)
      | Err e => Err e
}

instance Apply (Result E *) = {
  ap: fRes xRes =>
    (fRes, xRes) match
      | (Ok f, Ok x)   => Ok (f x)
      | (Err e, _)     => Err e
      | (_, Err e)     => Err e
}

instance Applicative (Result E *) = {
  of: Ok
}

instance Chain (Result E *) = {
  chain: f res =>
    res match
      | Ok x  => f x
      | Err e => Err e
}

instance Monad (Result E *) = {}
"#;
