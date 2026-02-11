pub const MODULE_NAME: &str = "aivi.file";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.file = {
  export FileStats
  export open, readAll, close
  export readText, writeText, exists, stat, delete

  use aivi

  FileStats = { size: Int, created: Int, modified: Int, isFile: Bool, isDirectory: Bool }

  open : Text -> Resource FileHandle
  open path = resource {
    handle <- file.open path
    yield handle
    _ <- file.close handle
  }

  readAll : FileHandle -> Effect Text (Result Text Text)
  readAll handle = attempt (file.readAll handle)

  close : FileHandle -> Effect Text Unit
  close handle = file.close handle

  readText : Text -> Effect Text (Result Text Text)
  readText path = attempt (file.read path)

  writeText : Text -> Text -> Effect Text (Result Text Unit)
  writeText path contents = attempt (file.write_text path contents)

  exists : Text -> Effect Text Bool
  exists path = file.exists path

  stat : Text -> Effect Text (Result Text FileStats)
  stat path = attempt (file.stat path)

  delete : Text -> Effect Text (Result Text Unit)
  delete path = attempt (file.delete path)
}
"#;
