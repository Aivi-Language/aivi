# Image Sources

<!-- quick-info: {"kind":"topic","name":"image sources"} -->
Image sources expose typed metadata and decoded pixel data for boundary-safe processing.
<!-- /quick-info -->

## APIs (v0.1)

- `file.imageMeta : Text -> Source Image A`
- `file.image : Text -> Source Image A`

## Example

```aivi
ImageMeta = { width: Int, height: Int, format: Text }

do Effect {
  meta <- load (file.imageMeta "./photo.jpg")
  img  <- load (file.image "./photo.jpg")
  pure { meta, img }
}
```
