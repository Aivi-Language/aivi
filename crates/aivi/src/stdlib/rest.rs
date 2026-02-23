pub const MODULE_NAME: &str = "aivi.rest";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.rest
export Header, Request
export get, post, fetch

use aivi
use aivi.url (Url)

Header = { name: Text, value: Text }
Request = {
  method: Text
  url: Url
  headers: List Header
  body: Option Text
  timeoutMs: Option Int
  retryCount: Option Int
  bearerToken: Option Text
  strictStatus: Option Bool
}

get : Url -> Effect Text A
get = url => load (rest.get url)

post : Url -> Text -> Effect Text A
post = url body => load (rest.post url body)

fetch : Request -> Effect Text A
fetch = request => load (rest.fetch request)
"#;
