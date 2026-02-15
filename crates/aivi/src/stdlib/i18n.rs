pub const MODULE_NAME: &str = "aivi.i18n";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.i18n
export Locale, Key, Message, Bundle, Catalog
export parseLocale, key, message, render
export bundleFromProperties, bundleFromPropertiesFile
export keyText, messageText
export tResult, tOpt, t, tWithFallback
export fallbackTags, catalogFromBundles, bundlesForLocale, bundleForLocale
export tCatalog, tCatalogWithDefault

use aivi

Locale = { language: Text, region: Option Text, variants: List Text, tag: Text }
Key = { tag: Text, body: Text, flags: Text }
Message = { tag: Text, body: Text, flags: Text }
Bundle = { locale: Locale, entries: Map Text Message }
Catalog = Map Text Bundle

parseLocale : Text -> Result Text Locale
parseLocale = tag => i18n.parseLocale tag

key : Text -> Result Text Key
key = text => i18n.key text

message : Text -> Result Text Message
message = text => i18n.message text

render : Message -> {} -> Result Text Text
render = msg args => i18n.render msg args

bundleFromProperties : Locale -> Text -> Result Text Bundle
bundleFromProperties = locale props => i18n.bundleFromProperties locale props

bundleFromPropertiesFile : Locale -> Text -> Effect Text (Result Text Bundle)
bundleFromPropertiesFile = locale path => effect {
  res <- attempt (file.read path)
  res ?
    | Err e => pure (Err e)
    | Ok txt => pure (bundleFromProperties locale txt)
}

keyText : Key -> Text
keyText = k => k.body

messageText : Message -> Text
messageText = m => m.body

tResult : Bundle -> Key -> {} -> Result Text Text
tResult = bundle k args =>
  Map.get (keyText k) bundle.entries ?
    | None => Err (text.concat ["missing key: ", keyText k])
    | Some msg => render msg args

tOpt : Bundle -> Key -> {} -> Option Text
tOpt = bundle k args =>
  (tResult bundle k args) ?
    | Ok txt => Some txt
    | Err _  => None

t : Bundle -> Key -> {} -> Text
t = bundle k args =>
  (tResult bundle k args) ?
    | Ok txt => txt
    | Err _  => keyText k

tWithFallback : List Bundle -> Key -> {} -> Text
tWithFallback = bundles k args => bundles ?
  | [] => keyText k
  | [b, ...rest] =>
    (tOpt b k args) ?
      | Some txt => txt
      | None => tWithFallback rest k args

// Locale tags are normalized by `parseLocale` (e.g. `en_us` -> `en-US`).
// Fallback tags strip subtags from right-to-left:
// `zh-Hant-TW` -> [`zh-Hant-TW`, `zh-Hant`, `zh`].
fallbackTags : Locale -> List Text
fallbackTags = locale => fallbackTagsFromTag (text.trim locale.tag)

fallbackTagsFromTag : Text -> List Text
fallbackTagsFromTag = tag =>
  if text.isEmpty tag then [] else fallbackTagsFromNonEmptyTag tag

fallbackTagsFromNonEmptyTag : Text -> List Text
fallbackTagsFromNonEmptyTag = tag =>
  (text.lastIndexOf tag "-") ?
    | None => [tag]
    | Some i => if i <= 0 then [tag] else [tag, ...fallbackTagsFromNonEmptyTag (text.slice tag 0 i)]

// Build a catalog keyed by normalized locale tag (`Bundle.locale.tag`).
// When duplicates exist, the last bundle wins (right-biased).
catalogFromBundles : List Bundle -> Catalog
catalogFromBundles = bundles =>
  bundles ?
    | [] => Map.empty
    | [b, ...rest] => Map.insert b.locale.tag b (catalogFromBundles rest)

bundleForLocale : Catalog -> Locale -> Option Bundle
bundleForLocale = catalog locale => bundleForTags catalog (fallbackTags locale)

bundleForTags : Catalog -> List Text -> Option Bundle
bundleForTags = catalog tags => tags ?
  | [] => None
  | [t, ...rest] =>
    (Map.get t catalog) ?
      | Some b => Some b
      | None   => bundleForTags catalog rest

bundlesForLocale : Catalog -> Locale -> List Bundle
bundlesForLocale = catalog locale => bundlesForTags catalog (fallbackTags locale)

bundlesForTags : Catalog -> List Text -> List Bundle
bundlesForTags = catalog tags => tags ?
  | [] => []
  | [t, ...rest] =>
    (Map.get t catalog) ?
      | None   => bundlesForTags catalog rest
      | Some b => [b, ...bundlesForTags catalog rest]

tCatalog : Catalog -> Locale -> Key -> {} -> Text
tCatalog = catalog locale k args =>
  tWithFallback (bundlesForLocale catalog locale) k args

tCatalogWithDefault : Catalog -> Locale -> Bundle -> Key -> {} -> Text
tCatalogWithDefault = catalog locale defaultBundle k args =>
  tWithFallback (append1 (bundlesForLocale catalog locale) defaultBundle) k args

append1 : List A -> A -> List A
append1 = xs x => xs ?
  | [] => [x]
  | [h, ...t] => [h, ...append1 t x]
"#;
