# aivi.gtk.styles

Typed constants for GTK4 and libadwaita CSS utility class names, plus a `classes` helper for combining them.

Use these with the `cssClasses` attribute on any widget instead of writing raw strings. They are the exact class names that GTK4 and libadwaita define; AIVI passes them directly to the widget's CSS engine.

## Import

```aivi
use aivi.gtk.styles (
    suggestedAction
    title1
    dimLabel
    classes
)
```

## Overview

| Name | Type | Description |
|------|------|-------------|
| `title1` | `Text` | `"title-1"` — display-size heading |
| `title2` | `Text` | `"title-2"` — large section heading |
| `title3` | `Text` | `"title-3"` — medium section heading |
| `title4` | `Text` | `"title-4"` — small/bold subheading |
| `heading` | `Text` | `"heading"` — standard heading weight |
| `caption` | `Text` | `"caption"` — small secondary text |
| `captionHeading` | `Text` | `"caption-heading"` — bold caption scale |
| `numeric` | `Text` | `"numeric"` — uniform-width digit rendering |
| `suggestedAction` | `Text` | `"suggested-action"` — primary CTA button (accent colour) |
| `destructiveAction` | `Text` | `"destructive-action"` — danger/irreversible action |
| `flatStyle` | `Text` | `"flat"` — no background or frame |
| `circular` | `Text` | `"circular"` — equal width and height, circle shape |
| `pill` | `Text` | `"pill"` — fully rounded ends |
| `raised` | `Text` | `"raised"` — slight surface elevation |
| `linked` | `Text` | `"linked"` — removes gap between adjacent buttons |
| `osd` | `Text` | `"osd"` — dark semi-transparent on-screen-display style |
| `success` | `Text` | `"success"` — green confirmation tint |
| `warning` | `Text` | `"warning"` — yellow warning tint |
| `errorStyle` | `Text` | `"error"` — red error/destructive tint |
| `accent` | `Text` | `"accent"` — follows system or app accent hue |
| `dimLabel` | `Text` | `"dim-label"` — reduced-opacity secondary text |
| `card` | `Text` | `"card"` — rounded card surface |
| `boxedList` | `Text` | `"boxed-list"` — bordered, rounded ListBox rows |
| `toolbar` | `Text` | `"toolbar"` — toolbar surface |
| `frameStyle` | `Text` | `"frame"` — thin container border |
| `backgroundStyle` | `Text` | `"background"` — app background layer |
| `viewStyle` | `Text` | `"view"` — primary reading/editing surface |
| `undershoot` | `Text` | `"undershoot"` — scrolled-content overlay hint |
| `classes` | `List Text -> Text` | Combines names into a space-separated string |

## Basic usage

```aivi
use aivi.gtk.styles (
    suggestedAction
    destructiveAction
    title1
    dimLabel
)

value view =
    <Window title="Demo">
        <Box orientation="Vertical" spacing={12} marginTop={24} marginStart={24} marginEnd={24}>
            <Label text="Welcome" cssClasses={title1} />
            <Label text="Secondary info" cssClasses={dimLabel} />
            <Box orientation="Horizontal" spacing={8}>
                <Button label="Save" cssClasses={suggestedAction} />
                <Button label="Delete" cssClasses={destructiveAction} />
            </Box>
        </Box>
    </Window>
```

## Combining multiple classes

Use `classes` to join several class names into a single space-separated value:

```aivi
use aivi.gtk.styles (
    circular
    flatStyle
    osd
    classes
)

value iconButtonClasses : Text =
    classes [
        circular,
        flatStyle
    ]

value view = <Button iconName="list-add-symbolic" cssClasses={iconButtonClasses} />
```

## Boxed preference list

```aivi
use aivi.gtk.styles (boxedList)

value view =
    <Window title="Settings">
        <Box orientation="Vertical" marginTop={24} marginStart={24} marginEnd={24}>
            <ListBox selectionMode="None" cssClasses={boxedList}>
                <SwitchRow title="Enable notifications" active={True} />
                <SwitchRow title="Dark mode" active={False} />
            </ListBox>
        </Box>
    </Window>
```

## Card surface

```aivi
use aivi.gtk.styles (card)

value view =
    <Window title="Cards">
        <Box orientation="Vertical" spacing={12} marginTop={16} marginStart={16} marginEnd={16}>
            <Box orientation="Vertical" cssClasses={card} marginStart={8} marginEnd={8} marginTop={8} marginBottom={8}>
                <Label text="Card title" cssClasses="heading" />
                <Label text="Card body text goes here." />
            </Box>
        </Box>
    </Window>
```

## Notes

- Every constant is a plain `Text` value — you can use them anywhere a `Text` expression is expected.
- `cssClasses` on any widget accepts a whitespace-separated string; `classes` produces exactly that.
- These are the real GTK4/libadwaita CSS class names; refer to the [Adwaita stylesheet documentation](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/1-latest/style-classes.html) for the full catalogue and visual reference.
