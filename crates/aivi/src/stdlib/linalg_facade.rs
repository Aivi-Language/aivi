pub const MODULE_NAME: &str = "aivi.linalg";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.linalg
export Vec, Mat
export dot, matMul, solve2x2
export LinearAlgebra

use aivi.linear_algebra"#;
