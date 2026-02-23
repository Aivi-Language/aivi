# REST / HTTP Sources

<!-- quick-info: {"kind":"topic","name":"rest http sources"} -->

AIVI supports both low-level HTTP (`http`/`https`) and a REST-oriented facade (`rest`) as typed `Source` boundaries.

<!-- /quick-info -->

## APIs (v0.1)

- `http.get/post/fetch`
- `https.get/post/fetch`
- `rest.get/post/fetch`

`rest.fetch` supports request-level options (timeouts, retries, bearer auth, strict status handling).

## Example

```aivi
Request = {
  method: Text
  url: Url
  headers: List { name: Text, value: Text }
  body: Option Text
  timeoutMs: Option Int
  retryCount: Option Int
  bearerToken: Option Text
  strictStatus: Option Bool
}

User = { name: Text, age: Int, gender: Text }

usersSource : Source RestApi (List User)
usersSource = rest.get ~u(https://api.example.com/users)

do Effect {
  users <- load usersSource
  pure users
}
```
