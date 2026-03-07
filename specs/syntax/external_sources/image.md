# Image Sources

<!-- quick-info: {"kind":"topic","name":"image sources"} -->
Image sources expose typed metadata and decoded pixel data for boundary-safe processing.
<!-- /quick-info -->

Image sources let you read image files without treating them as unstructured blobs.

In practice, there are two common needs:

- inspect metadata such as dimensions or format,
- load the image payload for further processing.

## APIs

- `file.imageMeta : Text -> Source Image A`
- `file.image : Text -> Source Image A`

## Choosing between metadata and full image reads

- use `file.imageMeta` when you only need descriptive information such as width, height, or format
- use `file.image` when you need the decoded image itself

Reading only metadata can be cheaper and simpler when you are validating uploads, building previews, or indexing assets.

## Example

```aivi
ImageMeta = { width: Int, height: Int, format: Text }

do Effect {
  meta <- load (file.imageMeta "./photo.jpg")  -- inspect the file without loading the full image payload
  img  <- load (file.image "./photo.jpg")      -- decode the image for further work
  pure { meta, img }
}
```

## Typical uses

- validating user-uploaded images,
- reading dimensions before layout or resizing,
- generating thumbnails,
- collecting metadata for asset catalogs.
