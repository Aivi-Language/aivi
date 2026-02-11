pub const MODULE_NAME: &str = "aivi.linear_algebra";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.linear_algebra = {
  export Vec, Mat
  export dot, matMul, solve2x2
  export domain LinearAlgebra

  use aivi

  Vec = { size: Int, data: List Float }
  Mat = { rows: Int, cols: Int, data: List Float }

  map : (A -> B) -> List A -> List B
  map f items = items ?
    | [] => []
    | [x, ...xs] => [f x, ...map f xs]

  zipWith : (A -> B -> C) -> List A -> List B -> List C
  zipWith f left right = (left, right) ?
    | ([], _) => []
    | (_, []) => []
    | ([x, ...xs], [y, ...ys]) => [f x y, ...zipWith f xs ys]

  add : Float -> Float -> Float
  add a b = a + b

  sub : Float -> Float -> Float
  sub a b = a - b

  domain LinearAlgebra over Vec = {
    (+) : Vec -> Vec -> Vec
    (+) a b = { size: a.size, data: zipWith add a.data b.data }

    (-) : Vec -> Vec -> Vec
    (-) a b = { size: a.size, data: zipWith sub a.data b.data }

    (*) : Vec -> Float -> Vec
    (*) v s = { size: v.size, data: map (_ * s) v.data }
  }

  dot : Vec -> Vec -> Float
  dot a b = linalg.dot a b

  matMul : Mat -> Mat -> Mat
  matMul a b = linalg.matMul a b

  solve2x2 : Mat -> Vec -> Vec
  solve2x2 m v = linalg.solve2x2 m v
}
"#;
