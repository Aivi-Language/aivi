# Async Data

Fetching data from an API is one of the first things most apps need to do.
In AIVI, an HTTP response is just another event that drives a signal.
There is no `async`/`await`, no `.then()`, no `Promise`.

## The pattern

```
@source http.get → Signal (Result Data) → ||> Ok/Err → markup
```

1. Declare a signal with `@source http.get`.
2. The signal holds `Result Data` — either `Ok` the parsed response or `Err` a message.
3. Use `\|\|>` or `T\|>`/`F\|>` to branch on the result.
4. Bind the branched signals to markup.

## A complete example

Fetching a user profile:

```aivi
type User = {
    id:       Int,
    name:     Text,
    email:    Text,
    bio:      Text
}

type LoadState A =
  | Loading
  | Loaded A
  | Failed Text

@source http.get "https://api.example.com/user/1"
sig userResponse : Signal (Result User)

sig userState : Signal (LoadState User) =
    userResponse
     ||> Ok user  => Loaded user
     ||> Err msg  => Failed msg

sig userName : Signal Text =
    userState
     ||> Loaded user => user.name
     ||> Loading     => "Loading…"
     ||> Failed _    => "Unknown"

sig userBio : Signal Text =
    userState
     ||> Loaded user => user.bio
     ||> Loading     => ""
     ||> Failed msg  => "Error: {msg}"

val main =
    <Window title="User Profile">
        <Box orientation={Vertical} spacing={12}>
            <Label text={userName} />
            <Label text={userBio} />
        </Box>
    </Window>

export main
```

## Handling the loading state

The above example maps `Loading` to a placeholder string. For a proper loading spinner:

```aivi
sig isLoading : Signal Bool =
    userState
     ||> Loading => True
     ||> _       => False

val main =
    <Window title="Profile">
        <Box orientation={Vertical} spacing={12}>
            <show when={isLoading}>
                <Spinner active={True} />
            </show>
            <Label text={userName} />
        </Box>
    </Window>
```

## Retrying on error

```aivi
@source button.clicked "retry"
sig retryClicked : Signal Unit

@source http.get "https://api.example.com/data" with {
    retry: retryClicked
}
sig data : Signal (Result Payload)
```

Passing `retry: retryClicked` tells the source to re-fetch when `retryClicked` fires.

## Chaining requests

When a second request depends on the result of a first, use `?\|>` to gate the second source:

```aivi
@source http.get "https://api.example.com/user/1"
sig userResult : Signal (Result User)

sig userId : Signal Int =
    userResult
     ?|> \r => r == Ok _
     ||> Ok user => user.id

@source http.get "https://api.example.com/posts" with {
    params: userId
}
sig postsResult : Signal (Result (List Post))
```

`userId` only has a value when the user loaded successfully.
The posts request does not fire until `userId` is available.

## Why this is better than callbacks

In callback-based code, each step nests inside the previous one:

```
// typical callback hell (pseudo-code)
fetchUser(id, (err, user) => {
  if (err) { showError(err); return }
  fetchPosts(user.id, (err, posts) => {
    if (err) { showError(err); return }
    renderPosts(posts)
  })
})
```

In AIVI, the dependency is declared, not nested:

```aivi
sig userId : Signal Int = userResult ||> Ok u => u.id
sig posts  : Signal (Result (List Post)) = ... using userId
sig view   : Signal Markup = posts ||> Ok ps => renderPosts ps
```

Each step is a separate named signal. No nesting, no error routing, no lifecycle cleanup.

## Summary

- `@source http.get "url"` produces a `Signal (Result T)`.
- Map the result through `\|\|>` arms for `Ok` and `Err`.
- Use `LoadState A` or similar to represent `Loading` / `Loaded` / `Failed`.
- Gate dependent requests with `?\|>`.
- Retry by passing a click signal to the source.
