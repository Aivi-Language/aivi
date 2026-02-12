pub const MODULE_NAME: &str = "aivi.i18n";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.i18n
export Locale, Key, Message, Bundle
export parseLocale, key, message, render
export bundleFromProperties, bundleFromPropertiesFile
export keyText, messageText
export tResult, tOpt, t, tWithFallback

use aivi

type Locale = { language: Text, region: Option Text, variants: List Text, tag: Text }
type Key = { tag: Text, body: Text, flags: Text }
type Message = { tag: Text, body: Text, flags: Text }
type Bundle = { locale: Locale, entries: Map Text Message }

parseLocale : Text -> Result Text Locale
parseLocale tag = i18n.parseLocale tag

key : Text -> Result Text Key
key text = i18n.key text

message : Text -> Result Text Message
message text = i18n.message text

render : Message -> {} -> Result Text Text
render msg args = i18n.render msg args

bundleFromProperties : Locale -> Text -> Result Text Bundle
bundleFromProperties locale props = i18n.bundleFromProperties locale props

bundleFromPropertiesFile : Locale -> Text -> Effect Text (Result Text Bundle)
bundleFromPropertiesFile locale path = effect {
  res <- attempt (file.read path)
  res ?
    | Err e => pure (Err e)
    | Ok txt => pure (bundleFromProperties locale txt)
}

keyText : Key -> Text
keyText k = k.body

messageText : Message -> Text
messageText m = m.body

tResult : Bundle -> Key -> {} -> Result Text Text
tResult bundle k args =
  Map.get (keyText k) bundle.entries ?
    | None => Err (text.concat ["missing key: ", keyText k])
    | Some msg => render msg args

tOpt : Bundle -> Key -> {} -> Option Text
tOpt bundle k args =
  (tResult bundle k args) ?
    | Ok txt => Some txt
    | Err _  => None

t : Bundle -> Key -> {} -> Text
t bundle k args =
  (tResult bundle k args) ?
    | Ok txt => txt
    | Err _  => keyText k

tWithFallback : List Bundle -> Key -> {} -> Text
tWithFallback bundles k args = bundles ?
  | [] => keyText k
  | [b, ...rest] =>
    (tOpt b k args) ?
      | Some txt => txt
      | None => tWithFallback rest k args
"#;
