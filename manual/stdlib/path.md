# aivi.path

`aivi.path` provides synchronous, pure lexical operations on path text. These functions do not access the filesystem. Use `FsSource` for existence checks and I/O.

The module also exports the nominal `Path` vocabulary type, `PathSource`, and `PathError` with `InvalidPath` and `PathNotFound`. The MVP has no public conversion between `Text` and `Path`; the lexical functions operate on `Text`.

## API

| Export | Type | Behavior |
| --- | --- | --- |
| `parent` | `Text -> Option Text` | Return the containing path, if any |
| `filename` | `Text -> Option Text` | Return the final component |
| `stem` | `Text -> Option Text` | Return the filename without its final extension |
| `extension` | `Text -> Option Text` | Return the final extension without the dot |
| `join` | `Text -> Text -> Text` | Join a base path and segment |
| `isAbsolute` | `Text -> Bool` | Test whether a path is absolute |
| `normalize` | `Text -> Text` | Resolve `.` and `..` lexically |

```aivi
use aivi.path (
    join
    normalize
)

value configPath : Text = join "/etc/demo" "app.conf"
value backupPath : Text = normalize (join "/etc/demo" "../demo/app.conf.bak")
```
