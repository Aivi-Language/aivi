pub const MODULE_NAME: &str = "aivi.system";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.system
export env, args, exit, localeTag

use aivi

env = {
  get: key => load (system.env.get key)
  decode: prefix => load (system.env.decode prefix)
  set: key value => system.env.set key value
  remove: key => system.env.remove key
}

args : Effect Text (List Text)
args = system.args Unit

localeTag : Effect Text (Option Text)
localeTag = system.localeTag Unit

exit : Int -> Effect Text Unit
exit = code => system.exit code
"#;
