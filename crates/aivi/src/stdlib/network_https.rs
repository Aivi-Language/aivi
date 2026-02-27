pub const MODULE_NAME: &str = "aivi.net.https";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.net.https
export Header, Body, Request, Response, Error
export get, post, fetch

use aivi
use aivi.url (Url)

Header = { name: Text, value: Text }
Body = Plain Text | Form (List Header)
Request = { method: Text, url: Url, headers: List Header, body: Option Body }
Response = { status: Int, headers: List Header, body: Text }
Error = { message: Text }

get : Url -> Effect Text (Result Error Response)
get = url => load (https.get url)

post : Url -> Text -> Effect Text (Result Error Response)
post = url body => load (https.post url body)

fetch : Request -> Effect Text (Result Error Response)
fetch = request => load (https.fetch request)
"#;
