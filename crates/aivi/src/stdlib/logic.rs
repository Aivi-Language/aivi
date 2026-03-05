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
export Filterable
export Alternative, Plus

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

// 5b. Filtering

class Filterable (F A) = given (A: Any), Functor {
  filter: (A -> Bool) -> F A -> F A
}

// 5c. Alternatives

class Alternative (F A) = given (A: Any), Applicative {
  alt: F A -> F A -> F A
}

class Plus (F A) = given (A: Any), Alternative {
  zero: F A
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

// List

instance Functor (List A) = given (A: Any) {
  map: f xs => List.map f xs
}

instance Filterable (List A) = given (A: Any) {
  filter: pred xs => List.filter pred xs
}

instance Foldable (List A) = given (A: Any) {
  reduce: f init xs => List.foldl f init xs
}

instance Traversable (List A) = given (A: Any) {
  traverse: f xs => xs match
    | []           => pure []
    | [x, ...rest] => do Effect {
      y <- f x
      ys <- traverse f rest
      pure [y, ...ys]
    }
}

instance Apply (List A) = given (A: Any) {
  ap: fs xs => List.flatMap (f => List.map f xs) fs
}

instance Applicative (List A) = given (A: Any) {
  of: x => [x]
}

instance Chain (List A) = given (A: Any) {
  chain: f xs => List.flatMap f xs
}

instance Monad (List A) = given (A: Any) {}

instance Semigroup (List A) = given (A: Any) {
  concat: xs ys => xs match
    | []           => ys
    | [x, ...rest] => [x, ...concat rest ys]
}

instance Monoid (List A) = given (A: Any) {
  empty: []
}

instance Alternative (List A) = given (A: Any) {
  alt: ys xs => if List.isEmpty xs then ys else xs
}

instance Plus (List A) = given (A: Any) {
  zero: []
}

// Option additional instances

instance Filterable (Option A) = given (A: Any) {
  filter: pred opt => opt match
    | Some x => if pred x then Some x else None
    | None   => None
}

instance Foldable (Option A) = given (A: Any) {
  reduce: f init opt => opt match
    | Some x => f init x
    | None   => init
}

instance Traversable (Option A) = given (A: Any) {
  traverse: f opt => opt match
    | Some x => do Effect {
      y <- f x
      pure (Some y)
    }
    | None => pure None
}

instance Semigroup (Option A) = given (A: Any) {
  concat: a b => (a, b) match
    | (Some x, _) => Some x
    | (None, y)   => y
}

instance Alternative (Option A) = given (A: Any) {
  alt: fallback opt => opt match
    | Some x => Some x
    | None   => fallback
}

instance Plus (Option A) = given (A: Any) {
  zero: None
}

// Result additional instances

instance Foldable (Result E A) = given (A: Any) {
  reduce: f init res => res match
    | Ok x  => f init x
    | Err _ => init
}

instance Traversable (Result E A) = given (A: Any) {
  traverse: f res => res match
    | Ok x  => do Effect {
      y <- f x
      pure (Ok y)
    }
    | Err e => pure (Err e)
}

instance Bifunctor (Result E A) = given (A: Any) {
  bimap: f g res => res match
    | Ok x  => Ok (g x)
    | Err e => Err (f e)
}

instance Alternative (Result E A) = given (A: Any) {
  alt: fallback res => res match
    | Ok x  => Ok x
    | Err _ => fallback
}

// Map

instance Functor (Map K V) = given (V: Any) {
  map: f m => Map.map f m
}

instance Filterable (Map K V) = given (V: Any) {
  filter: pred m => Map.filterWithKey (_ v => pred v) m
}

instance Foldable (Map K V) = given (V: Any) {
  reduce: f init m => Map.foldWithKey (acc _ v => f acc v) init m
}

instance Semigroup (Map K V) = given (V: Any) {
  concat: a b => Map.union a b
}

instance Monoid (Map K V) = given (V: Any) {
  empty: Map.empty
}
"#;
