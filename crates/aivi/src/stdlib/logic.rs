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

class Ord A = {
  equals: A -> A -> Bool
  lte: A -> A -> Bool
}

// 2. Monoids and Semigroups

class Semigroup A = {
  concat: A -> A -> A
}

class Monoid A = {
  concat: A -> A -> A
  empty: A
}

class Group A = {
  concat: A -> A -> A
  empty: A
  invert: A -> A
}

// 3. Categories

class Semigroupoid (F * *) = {
  compose: F B C -> F A B -> F A C
}

class Category (F * *) = {
  compose: F B C -> F A B -> F A C
  id: F A A
}

// 4. Functional Mappings

class Functor (F *) = {
  map: (A -> B) -> F A -> F B
}

class Apply (F *) = {
  map: (A -> B) -> F A -> F B
  ap: F (A -> B) -> F A -> F B
}

class Applicative (F *) = {
  map: (A -> B) -> F A -> F B
  ap: F (A -> B) -> F A -> F B
  of: A -> F A
}

class Chain (F *) = {
  map: (A -> B) -> F A -> F B
  ap: F (A -> B) -> F A -> F B
  chain: (A -> F B) -> F A -> F B
}

class Monad (M *) = {
  map: (A -> B) -> M A -> M B
  ap: M (A -> B) -> M A -> M B
  of: A -> M A
  chain: (A -> M B) -> M A -> M B
}

// 5. Folds and Traversals

class Foldable (F *) = {
  reduce: (B -> A -> B) -> B -> F A -> B
}

class Traversable (T *) = {
  map: (A -> B) -> T A -> T B
  reduce: (B -> A -> B) -> B -> T A -> B
  traverse: (Applicative F) => (A -> F B) -> T A -> F (T B)
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
    opt ?
      | None   => None
      | Some x => Some (f x)
}

instance Apply (Option *) = {
  map: f opt =>
    opt ?
      | None   => None
      | Some x => Some (f x)

  ap: fOpt opt =>
    (fOpt, opt) ?
      | (Some f, Some x) => Some (f x)
      | _                => None
}

instance Applicative (Option *) = {
  map: f opt =>
    opt ?
      | None   => None
      | Some x => Some (f x)

  ap: fOpt opt =>
    (fOpt, opt) ?
      | (Some f, Some x) => Some (f x)
      | _                => None

  of: Some
}

instance Chain (Option *) = {
  map: f opt =>
    opt ?
      | None   => None
      | Some x => Some (f x)

  ap: fOpt opt =>
    (fOpt, opt) ?
      | (Some f, Some x) => Some (f x)
      | _                => None

  chain: f opt =>
    opt ?
      | None   => None
      | Some x => f x
}

instance Monad (Option *) = {
  map: f opt =>
    opt ?
      | None   => None
      | Some x => Some (f x)

  ap: fOpt opt =>
    (fOpt, opt) ?
      | (Some f, Some x) => Some (f x)
      | _                => None

  of: Some

  chain: f opt =>
    opt ?
      | None   => None
      | Some x => f x
}

// Result

instance Functor (Result E *) = {
  map: f res =>
    res ?
      | Ok x  => Ok (f x)
      | Err e => Err e
}

instance Apply (Result E *) = {
  map: f res =>
    res ?
      | Ok x  => Ok (f x)
      | Err e => Err e

  ap: fRes xRes =>
    (fRes, xRes) ?
      | (Ok f, Ok x)   => Ok (f x)
      | (Err e, _)     => Err e
      | (_, Err e)     => Err e
}

instance Applicative (Result E *) = {
  map: f res =>
    res ?
      | Ok x  => Ok (f x)
      | Err e => Err e

  ap: fRes xRes =>
    (fRes, xRes) ?
      | (Ok f, Ok x)   => Ok (f x)
      | (Err e, _)     => Err e
      | (_, Err e)     => Err e

  of: Ok
}

instance Chain (Result E *) = {
  map: f res =>
    res ?
      | Ok x  => Ok (f x)
      | Err e => Err e

  ap: fRes xRes =>
    (fRes, xRes) ?
      | (Ok f, Ok x)   => Ok (f x)
      | (Err e, _)     => Err e
      | (_, Err e)     => Err e

  chain: f res =>
    res ?
      | Ok x  => f x
      | Err e => Err e
}

instance Monad (Result E *) = {
  map: f res =>
    res ?
      | Ok x  => Ok (f x)
      | Err e => Err e

  ap: fRes xRes =>
    (fRes, xRes) ?
      | (Ok f, Ok x)   => Ok (f x)
      | (Err e, _)     => Err e
      | (_, Err e)     => Err e

  of: Ok

  chain: f res =>
    res ?
      | Ok x  => f x
      | Err e => Err e
}

// List

append = xs ys =>
  xs ?
    | []        => ys
    | [h, ...t] => [h, ...append t ys]

mapList = f xs =>
  xs ?
    | []        => []
    | [h, ...t] => [f h, ...mapList f t]

concatMap = f xs =>
  xs ?
    | []        => []
    | [h, ...t] => append (f h) (concatMap f t)

instance Functor (List *) = {
  map: f xs => mapList f xs
}

instance Apply (List *) = {
  map: f xs => mapList f xs
  ap: fs xs => concatMap (f => mapList f xs) fs
}

instance Applicative (List *) = {
  map: f xs => mapList f xs
  ap: fs xs => concatMap (f => mapList f xs) fs
  of: x => [x]
}

instance Chain (List *) = {
  map: f xs => mapList f xs
  ap: fs xs => concatMap (f => mapList f xs) fs
  chain: f xs => concatMap f xs
}

instance Monad (List *) = {
  map: f xs => mapList f xs
  ap: fs xs => concatMap (f => mapList f xs) fs
  of: x => [x]
  chain: f xs => concatMap f xs
}
"#;
