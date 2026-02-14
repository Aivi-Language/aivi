pub const MODULE_NAME: &str = "aivi.crypto";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.crypto
export sha256, randomUuid, randomBytes

use aivi

sha256 : Text -> Text
sha256 = value => crypto.sha256 value

randomUuid : Effect Text Text
randomUuid = crypto.randomUuid Unit

randomBytes : Int -> Effect Text Bytes
randomBytes = count => crypto.randomBytes count
"#;
