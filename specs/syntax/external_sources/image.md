# Image Sources

<!-- quick-info: {"kind":"topic","name":"image sources"} -->
Image sources expose typed metadata and decoded pixel data for boundary-safe processing.
<!-- /quick-info -->

Image sources let you read image files without treating them as unstructured blobs.
They are a good fit when you want to validate uploads, inspect dimensions before layout, or hand pixel data to an image-processing step while keeping failure handling at the source boundary.

In practice, there are two common needs:

- inspect metadata such as dimensions or format,
- load the image payload for further processing.

## APIs

- `file.imageMeta : Text -> Source Image A`
- `file.image : Text -> Source Image A`

Both constructors are schema-driven: the surrounding binding or function signature determines `A`.
In practice, `file.imageMeta` and `file.image` usually target different record shapes even though they share the same `Source Image A` form.

Both APIs take a filesystem path, but the source kind is `Image`, so `load` reports `SourceError Image` rather than `SourceError File`.
If you want the path-based `Effect` wrappers instead of `Source` values, see [File Domain](../../stdlib/system/file.md).

## Choosing between metadata and full image reads

- use `file.imageMeta` when you only need descriptive information such as width, height, or format
- use `file.image` when you need the decoded image itself

Reading only metadata can be cheaper and simpler when you are validating uploads, building previews, or indexing assets.
`file.image` is the heavier option in the current v0.1 runtime because it eagerly decodes the whole file into row-major RGB pixels.

If you care about the original file/container format, keep the metadata read.
In the current v0.1 runtime, `file.image` decodes pixels into RGB rows for processing, so its `format` field describes the decoded pixel layout rather than the original on-disk container.
That conversion also drops alpha information today, because the runtime normalizes successful full-image loads to RGB tuples.

## Current decoded shape in v0.1

`file.imageMeta` is typically decoded into a small record such as:

```aivi
{ width: Int, height: Int, format: Text }
```

The `format` field here reflects the file/container format guessed from the image bytes, typically values such as `"Png"` or `"Jpeg"`.

`file.image` currently produces a record with width, height, a decoded format marker, and row-major RGB pixels:

```aivi
{
  width: Int,
  height: Int,
  format: Text,
  pixels: List (List (Int, Int, Int))
}
```

For successful full-image reads in v0.1, `format` is currently `"Rgb8"`.
That shape is useful for thumbnailing, analysis, and other pixel-oriented work when RGB pixels are enough.

## Capability mapping

Loading either image source requires `file.read`.
See [Capabilities](../capabilities.md) for the capability vocabulary and [File Sources](file.md) for the wider file-source family.

## Example

<<< ../../snippets/from_md/syntax/external_sources/image/block_01.aivi{aivi}

This minimal example shows both APIs side by side.
In real programs, the `meta` binding is often enough for validation or layout decisions, and many workflows only call `file.image` later when they truly need `img.pixels`.

## Failure modes

Image sources can currently fail in slightly different ways depending on which constructor you use:

- **transport / I/O failure** for both constructors: the path does not exist, cannot be opened, or cannot be read.
- **metadata decode failure** for `file.imageMeta`: the runtime cannot identify an image format or cannot read dimensions from the bytes.
- **full-image safety failure** for `file.image`: images above `16,000,000` pixels are rejected instead of being decoded eagerly.

One implementation detail matters when you read full images today: `file.image` performs opening and decoding in one runtime step, so unsupported or corrupt files may currently surface as transport-style errors instead of decode-style errors.
If you need a clearer distinction between “not readable” and “not a valid image,” read `file.imageMeta` first and only decode pixels after that succeeds.

Typical decode diagnostics point at the image source kind and the part that failed. For example:

```text
failed to parse source [Image] at $.pixels
expected at most 16,000,000 pixels but received 24576000
image is too large to decode safely
```

For retry, caching, or provenance policies around image loads, see [Source Composition](composition.md).

## Typical uses

- validating user-uploaded images,
- reading dimensions before layout or resizing,
- generating thumbnails,
- collecting metadata for asset catalogs.
