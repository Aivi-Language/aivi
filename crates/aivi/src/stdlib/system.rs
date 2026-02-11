pub const MODULE_NAME: &str = "aivi.system";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.system = {
  export env, args, exit

  use aivi

  env = system.env

  args : Effect Text (List Text)
  args = system.args Unit

  exit : Int -> Effect Text Unit
  exit code = system.exit code
}
"#;
