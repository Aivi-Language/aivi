pub const MODULE_NAME: &str = "aivi.net.http";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.net.http
export Header, Body, Request, Response, Error
export get, post, fetch

use aivi
use aivi.url (Url)
use aivi.json (JsonValue)

Header = { name: Text, value: Text }
Body = Plain Text | Form (List Header) | Json JsonValue
Request = { method: Text, url: Url, headers: List Header, body: Option Body }
Response = { status: Int, headers: List Header, body: Text }
Error = { message: Text }

get : Url -> Effect Text (Result Error Response)
get = url => load (http.get url)

post : Url -> Text -> Effect Text (Result Error Response)
post = url body => load (http.post url body)

fetch : Request -> Effect Text (Result Error Response)
fetch = request => load (http.fetch request)
"#;
