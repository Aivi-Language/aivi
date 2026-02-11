pub const MODULE_NAME: &str = "aivi.console";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.console = {
  export AnsiColor, AnsiStyle
  export log, println, print, error, readLine
  export color, bgColor, style, strip

  use aivi

  type AnsiColor = Black | Red | Green | Yellow | Blue | Magenta | Cyan | White | Default
  type AnsiStyle = {
    fg: Option AnsiColor
    bg: Option AnsiColor
    bold: Bool
    dim: Bool
    italic: Bool
    underline: Bool
    blink: Bool
    inverse: Bool
    hidden: Bool
    strike: Bool
  }

  log : Text -> Effect Text Unit
  log value = console.log value

  println : Text -> Effect Text Unit
  println value = console.println value

  print : Text -> Effect Text Unit
  print value = console.print value

  error : Text -> Effect Text Unit
  error value = console.error value

  readLine : Effect Text (Result Text Text)
  readLine = console.readLine Unit

  color : AnsiColor -> Text -> Text
  color tone value = console.color tone value

  bgColor : AnsiColor -> Text -> Text
  bgColor tone value = console.bgColor tone value

  style : AnsiStyle -> Text -> Text
  style attrs value = console.style attrs value

  strip : Text -> Text
  strip value = console.strip value
}
"#;
