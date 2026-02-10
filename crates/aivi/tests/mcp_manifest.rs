use std::fs;

#[test]
fn mcp_manifest_collects_decorated_bindings_from_parsed_modules() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("mod.aivi");
    fs::write(
        &path,
        r#"@no_prelude
module Example.Mod = {
  @mcp_tool
  search : Int -> Int
  search x = x

  @mcp_resource
  config = "ok"
}
"#,
    )
    .expect("write module");

    let target = path.to_string_lossy().to_string();
    let diagnostics = aivi::load_module_diagnostics(&target).expect("diagnostics");
    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got: {}",
        diagnostics.len()
    );

    let modules = aivi::load_modules(&target).expect("modules");
    let manifest = aivi::collect_mcp_manifest(&modules);
    assert!(manifest.tools.iter().any(|tool| tool.name == "Example.Mod.search"));
    assert!(manifest
        .resources
        .iter()
        .any(|res| res.name == "Example.Mod.config"));
}

#[test]
fn mcp_manifest_generates_input_schema_from_type_sigs() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("schema.aivi");
    fs::write(
        &path,
        r#"@no_prelude
module Example.Schema = {
  @mcp_tool
  search : Int -> List Text -> Effect Http Text
  search n terms = "ok"
}
"#,
    )
    .expect("write module");

    let target = path.to_string_lossy().to_string();
    let modules = aivi::load_modules(&target).expect("modules");
    let manifest = aivi::collect_mcp_manifest(&modules);
    let tool = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "Example.Schema.search")
        .expect("tool exists");

    assert_eq!(tool.input_schema["type"], "object");
    assert_eq!(tool.input_schema["properties"]["n"]["type"], "integer");
    assert_eq!(tool.input_schema["properties"]["terms"]["type"], "array");
    assert_eq!(
        tool.input_schema["properties"]["terms"]["items"]["type"],
        "string"
    );
}

