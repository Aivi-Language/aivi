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

class Semigroupoid (F A B) = given (A: Any, B: Any) {
  compose: F B C -> F A C
}

class Category (F A B) = given (A: Any, B: Any), Semigroupoid {
  id: F A A
}

// 4. Functional Mappings

class Functor (F A) = given (A: Any) {
  map: (A -> B) -> F B
}

class Apply (F A) = given (A: Any), Functor {
  ap: F (A -> B) -> F B
}

class Applicative (F A) = given (A: Any), Apply {
  of: A -> F A
}

class Chain (F A) = given (A: Any), Apply {
  chain: (A -> F B) -> F B
}

class Monad (M A) = given (A: Any), Applicative, Chain {}

// 5. Folds and Traversals

class Foldable (F A) = given (A: Any) {
  reduce: (B -> A -> B) -> B -> B
}

class Traversable (T A) = given (A: Any), Functor, Foldable {
  traverse: (A -> F B) -> F (T B)
}

// 6. Higher-Order Mappings

class Bifunctor (F A B) = given (A: Any, B: Any) {
  bimap: (A -> C) -> (B -> D) -> F C D
}

class Profunctor (F A B) = given (A: Any, B: Any) {
  promap: (A -> B) -> (C -> D) -> F A D
}

// ------------------------------------------------------------
// Core ADT instances
// ------------------------------------------------------------

// Option

instance Functor (Option A) = given (A: Any) {
  map: f opt =>
    opt match
      | None   => None
      | Some x => Some (f x)
}

instance Apply (Option A) = given (A: Any) {
  ap: fOpt opt =>
    (fOpt, opt) match
      | (Some f, Some x) => Some (f x)
      | _                => None
}

instance Applicative (Option A) = given (A: Any) {
  of: Some
}

instance Chain (Option A) = given (A: Any) {
  chain: f opt =>
    opt match
      | None   => None
      | Some x => f x
}

instance Monad (Option A) = given (A: Any) {}

// Result

instance Functor (Result E A) = given (A: Any) {
  map: f res =>
    res match
      | Ok x  => Ok (f x)
      | Err e => Err e
}

instance Apply (Result E A) = given (A: Any) {
  ap: fRes xRes =>
    (fRes, xRes) match
      | (Ok f, Ok x)   => Ok (f x)
      | (Err e, _)     => Err e
      | (_, Err e)     => Err e
}

instance Applicative (Result E A) = given (A: Any) {
  of: Ok
}

instance Chain (Result E A) = given (A: Any) {
  chain: f res =>
    res match
      | Ok x  => f x
      | Err e => Err e
}

instance Monad (Result E A) = given (A: Any) {}
"#;
