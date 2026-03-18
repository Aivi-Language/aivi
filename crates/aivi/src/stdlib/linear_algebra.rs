pub const MODULE_NAME: &str = "aivi.linear_algebra";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.linear_algebra
export Vec, Mat
export dot, matMul, solve2x2
export domain LinearAlgebra

use aivi

Vec = { size: Int, data: List Float }
Mat = { rows: Int, cols: Int, data: List Float }

domain LinearAlgebra over Vec = {
  (+) : Vec -> Vec -> Vec
  (+) = a b => linalg.addVec a b

  (-) : Vec -> Vec -> Vec
  (-) = a b => linalg.subVec a b

  (*) : Vec -> Float -> Vec
  (*) = v s => linalg.scaleVec v s
}

dot : Vec -> Vec -> Float
dot = a b => linalg.dot a b

matMul : Mat -> Mat -> Mat
matMul = a b => linalg.matMul a b

solve2x2 : Mat -> Vec -> Vec
solve2x2 = m v => linalg.solve2x2 m v
"#;
