pub const MODULE_NAME: &str = "aivi.crypto";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.crypto
export sha256, sha384, sha512
export hmacSha256, hmacSha512, hmacVerify
export hashPassword, verifyPassword
export randomUuid, randomBytes
export secureEquals, fromHex, toHex

use aivi

sha256 : Text -> Text
sha256 = value => crypto.sha256 value

sha384 : Text -> Text
sha384 = value => crypto.sha384 value

sha512 : Text -> Text
sha512 = value => crypto.sha512 value

hmacSha256 : Bytes -> Bytes -> Bytes
hmacSha256 = key msg => crypto.hmacSha256 key msg

hmacSha512 : Bytes -> Bytes -> Bytes
hmacSha512 = key msg => crypto.hmacSha512 key msg

hmacVerify : Bytes -> Bytes -> Bytes -> Bool
hmacVerify = key msg tag => crypto.hmacVerify key msg tag

hashPassword : Text -> Effect Text Text
hashPassword = password => crypto.hashPassword password

verifyPassword : Text -> Text -> Effect Text Bool
verifyPassword = password hash => crypto.verifyPassword password hash

randomUuid : Effect Text Text
randomUuid = crypto.randomUuid Unit

randomBytes : Int -> Effect Text Bytes
randomBytes = count => crypto.randomBytes count

secureEquals : Bytes -> Bytes -> Bool
secureEquals = a b => crypto.secureEquals a b

fromHex : Text -> Result Text Bytes
fromHex = hex => crypto.fromHex hex

toHex : Bytes -> Text
toHex = bytes => crypto.toHex bytes
"#;
