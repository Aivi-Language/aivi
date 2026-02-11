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

class Ord A =
  Setoid A & {
    lte: A -> A -> Bool
  }

// 2. Monoids and Semigroups

class Semigroup A = {
  concat: A -> A -> A
}

class Monoid A =
  Semigroup A & {
    empty: A
  }

class Group A =
  Monoid A & {
    invert: A -> A
  }

// 3. Categories

class Semigroupoid (F * *) = {
  compose: F B C -> F A B -> F A C
}

class Category (F * *) =
  Semigroupoid F & {
    id: F A A
  }

// 4. Functional Mappings

class Functor (F *) = {
  map: (A -> B) -> F A -> F B
}

class Apply (F *) =
  Functor F & {
    ap: F (A -> B) -> F A -> F B
  }

class Applicative (F *) =
  Apply F & {
    of: A -> F A
  }

class Chain (F *) =
  Apply F & {
    chain: (A -> F B) -> F A -> F B
  }

class Monad (M *) =
  Applicative M & Chain M

// 5. Folds and Traversals

class Foldable (F *) = {
  reduce: (B -> A -> B) -> B -> F A -> B
}

class Traversable (T *) =
  Functor T & Foldable T & {
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

instance Apply (Option *) =
  Functor (Option *) & {
    ap: fOpt opt =>
      (fOpt, opt) ?
        | (Some f, Some x) => Some (f x)
        | _                => None
  }

instance Applicative (Option *) =
  Apply (Option *) & {
    of: Some
  }

instance Chain (Option *) =
  Apply (Option *) & {
    chain: f opt =>
      opt ?
        | None   => None
        | Some x => f x
  }

instance Monad (Option *) =
  Applicative (Option *) & Chain (Option *)

// Result

instance Functor (Result E *) = {
  map: f res =>
    res ?
      | Ok x  => Ok (f x)
      | Err e => Err e
}

instance Apply (Result E *) =
  Functor (Result E *) & {
    ap: fRes xRes =>
      (fRes, xRes) ?
        | (Ok f, Ok x)   => Ok (f x)
        | (Err e, _)     => Err e
        | (_, Err e)     => Err e
  }

instance Applicative (Result E *) =
  Apply (Result E *) & {
    of: Ok
  }

instance Chain (Result E *) =
  Apply (Result E *) & {
    chain: f res =>
      res ?
        | Ok x  => f x
        | Err e => Err e
  }

instance Monad (Result E *) =
  Applicative (Result E *) & Chain (Result E *)

// List

append = xs ys =>
  xs ?
    | []        => ys
    | [h, ...t] => [h, ...append t ys]

mapList = f xs =>
  xs ?
    | []        => []
    | [h, ...t] => [f h, ...mapList t f]

concatMap = f xs =>
  xs ?
    | []        => []
    | [h, ...t] => append (f h) (concatMap f t)

instance Functor (List *) = {
  map: f xs => mapList f xs
}

instance Apply (List *) =
  Functor (List *) & {
    ap: fs xs => concatMap (f => mapList f xs) fs
  }

instance Applicative (List *) =
  Apply (List *) & {
    of: x => [x]
  }

instance Chain (List *) =
  Apply (List *) & {
    chain: f xs => concatMap f xs
  }

instance Monad (List *) =
  Applicative (List *) & Chain (List *)
"#;
