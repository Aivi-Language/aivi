# How to fetch HTTP data

Use an `http.get` source when you want external data to enter your app through a typed boundary and
flow into ordinary signals.

## Example

```aivi
type User = {
    id: Int,
    name: Text
}

signal refreshClick : Signal Unit

@source http.get "https://api.example.com/users" with {
    refreshOn: refreshClick
}
signal usersResult : Signal (Result HttpError (List User))

signal users = usersResult
 ||> Ok loaded -> loaded
 ||> Err _     -> []

signal headline = users
  |> length
  |> "Users: {.}"

value main =
    <Window title="Users">
        <Box orientation="vertical" spacing={12} marginTop={16} marginBottom={16} marginStart={16} marginEnd={16}>
            <Button label="Refresh" onClick={refreshClick} />
            <Label text={headline} />
            <Box orientation="vertical" spacing={6}>
                <each of={users} as={user} key={user.id}>
                    <Label text={user.name} />
                </each>
            </Box>
        </Box>
    </Window>

export main
```

## Why this shape works

1. `@source http.get ...` declares the outside-world boundary.
2. `usersResult` keeps success or failure explicit in the type.
3. `users` turns the successful branch into plain data for ordinary UI work.
4. The UI stays declarative: it reacts to the signal graph instead of running a callback chain.

## Common variations

- Need loading and failure UI as first-class states? Use an explicit sum type from
  [How to model loading and error states](/how-to/loading-and-error-states).
- Need a typed client from an OpenAPI spec? Use [OpenAPI source guide](/guide/openapi-source).
- Need periodic refresh? Add a timer signal and feed it into `refreshOn`.
