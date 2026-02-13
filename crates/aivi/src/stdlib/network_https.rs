pub const MODULE_NAME: &str = "aivi.net.https";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.net.https
export Header, Request, Response, Error
export get, post, fetch

use aivi
use aivi.url (Url)

Header = { name: Text, value: Text }
Request = { method: Text, url: Url, headers: List Header, body: Option Text }
Response = { status: Int, headers: List Header, body: Text }
Error = { message: Text }

get : Url -> Effect Text (Result Error Response)
get url = https.get url

post : Url -> Text -> Effect Text (Result Error Response)
post url body = https.post url body

fetch : Request -> Effect Text (Result Error Response)
fetch request = https.fetch request"#;
