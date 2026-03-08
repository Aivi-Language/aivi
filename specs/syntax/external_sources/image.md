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

<<< ../../snippets/from_md/syntax/external_sources/image/block_01.aivi{aivi}


## Typical uses

- validating user-uploaded images,
- reading dimensions before layout or resizing,
- generating thumbnails,
- collecting metadata for asset catalogs.
