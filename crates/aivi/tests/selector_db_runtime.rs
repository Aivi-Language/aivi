mod native_fixture;

use native_fixture::run_jit_err;

#[test]
fn selector_update_rejects_non_lowered_patch_functions_on_db_backed_path() {
    let err = run_jit_err(
        "selector-db-runtime",
        r#"@no_prelude
module app.main

use aivi
use aivi.database

main : Effect Text Unit
main = do Effect {
  conn <- connect { driver: Sqlite, url: ":memory:" }
  users0 = table "selector_update_patch_users"[]
  _ <- runMigrationsOn conn [users0]
  users1 <- insertOn conn users0 { id: 1, name: "Alice" }
  _ <- updateOn conn users1[id == 1] (u => u)
  pure Unit
}
"#,
    );
    let rendered = err.render(false);
    assert!(
        rendered
            .contains("database.updateOn requires a patch block in the lowered SQL-backed subset"),
        "unexpected runtime error:\n{rendered}"
    );
}
