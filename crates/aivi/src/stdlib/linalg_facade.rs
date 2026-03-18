pub const MODULE_NAME: &str = "aivi.linalg";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.linalg
export Vec, Mat
export dot, matMul, solve2x2
export domain LinearAlgebra

use aivi
use aivi.linear_algebra (Vec, Mat, dot, matMul, solve2x2)

map : (A -> B) -> List A -> List B
map = f items => List.map f items

zipWith : (A -> B -> C) -> List A -> List B -> List C
zipWith = f left right => (left, right) match
  | ([], _) => []
  | (_, []) => []
  | ([x, ...xs], [y, ...ys]) => [f x y, ...zipWith f xs ys]

add : Float -> Float -> Float
add = a b => a + b

sub : Float -> Float -> Float
sub = a b => a - b

domain LinearAlgebra over Vec = {
  (+) : Vec -> Vec -> Vec
  (+) = a b => { size: a.size, data: zipWith add a.data b.data }

  (-) : Vec -> Vec -> Vec
  (-) = a b => { size: a.size, data: zipWith sub a.data b.data }

  (*) : Vec -> Float -> Vec
  (*) = v s => { size: v.size, data: map (_ * s) v.data }
}"#;
