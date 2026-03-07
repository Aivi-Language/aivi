use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use aivi::{infer_value_types, parse_modules, Module, ModuleItem};
use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position, Url};

use crate::backend::Backend;
use crate::doc_index::DocIndex;
use crate::state::IndexedModule;

use super::resolve_import_name;

impl Backend {
    fn hover_debug_enabled() -> bool {
        std::env::var_os("AIVI_LSP_DEBUG_HOVER").is_some()
    }

    fn hover_debug(message: impl std::fmt::Display) {
        if Self::hover_debug_enabled() {
            eprintln!("[aivi-lsp:hover] {message}");
        }
    }

    fn hover_markdown(value: String) -> Hover {
        Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: None,
        }
    }

    fn append_markdown_section(contents: &mut String, extra: &str) {
        if extra.is_empty() || contents.contains(extra) {
            return;
        }
        contents.push_str("\n\n---\n\n");
        contents.push_str(extra);
    }

    fn append_gtk_architecture_docs(contents: &mut String, ident: &str) {
        if let Some(extra) = Self::gtk_architecture_doc_for_ident(ident) {
            Self::append_markdown_section(contents, &extra);
        }
    }

    fn append_source_tooling_docs(contents: &mut String, ident: &str) {
        if let Some(extra) = Self::source_tooling_doc_for_ident(ident) {
            Self::append_markdown_section(contents, &extra);
        }
    }

    fn gtk_architecture_doc_for_ident(ident: &str) -> Option<String> {
        let (badge, body) = match ident {
            "gtkApp" => (
                "function",
                "`gtkApp`\n\nHosts the blessed GTK `Model → View → Msg → Update` architecture.\n\nUse `toMsg: auto` for common constructor-style signal bindings, keep explicit `toMsg` for richer routing, and return the next `{ model, commands }` step from `update`.",
            ),
            "AppStep" => (
                "type",
                "`AppStep model msg = { model: model, commands: List (Command msg) }`\n\nThe committed model renders first; commands run afterwards. You can return this record directly, or use `appStep` / `appStepWith` as shorthand.",
            ),
            "auto" => (
                "function",
                "`auto : GtkSignalEvent -> Option msg`\n\nAutomatic `gtkApp` signal routing for common constructor bindings such as `onInput={ NameChanged }` and `onClick={ Save }`.\n\n`auto` works best when each signal is unique in the current view or attached to a widget with an `id=\"...\"` name.",
            ),
            "appStep" => (
                "function",
                "`appStep : s -> AppStep s msg`\n\nOptional shorthand for `{ model, commands: [] }`.",
            ),
            "appStepWith" => (
                "function",
                "`appStepWith : s -> List (Command msg) -> AppStep s msg`\n\nOptional shorthand for returning the next model together with post-update commands such as timers or one-shot effects.",
            ),
            "noSubscriptions" => (
                "function",
                "`noSubscriptions : s -> List (Subscription msg)`\n\nUse this as the default `subscriptions` field when the current model does not need timers or external event feeds.",
            ),
            "commandAfter" => (
                "function",
                "`commandAfter : { key: CommandKey, millis: Int, msg: msg } -> Command msg`\n\nSchedule a one-shot delayed message after the current update commits. Prefer this for transient timers; use `subscriptionEvery` for repeating ticks.",
            ),
            "commandPerform" => (
                "function",
                "`commandPerform : { run: Effect GtkError msg, onError: Option (GtkError -> msg) } -> Command msg`\n\nRun a one-shot effect after `update` and map its result back into a `Msg` without leaving the blessed `gtkApp` loop.",
            ),
            "commandEmit" => (
                "function",
                "`commandEmit : msg -> Command msg`\n\nQueue a synthetic follow-up message after the current `update` commits.",
            ),
            "subscriptionEvery" => (
                "function",
                "`subscriptionEvery : { key: SubscriptionKey, millis: Int, tag: msg } -> Subscription msg`\n\nDescribe a repeating timer derived from the current model. `gtkApp` diffs subscriptions by key on every committed update.",
            ),
            "subscriptionSource" => (
                "function",
                "`subscriptionSource`\n\nBridge a long-lived external feed into the `gtkApp` loop. Use this for extra GTK receivers, watches, or background channels that should emit `Msg` values over time.",
            ),
            "GtkSignalEvent" => (
                "type",
                "`GtkSignalEvent`\n\nTyped event stream for the primary GTK signal feed hosted by `gtkApp`.\n\nMatch constructors in `toMsg` by widget `id=\"...\"` name strings rather than raw `WidgetId` values when possible.",
            ),
            "GtkClicked" => (
                "constructor",
                "`GtkClicked WidgetId Text`\n\nRaised for button-like click signals. The second field is the widget's `id=\"...\"` name, which is the preferred key to match in `toMsg`.",
            ),
            "GtkInputChanged" => (
                "constructor",
                "`GtkInputChanged WidgetId Text Text`\n\nRaised when editable text changes. In the blessed forms flow, map this to a domain message and call `setValue` in `update`.",
            ),
            "GtkFocusOut" => (
                "constructor",
                "`GtkFocusOut WidgetId Text`\n\nRaised when a widget loses focus. Use this with `touch` so `visibleErrors` can reveal validation messages after blur.",
            ),
            "GtkFocusIn" => (
                "constructor",
                "`GtkFocusIn WidgetId Text`\n\nRaised when a widget gains focus. Use it when the model needs explicit focus tracking.",
            ),
            "GtkActivated" => (
                "constructor",
                "`GtkActivated WidgetId Text`\n\nRaised for activation-style signals such as entry submit or row activation.",
            ),
            "GtkToggled" => (
                "constructor",
                "`GtkToggled WidgetId Text Bool`\n\nRaised for boolean toggle state changes. The final field carries the new checked state.",
            ),
            "GtkValueChanged" => (
                "constructor",
                "`GtkValueChanged WidgetId Text Float`\n\nRaised for slider or spin-button style value changes. Use it for model updates that track numeric input continuously.",
            ),
            "GtkUnknownSignal" => (
                "constructor",
                "`GtkUnknownSignal WidgetId Text Text Text Text`\n\nFallback for lower-level or not-yet-specialized signals. Prefer the typed constructors when available.",
            ),
            "Field" => (
                "type",
                "`Field A = { value: A, touched: Bool, dirty: Bool }`\n\nLightweight GTK form state. Keep `Field` values in the `gtkApp` model so validation, rendering, and commands stay in the same loop.",
            ),
            "field" => (
                "function",
                "`field : A -> Field A`\n\nCreate the initial form field state for a model.",
            ),
            "setValue" => (
                "function",
                "`setValue : A -> Field A -> Field A`\n\nUpdate a field from `GtkInputChanged` messages while marking it dirty.",
            ),
            "touch" => (
                "function",
                "`touch : Field A -> Field A`\n\nMark a field as touched after blur, usually from a `GtkFocusOut`-driven message.",
            ),
            "visibleErrors" => (
                "function",
                "`visibleErrors : Bool -> (A -> Validation (List E) B) -> Field A -> List E`\n\nShow validation errors only after submit or blur. Pair this with a `submitted` flag in the `gtkApp` model and `touch` in `update`.",
            ),
            _ => return None,
        };

        Some(Self::hover_badge_markdown(badge, body.to_string()))
    }

    fn source_tooling_doc_for_ident(ident: &str) -> Option<String> {
        let (badge, body) = match ident {
            "load" => (
                "source-pipeline",
                "`load`\n\nExecutes a `Source K A` in canonical source-pipeline order: acquire connector data, decode with the source schema, run pure `source.transform` stages, then run `source.validate` stages before producing `A` or `SourceError K`.",
            ),
            "SourceError" => (
                "source-error",
                "`SourceError K = IOError Text | DecodeError (List DecodeError)`\n\nStructured source failures distinguish connector/transport errors from schema and validation failures. `source.validate` feeds semantic failures into the same `DecodeError` bucket so callers can inspect one error surface.",
            ),
            "file.json" => (
                "source-constructor",
                "`file.json`\n\nSchema-first JSON source constructor.\n\nPrefer the record form:\n```aivi\nfile.json {\n  path: \"./users.json\"\n  schema: source.schema.derive\n}\n```\nThe compatibility string form still works, but the record form gives hover and diagnostics a stable schema contract to describe before `load` runs.",
            ),
            "env.decode" => (
                "source-constructor",
                "`env.decode`\n\nSchema-first environment decoder.\n\nPrefer the record form:\n```aivi\nenv.decode {\n  prefix: \"AIVI_APP\"\n  schema: source.schema.derive\n}\n```\nUse this when the surrounding result type should drive record decoding and source-schema hover.",
            ),
            "source.transform" => (
                "source-stage",
                "`source.transform`\n\nPure, total normalization after decode.\n\nUse this for reshaping, sorting, filtering, or projecting decoded values. If a stage can reject data, prefer `source.validate` so failures remain structured `DecodeError` values.",
            ),
            "source.validate" => (
                "source-stage",
                "`source.validate`\n\nSemantic validation stage for schema-decoded values.\n\n`source.validate` expects `Validation (List DecodeError) B` and folds `Invalid` results into `SourceError.DecodeError`. Validation failures are not retried as transport failures.",
            ),
            "source.decodeErrors" => (
                "source-stage",
                "`source.decodeErrors`\n\nExtract the accumulated `List DecodeError` from a `SourceError K`.\n\n`IOError` values produce `[]`, so this is the helper to use when UI or tests want to render only schema/validation mismatches.",
            ),
            "source.schema.derive" => (
                "source-schema",
                "`source.schema.derive`\n\nDerive the external schema contract from the declared result type of the source.\n\nTop-level schema-first declarations should carry an explicit `Source ...` type signature so tooling can describe that schema before any later `load` call.",
            ),
            _ => return None,
        };

        Some(Self::hover_badge_markdown(badge, body.to_string()))
    }

    fn gtk_app_field_doc(field_name: &str) -> Option<String> {
        let body = match field_name {
            "id" => "`id` : `Text`\n\nApplication identifier passed to GTK during startup.",
            "title" => {
                "`title` : `Text`\n\nPrimary window title shown by the blessed `gtkApp` host."
            }
            "size" => "`size` : `(Int, Int)`\n\nInitial window width and height.",
            "model" => {
                "`model` : `s`\n\nInitial application state. Keep form fields, submit flags, and subscription-driving state here."
            }
            "onStart" => {
                "`onStart` : `AppId -> WindowId -> Effect GtkError Unit`\n\nOne-time startup hook that runs before the initial render."
            }
            "subscriptions" => {
                "`subscriptions` : `s -> List (Subscription msg)`\n\nDerive long-lived timers or external feeds from the current model. Use `noSubscriptions` when the app has nothing to listen to."
            }
            "view" => {
                "`view` : `s -> GtkNode`\n\nPure projection from the current model into the GTK node tree. Keep rendering here and leave effects to commands/subscriptions."
            }
            "toMsg" => {
                "`toMsg` : `GtkSignalEvent -> Option msg`\n\nTranslate the primary GTK signal stream into domain messages. Start with `auto` for common constructor bindings; match by widget `id=\"...\"` when you need explicit routing, feed `GtkInputChanged` into `setValue`, and use `GtkFocusOut` to drive `touch`."
            }
            "update" => {
                "`update` : `msg -> s -> Effect GtkError (AppStep s msg)`\n\nCommit the next model and any post-update commands. Return the `{ model, commands }` record directly, or use `appStep` / `appStepWith` as shorthand."
            }
            _ => return None,
        };

        Some(Self::hover_badge_markdown(
            "gtk-app-field",
            body.to_string(),
        ))
    }

    fn direct_record_field_name_at_position(
        expr: &aivi::Expr,
        position: Position,
    ) -> Option<&aivi::SpannedName> {
        let fields = match expr {
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => fields,
            _ => return None,
        };

        for field in fields {
            for segment in &field.path {
                if let aivi::PathSegment::Field(name) = segment {
                    let range = Self::span_to_range(name.span.clone());
                    if Self::range_contains_position(&range, position) {
                        return Some(name);
                    }
                }
            }
        }
        None
    }

    fn find_gtk_app_record_at_position_in_expr(
        expr: &aivi::Expr,
        position: Position,
    ) -> Option<&aivi::Expr> {
        if !Self::expr_contains_position_for_hover(expr, position) {
            return None;
        }

        use aivi::Expr;

        match expr {
            Expr::Call { func, args, .. } => {
                if let Expr::Ident(name) = func.as_ref() {
                    if name.name == "gtkApp" {
                        for arg in args {
                            if Self::expr_contains_position_for_hover(arg, position)
                                && matches!(arg, Expr::Record { .. } | Expr::PatchLit { .. })
                            {
                                return Some(arg);
                            }
                        }
                    }
                }

                Self::find_gtk_app_record_at_position_in_expr(func, position).or_else(|| {
                    args.iter().find_map(|arg| {
                        Self::find_gtk_app_record_at_position_in_expr(arg, position)
                    })
                })
            }
            Expr::Suffixed { base, .. } | Expr::UnaryNeg { expr: base, .. } => {
                Self::find_gtk_app_record_at_position_in_expr(base, position)
            }
            Expr::Mock {
                substitutions,
                body,
                ..
            } => substitutions
                .iter()
                .find_map(|sub| {
                    sub.value.as_ref().and_then(|value| {
                        Self::find_gtk_app_record_at_position_in_expr(value, position)
                    })
                })
                .or_else(|| Self::find_gtk_app_record_at_position_in_expr(body, position)),
            Expr::TextInterpolate { parts, .. } => parts.iter().find_map(|part| match part {
                aivi::TextPart::Text { .. } => None,
                aivi::TextPart::Expr { expr, .. } => {
                    Self::find_gtk_app_record_at_position_in_expr(expr, position)
                }
            }),
            Expr::List { items, .. } => items.iter().find_map(|item| {
                Self::find_gtk_app_record_at_position_in_expr(&item.expr, position)
            }),
            Expr::Tuple { items, .. } => items
                .iter()
                .find_map(|item| Self::find_gtk_app_record_at_position_in_expr(item, position)),
            Expr::Index { base, index, .. } => {
                Self::find_gtk_app_record_at_position_in_expr(base, position)
                    .or_else(|| Self::find_gtk_app_record_at_position_in_expr(index, position))
            }
            Expr::Lambda { body, .. } | Expr::CapabilityScope { body, .. } => {
                Self::find_gtk_app_record_at_position_in_expr(body, position)
            }
            Expr::Match {
                scrutinee, arms, ..
            } => scrutinee
                .as_ref()
                .and_then(|value| Self::find_gtk_app_record_at_position_in_expr(value, position))
                .or_else(|| {
                    arms.iter().find_map(|arm| {
                        Self::find_gtk_app_record_at_position_in_expr(&arm.body, position).or_else(
                            || {
                                arm.guard.as_ref().and_then(|guard| {
                                    Self::find_gtk_app_record_at_position_in_expr(guard, position)
                                })
                            },
                        )
                    })
                }),
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => Self::find_gtk_app_record_at_position_in_expr(cond, position)
                .or_else(|| Self::find_gtk_app_record_at_position_in_expr(then_branch, position))
                .or_else(|| Self::find_gtk_app_record_at_position_in_expr(else_branch, position)),
            Expr::Binary { left, right, .. } => {
                Self::find_gtk_app_record_at_position_in_expr(left, position)
                    .or_else(|| Self::find_gtk_app_record_at_position_in_expr(right, position))
            }
            Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
                fields.iter().find_map(|field| {
                    Self::find_gtk_app_record_at_position_in_expr(&field.value, position)
                })
            }
            Expr::Block { items, .. } => items.iter().find_map(|item| match item {
                aivi::BlockItem::Bind { expr, .. }
                | aivi::BlockItem::Let { expr, .. }
                | aivi::BlockItem::Filter { expr, .. }
                | aivi::BlockItem::Yield { expr, .. }
                | aivi::BlockItem::Recurse { expr, .. }
                | aivi::BlockItem::Expr { expr, .. } => {
                    Self::find_gtk_app_record_at_position_in_expr(expr, position)
                }
                aivi::BlockItem::When { cond, effect, .. }
                | aivi::BlockItem::Unless { cond, effect, .. } => {
                    Self::find_gtk_app_record_at_position_in_expr(cond, position)
                        .or_else(|| Self::find_gtk_app_record_at_position_in_expr(effect, position))
                }
                aivi::BlockItem::Given {
                    cond, fail_expr, ..
                } => Self::find_gtk_app_record_at_position_in_expr(cond, position)
                    .or_else(|| Self::find_gtk_app_record_at_position_in_expr(fail_expr, position)),
                aivi::BlockItem::On {
                    transition,
                    handler,
                    ..
                } => Self::find_gtk_app_record_at_position_in_expr(transition, position)
                    .or_else(|| Self::find_gtk_app_record_at_position_in_expr(handler, position)),
            }),
            Expr::Ident(_) | Expr::Literal(_) | Expr::Raw { .. } | Expr::FieldSection { .. } => {
                None
            }
            Expr::FieldAccess { base, .. } => {
                Self::find_gtk_app_record_at_position_in_expr(base, position)
            }
        }
    }

    fn hover_for_gtk_app_field(modules: &[Module], position: Position) -> Option<String> {
        let module = Self::module_at_position(modules, position)?;

        let scan_expr = |expr: &aivi::Expr| {
            let record_expr = Self::find_gtk_app_record_at_position_in_expr(expr, position)?;
            let field_name = Self::direct_record_field_name_at_position(record_expr, position)?;
            Self::gtk_app_field_doc(&field_name.name)
        };

        for item in &module.items {
            match item {
                ModuleItem::Def(def) => {
                    if let Some(contents) = scan_expr(&def.expr) {
                        return Some(contents);
                    }
                }
                ModuleItem::InstanceDecl(instance) => {
                    for def in &instance.defs {
                        if let Some(contents) = scan_expr(&def.expr) {
                            return Some(contents);
                        }
                    }
                }
                ModuleItem::DomainDecl(domain) => {
                    for item in &domain.items {
                        if let aivi::DomainItem::Def(def) | aivi::DomainItem::LiteralDef(def) = item
                        {
                            if let Some(contents) = scan_expr(&def.expr) {
                                return Some(contents);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        None
    }

    /// Find the TypeExpr from a TypeSig for the given identifier in a module.
    fn find_type_sig_in_module(module: &Module, ident: &str) -> Option<aivi::TypeExpr> {
        let paren_ident = format!("({ident})");
        for item in module.items.iter() {
            match item {
                ModuleItem::TypeSig(sig)
                    if sig.name.name == ident || sig.name.name == paren_ident =>
                {
                    return Some(sig.ty.clone());
                }
                ModuleItem::ClassDecl(class_decl) => {
                    for member in class_decl.members.iter() {
                        if member.name.name == ident || member.name.name == paren_ident {
                            return Some(member.ty.clone());
                        }
                    }
                }
                ModuleItem::DomainDecl(domain_decl) => {
                    for di in domain_decl.items.iter() {
                        if let aivi::DomainItem::TypeSig(sig) = di {
                            if sig.name.name == ident || sig.name.name == paren_ident {
                                return Some(sig.ty.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Append type definitions for non-primitive types referenced in the ident's signature.
    fn append_type_definitions(
        contents: &mut String,
        ident: &str,
        resolved_module: &Module,
        current_module: &Module,
        workspace_modules: &HashMap<String, IndexedModule>,
    ) {
        let Some(type_expr) = Self::find_type_sig_in_module(resolved_module, ident) else {
            return;
        };
        let names = Self::collect_type_names(&type_expr);
        if names.is_empty() {
            return;
        }
        let mut defs = Vec::new();
        for name in &names {
            if let Some(brief) = Self::find_type_definition_brief(current_module, name) {
                defs.push(brief);
                continue;
            }
            if !std::ptr::eq(resolved_module, current_module) {
                if let Some(brief) = Self::find_type_definition_brief(resolved_module, name) {
                    defs.push(brief);
                    continue;
                }
            }
            let mut found = false;
            for use_decl in current_module.uses.iter() {
                let original = resolve_import_name(&use_decl.items, name);
                let imported = use_decl.wildcard || use_decl.items.is_empty() || original.is_some();
                if !imported {
                    continue;
                }
                let lookup = original.unwrap_or(name);
                if let Some(indexed) = workspace_modules.get(&use_decl.module.name) {
                    if let Some(brief) = Self::find_type_definition_brief(&indexed.module, lookup) {
                        defs.push(brief);
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                // Search all workspace modules as last resort
                for indexed in workspace_modules.values() {
                    if let Some(brief) = Self::find_type_definition_brief(&indexed.module, name) {
                        defs.push(brief);
                        break;
                    }
                }
            }
        }
        if defs.is_empty() {
            return;
        }
        contents.push_str("\n\n---\n\n");
        for (i, def) in defs.iter().enumerate() {
            contents.push('`');
            contents.push_str(def);
            contents.push('`');
            if i < defs.len() - 1 {
                contents.push_str("\n\n");
            }
        }
    }

    fn hover_fallback_for_unresolved_ident(ident: &str) -> String {
        let is_operator = !ident.is_empty()
            && ident
                .chars()
                .any(|ch| !ch.is_alphanumeric() && ch != '_' && ch != '.');
        let kind = if is_operator { "operator" } else { "value" };
        let base = format!(
            "`{ident}` : `_unresolved_`\n\nNo type signature was resolved in the current module/import scope."
        );
        Self::hover_badge_markdown(kind, base)
    }

    fn expr_contains_position_for_hover(expr: &aivi::Expr, position: Position) -> bool {
        let range = Self::span_to_range(Self::expr_span(expr).clone());
        Self::range_contains_position(&range, position)
    }

    fn collect_pattern_binders(pattern: &aivi::Pattern, out: &mut Vec<String>) {
        match pattern {
            aivi::Pattern::Ident(name) | aivi::Pattern::SubjectIdent(name) => {
                out.push(name.name.clone())
            }
            aivi::Pattern::At { name, pattern, .. } => {
                out.push(name.name.clone());
                Self::collect_pattern_binders(pattern, out);
            }
            aivi::Pattern::Tuple { items, .. } => {
                for item in items {
                    Self::collect_pattern_binders(item, out);
                }
            }
            aivi::Pattern::List { items, rest, .. } => {
                for item in items {
                    Self::collect_pattern_binders(item, out);
                }
                if let Some(rest) = rest {
                    Self::collect_pattern_binders(rest, out);
                }
            }
            aivi::Pattern::Record { fields, .. } => {
                for field in fields {
                    Self::collect_pattern_binders(&field.pattern, out);
                }
            }
            aivi::Pattern::Constructor { args, .. } => {
                for arg in args {
                    Self::collect_pattern_binders(arg, out);
                }
            }
            aivi::Pattern::Wildcard(_) | aivi::Pattern::Literal(_) => {}
        }
    }

    fn pattern_has_binding_at_position(
        pattern: &aivi::Pattern,
        ident: &str,
        position: Position,
    ) -> bool {
        match pattern {
            aivi::Pattern::Ident(name) | aivi::Pattern::SubjectIdent(name) => {
                name.name == ident
                    && Self::range_contains_position(
                        &Self::span_to_range(name.span.clone()),
                        position,
                    )
            }
            aivi::Pattern::At { name, pattern, .. } => {
                (name.name == ident
                    && Self::range_contains_position(
                        &Self::span_to_range(name.span.clone()),
                        position,
                    ))
                    || Self::pattern_has_binding_at_position(pattern, ident, position)
            }
            aivi::Pattern::Tuple { items, .. } => items
                .iter()
                .any(|item| Self::pattern_has_binding_at_position(item, ident, position)),
            aivi::Pattern::List { items, rest, .. } => {
                items
                    .iter()
                    .any(|item| Self::pattern_has_binding_at_position(item, ident, position))
                    || rest.as_deref().is_some_and(|rest| {
                        Self::pattern_has_binding_at_position(rest, ident, position)
                    })
            }
            aivi::Pattern::Record { fields, .. } => fields.iter().any(|field| {
                Self::pattern_has_binding_at_position(&field.pattern, ident, position)
            }),
            aivi::Pattern::Constructor { args, .. } => args
                .iter()
                .any(|arg| Self::pattern_has_binding_at_position(arg, ident, position)),
            aivi::Pattern::Wildcard(_) | aivi::Pattern::Literal(_) => false,
        }
    }

    fn local_binding_visible_in_expr(
        expr: &aivi::Expr,
        ident: &str,
        position: Position,
        in_scope: &mut Vec<String>,
    ) -> bool {
        if !Self::expr_contains_position_for_hover(expr, position) {
            return false;
        }
        match expr {
            aivi::Expr::Ident(name) => {
                name.name == ident
                    && in_scope.iter().any(|bound| bound == ident)
                    && Self::range_contains_position(
                        &Self::span_to_range(name.span.clone()),
                        position,
                    )
            }
            aivi::Expr::Lambda { params, body, .. } => {
                if params
                    .iter()
                    .any(|param| Self::pattern_has_binding_at_position(param, ident, position))
                {
                    return true;
                }
                let mut scoped = in_scope.clone();
                for param in params {
                    Self::collect_pattern_binders(param, &mut scoped);
                }
                Self::local_binding_visible_in_expr(body, ident, position, &mut scoped)
            }
            aivi::Expr::Match {
                scrutinee, arms, ..
            } => {
                if scrutinee.as_ref().is_some_and(|s| {
                    Self::local_binding_visible_in_expr(s, ident, position, in_scope)
                }) {
                    return true;
                }
                for arm in arms {
                    let arm_range = Self::span_to_range(arm.span.clone());
                    if !Self::range_contains_position(&arm_range, position) {
                        continue;
                    }
                    if Self::pattern_has_binding_at_position(&arm.pattern, ident, position) {
                        return true;
                    }
                    let mut scoped = in_scope.clone();
                    Self::collect_pattern_binders(&arm.pattern, &mut scoped);
                    if arm.guard.as_ref().is_some_and(|g| {
                        Self::local_binding_visible_in_expr(g, ident, position, &mut scoped)
                    }) || Self::local_binding_visible_in_expr(
                        &arm.body,
                        ident,
                        position,
                        &mut scoped,
                    ) {
                        return true;
                    }
                }
                false
            }
            aivi::Expr::Block { items, .. } => {
                let mut scoped = in_scope.clone();
                for item in items {
                    match item {
                        aivi::BlockItem::Bind { pattern, expr, .. }
                        | aivi::BlockItem::Let { pattern, expr, .. } => {
                            if Self::local_binding_visible_in_expr(
                                expr,
                                ident,
                                position,
                                &mut scoped,
                            ) {
                                return true;
                            }
                            if Self::pattern_has_binding_at_position(pattern, ident, position) {
                                return true;
                            }
                            Self::collect_pattern_binders(pattern, &mut scoped);
                        }
                        aivi::BlockItem::Filter { expr, .. }
                        | aivi::BlockItem::Yield { expr, .. }
                        | aivi::BlockItem::Recurse { expr, .. }
                        | aivi::BlockItem::Expr { expr, .. } => {
                            if Self::local_binding_visible_in_expr(
                                expr,
                                ident,
                                position,
                                &mut scoped,
                            ) {
                                return true;
                            }
                        }
                        aivi::BlockItem::When { cond, effect, .. }
                        | aivi::BlockItem::Unless { cond, effect, .. } => {
                            if Self::local_binding_visible_in_expr(
                                cond,
                                ident,
                                position,
                                &mut scoped,
                            ) || Self::local_binding_visible_in_expr(
                                effect,
                                ident,
                                position,
                                &mut scoped,
                            ) {
                                return true;
                            }
                        }
                        aivi::BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            if Self::local_binding_visible_in_expr(
                                cond,
                                ident,
                                position,
                                &mut scoped,
                            ) || Self::local_binding_visible_in_expr(
                                fail_expr,
                                ident,
                                position,
                                &mut scoped,
                            ) {
                                return true;
                            }
                        }
                        aivi::BlockItem::On {
                            transition,
                            handler,
                            ..
                        } => {
                            if Self::local_binding_visible_in_expr(
                                transition,
                                ident,
                                position,
                                &mut scoped,
                            ) || Self::local_binding_visible_in_expr(
                                handler,
                                ident,
                                position,
                                &mut scoped,
                            ) {
                                return true;
                            }
                        }
                    }
                }
                false
            }
            aivi::Expr::CapabilityScope { body, .. } => {
                Self::local_binding_visible_in_expr(body, ident, position, in_scope)
            }
            aivi::Expr::Suffixed { base, .. } | aivi::Expr::UnaryNeg { expr: base, .. } => {
                Self::local_binding_visible_in_expr(base, ident, position, in_scope)
            }
            aivi::Expr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
                aivi::TextPart::Text { .. } => false,
                aivi::TextPart::Expr { expr, .. } => {
                    Self::local_binding_visible_in_expr(expr, ident, position, in_scope)
                }
            }),
            aivi::Expr::List { items, .. } => items.iter().any(|item| {
                Self::local_binding_visible_in_expr(&item.expr, ident, position, in_scope)
            }),
            aivi::Expr::Tuple { items, .. } => items
                .iter()
                .any(|item| Self::local_binding_visible_in_expr(item, ident, position, in_scope)),
            aivi::Expr::Index { base, index, .. } => {
                Self::local_binding_visible_in_expr(base, ident, position, in_scope)
                    || Self::local_binding_visible_in_expr(index, ident, position, in_scope)
            }
            aivi::Expr::Call { func, args, .. } => {
                Self::local_binding_visible_in_expr(func, ident, position, in_scope)
                    || args.iter().any(|arg| {
                        Self::local_binding_visible_in_expr(arg, ident, position, in_scope)
                    })
            }
            aivi::Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                Self::local_binding_visible_in_expr(cond, ident, position, in_scope)
                    || Self::local_binding_visible_in_expr(then_branch, ident, position, in_scope)
                    || Self::local_binding_visible_in_expr(else_branch, ident, position, in_scope)
            }
            aivi::Expr::Binary { left, right, .. } => {
                Self::local_binding_visible_in_expr(left, ident, position, in_scope)
                    || Self::local_binding_visible_in_expr(right, ident, position, in_scope)
            }
            aivi::Expr::Record { fields, .. } | aivi::Expr::PatchLit { fields, .. } => {
                fields.iter().any(|field| {
                    Self::local_binding_visible_in_expr(&field.value, ident, position, in_scope)
                })
            }
            aivi::Expr::FieldAccess { base, field, .. } => {
                (field.name == ident
                    && in_scope.iter().any(|bound| bound == ident)
                    && Self::range_contains_position(
                        &Self::span_to_range(field.span.clone()),
                        position,
                    ))
                    || Self::local_binding_visible_in_expr(base, ident, position, in_scope)
            }
            aivi::Expr::FieldSection { field, .. } => {
                field.name == ident
                    && in_scope.iter().any(|bound| bound == ident)
                    && Self::range_contains_position(
                        &Self::span_to_range(field.span.clone()),
                        position,
                    )
            }
            aivi::Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                substitutions.iter().any(|sub| {
                    sub.value.as_ref().is_some_and(|v| {
                        Self::local_binding_visible_in_expr(v, ident, position, in_scope)
                    })
                }) || Self::local_binding_visible_in_expr(body, ident, position, in_scope)
            }
            aivi::Expr::Literal(_) | aivi::Expr::Raw { .. } => false,
        }
    }

    fn hover_contents_for_local_binding(
        module: &Module,
        ident: &str,
        position: Position,
        inferred: Option<&HashMap<String, String>>,
        workspace_modules: Option<&HashMap<String, IndexedModule>>,
    ) -> Option<String> {
        fn pattern_binds_name(pattern: &aivi::Pattern, name: &str) -> bool {
            match pattern {
                aivi::Pattern::Ident(n) | aivi::Pattern::SubjectIdent(n) => n.name == name,
                aivi::Pattern::At {
                    name: n, pattern, ..
                } => n.name == name || pattern_binds_name(pattern, name),
                aivi::Pattern::Tuple { items, .. } => {
                    items.iter().any(|item| pattern_binds_name(item, name))
                }
                aivi::Pattern::List { items, rest, .. } => {
                    items.iter().any(|item| pattern_binds_name(item, name))
                        || rest
                            .as_deref()
                            .is_some_and(|rest| pattern_binds_name(rest, name))
                }
                aivi::Pattern::Record { fields, .. } => fields
                    .iter()
                    .any(|field| pattern_binds_name(&field.pattern, name)),
                aivi::Pattern::Constructor { args, .. } => {
                    args.iter().any(|arg| pattern_binds_name(arg, name))
                }
                aivi::Pattern::Wildcard(_) | aivi::Pattern::Literal(_) => false,
            }
        }

        fn position_after(range: &tower_lsp::lsp_types::Range, position: Position) -> bool {
            position.line > range.end.line
                || (position.line == range.end.line && position.character >= range.end.character)
        }

        fn local_binding_expr_for_ident<'a>(
            expr: &'a aivi::Expr,
            ident: &str,
            position: Position,
        ) -> Option<(&'a aivi::Expr, bool)> {
            let range = Backend::span_to_range(Backend::expr_span(expr).clone());
            if !Backend::range_contains_position(&range, position) {
                return None;
            }

            let aivi::Expr::Block { items, .. } = expr else {
                return None;
            };

            let mut latest: Option<(&aivi::Expr, bool)> = None;
            for item in items {
                let item_span = match item {
                    aivi::BlockItem::Bind { span, .. }
                    | aivi::BlockItem::Let { span, .. }
                    | aivi::BlockItem::Filter { span, .. }
                    | aivi::BlockItem::Yield { span, .. }
                    | aivi::BlockItem::Recurse { span, .. }
                    | aivi::BlockItem::Expr { span, .. }
                    | aivi::BlockItem::When { span, .. }
                    | aivi::BlockItem::Unless { span, .. }
                    | aivi::BlockItem::Given { span, .. }
                    | aivi::BlockItem::On { span, .. } => span,
                };
                let item_range = Backend::span_to_range(item_span.clone());
                if !Backend::range_contains_position(&item_range, position)
                    && !position_after(&item_range, position)
                {
                    continue;
                }
                if Backend::range_contains_position(&item_range, position) {
                    if let aivi::BlockItem::Expr { expr, .. } = item {
                        if let Some(found) = local_binding_expr_for_ident(expr, ident, position) {
                            return Some(found);
                        }
                    }
                    if let aivi::BlockItem::Bind { pattern, expr, .. }
                    | aivi::BlockItem::Let { pattern, expr, .. } = item
                    {
                        if pattern_binds_name(pattern, ident) {
                            return Some((expr, matches!(item, aivi::BlockItem::Bind { .. })));
                        }
                        if let Some(found) = local_binding_expr_for_ident(expr, ident, position) {
                            return Some(found);
                        }
                    }
                } else if let aivi::BlockItem::Bind { pattern, expr, .. }
                | aivi::BlockItem::Let { pattern, expr, .. } = item
                {
                    if pattern_binds_name(pattern, ident) {
                        latest = Some((expr, matches!(item, aivi::BlockItem::Bind { .. })));
                    }
                }
            }

            latest
        }

        fn type_sig_expr_in_module(module: &Module, ident: &str) -> Option<aivi::TypeExpr> {
            module.items.iter().find_map(|item| match item {
                aivi::ModuleItem::TypeSig(sig) if sig.name.name == ident => Some(sig.ty.clone()),
                _ => None,
            })
        }

        fn resolve_type_sig_expr(
            current_module: &Module,
            ident: &str,
            workspace_modules: &HashMap<String, IndexedModule>,
        ) -> Option<aivi::TypeExpr> {
            if let Some(ty) = type_sig_expr_in_module(current_module, ident) {
                return Some(ty);
            }
            for use_decl in current_module.uses.iter() {
                let original = resolve_import_name(&use_decl.items, ident);
                let imported = use_decl.wildcard || use_decl.items.is_empty() || original.is_some();
                if !imported {
                    continue;
                }
                let lookup = original.unwrap_or(ident);
                let Some(indexed) = workspace_modules.get(&use_decl.module.name) else {
                    continue;
                };
                if let Some(ty) = type_sig_expr_in_module(&indexed.module, lookup) {
                    return Some(ty);
                }
            }
            None
        }

        fn apply_call_args(ty: &aivi::TypeExpr, mut args: usize) -> aivi::TypeExpr {
            let mut current = ty.clone();
            while args > 0 {
                match current {
                    aivi::TypeExpr::Func { params, result, .. } => {
                        if params.len() <= args {
                            args -= params.len();
                            current = *result;
                        } else {
                            return *result;
                        }
                    }
                    _ => return current,
                }
            }
            current
        }

        fn extract_bind_value_type(ty: &aivi::TypeExpr) -> aivi::TypeExpr {
            match ty {
                aivi::TypeExpr::Apply { base, args, .. }
                    if !args.is_empty()
                        && matches!(
                            base.as_ref(),
                            aivi::TypeExpr::Name(name)
                                if name.name == "Effect" || name.name == "Task"
                                    || name.name == "Result" || name.name == "Resource"
                                    || name.name == "Source"
                        ) =>
                {
                    // The value type is always the last type argument
                    // (e.g. Effect E A -> A, Result E A -> A, Task A -> A)
                    args.last().cloned().unwrap_or_else(|| ty.clone())
                }
                _ => ty.clone(),
            }
        }

        fn root_type_name(ty: &aivi::TypeExpr) -> Option<String> {
            match ty {
                aivi::TypeExpr::Name(name) => Some(
                    name.name
                        .rsplit('.')
                        .next()
                        .unwrap_or(&name.name)
                        .to_string(),
                ),
                aivi::TypeExpr::Apply { base, .. } => root_type_name(base),
                _ => None,
            }
        }

        fn find_alias_definition(module: &Module, alias_name: &str) -> Option<String> {
            module.items.iter().find_map(|item| match item {
                aivi::ModuleItem::TypeAlias(alias) if alias.name.name == alias_name => {
                    Some(format!(
                        "type {} = {}",
                        alias.name.name,
                        Backend::type_expr_to_string(&alias.aliased)
                    ))
                }
                _ => None,
            })
        }

        fn alias_definition_for_type(
            current_module: &Module,
            ty: &aivi::TypeExpr,
            workspace_modules: &HashMap<String, IndexedModule>,
        ) -> Option<String> {
            let alias_name = root_type_name(ty)?;
            if let Some(alias) = find_alias_definition(current_module, &alias_name) {
                return Some(alias);
            }
            for use_decl in current_module.uses.iter() {
                let original = resolve_import_name(&use_decl.items, &alias_name);
                let imported = use_decl.wildcard || use_decl.items.is_empty() || original.is_some();
                if !imported {
                    continue;
                }
                let lookup = original.unwrap_or(&alias_name);
                let Some(indexed) = workspace_modules.get(&use_decl.module.name) else {
                    continue;
                };
                if let Some(alias) = find_alias_definition(&indexed.module, lookup) {
                    return Some(alias);
                }
            }
            None
        }

        fn callee_ident_name(expr: &aivi::Expr) -> Option<String> {
            match expr {
                aivi::Expr::Ident(name) => Some(name.name.clone()),
                aivi::Expr::FieldAccess { field, .. } => Some(field.name.clone()),
                _ => None,
            }
        }

        for item in module.items.iter() {
            let aivi::ModuleItem::Def(def) = item else {
                continue;
            };
            if !Self::expr_contains_position_for_hover(&def.expr, position) {
                continue;
            }
            if def
                .params
                .iter()
                .any(|param| Self::pattern_has_binding_at_position(param, ident, position))
            {
                return Some(Self::hover_badge_markdown("value", format!("`{ident}`")));
            }
            let mut scope = Vec::new();
            for param in def.params.iter() {
                Self::collect_pattern_binders(param, &mut scope);
            }
            #[allow(clippy::collapsible_if, clippy::collapsible_match)]
            if Self::local_binding_visible_in_expr(&def.expr, ident, position, &mut scope) {
                if let Some(workspace_modules) = workspace_modules {
                    if let Some((bound_expr, is_bind)) =
                        local_binding_expr_for_ident(&def.expr, ident, position)
                    {
                        if let aivi::Expr::Call { func, args, .. } = bound_expr {
                            if let Some(callee) = callee_ident_name(func) {
                                if let Some(sig_ty) =
                                    resolve_type_sig_expr(module, &callee, workspace_modules)
                                {
                                    let mut ty = apply_call_args(&sig_ty, args.len());
                                    if is_bind {
                                        ty = extract_bind_value_type(&ty);
                                    }
                                    let ty_string = Self::type_expr_to_string(&ty);
                                    let mut base = format!("`{ident}` : `{ty_string}`");
                                    if let Some(alias_def) =
                                        alias_definition_for_type(module, &ty, workspace_modules)
                                    {
                                        base.push_str(&format!("\n\n`{alias_def}`"));
                                    }
                                    return Some(Self::hover_badge_markdown("value", base));
                                }
                            }
                        }
                    }
                }
                let base = inferred
                    .and_then(|types| types.get(ident))
                    .map(|ty| format!("`{ident}` : `{ty}`"));
                if let Some(base) = base {
                    return Some(Self::hover_badge_markdown("value", base));
                }
                // No type info available; fall through so span_types fallback can provide it.
                return None;
            }
        }
        None
    }

    pub(super) fn find_record_field_name_at_position(
        expr: &aivi::Expr,
        position: Position,
    ) -> Option<&aivi::SpannedName> {
        use aivi::Expr;
        match expr {
            Expr::Suffixed { base, .. } => Self::find_record_field_name_at_position(base, position),
            Expr::UnaryNeg { expr, .. } => Self::find_record_field_name_at_position(expr, position),
            Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
                for field in fields.iter() {
                    for segment in field.path.iter() {
                        if let aivi::PathSegment::Field(name) = segment {
                            let range = Self::span_to_range(name.span.clone());
                            if Self::range_contains_position(&range, position) {
                                return Some(name);
                            }
                        }
                    }
                    if let Some(found) =
                        Self::find_record_field_name_at_position(&field.value, position)
                    {
                        return Some(found);
                    }
                }
                None
            }
            Expr::FieldAccess { base, field, .. } => {
                let range = Self::span_to_range(field.span.clone());
                if Self::range_contains_position(&range, position) {
                    return Some(field);
                }
                Self::find_record_field_name_at_position(base, position)
            }
            Expr::FieldSection { field, .. } => {
                let range = Self::span_to_range(field.span.clone());
                if Self::range_contains_position(&range, position) {
                    return Some(field);
                }
                None
            }
            Expr::Ident(_) | Expr::Literal(_) | Expr::Raw { .. } => None,
            Expr::Mock {
                substitutions,
                body,
                ..
            } => substitutions
                .iter()
                .find_map(|sub| {
                    sub.value
                        .as_ref()
                        .and_then(|v| Self::find_record_field_name_at_position(v, position))
                })
                .or_else(|| Self::find_record_field_name_at_position(body, position)),
            Expr::TextInterpolate { parts, .. } => parts.iter().find_map(|part| match part {
                aivi::TextPart::Text { .. } => None,
                aivi::TextPart::Expr { expr, .. } => {
                    Self::find_record_field_name_at_position(expr, position)
                }
            }),
            Expr::List { items, .. } => items
                .iter()
                .find_map(|item| Self::find_record_field_name_at_position(&item.expr, position)),
            Expr::Tuple { items, .. } => items
                .iter()
                .find_map(|item| Self::find_record_field_name_at_position(item, position)),
            Expr::Index { base, index, .. } => {
                Self::find_record_field_name_at_position(base, position)
                    .or_else(|| Self::find_record_field_name_at_position(index, position))
            }
            Expr::Call { func, args, .. } => {
                Self::find_record_field_name_at_position(func, position).or_else(|| {
                    args.iter()
                        .find_map(|arg| Self::find_record_field_name_at_position(arg, position))
                })
            }
            Expr::Lambda {
                params: _, body, ..
            } => Self::find_record_field_name_at_position(body, position),
            Expr::Match {
                scrutinee, arms, ..
            } => scrutinee
                .as_ref()
                .and_then(|expr| Self::find_record_field_name_at_position(expr, position))
                .or_else(|| {
                    arms.iter().find_map(|arm| {
                        Self::find_record_field_name_at_position(&arm.body, position).or_else(
                            || {
                                arm.guard.as_ref().and_then(|guard| {
                                    Self::find_record_field_name_at_position(guard, position)
                                })
                            },
                        )
                    })
                }),
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => Self::find_record_field_name_at_position(cond, position)
                .or_else(|| Self::find_record_field_name_at_position(then_branch, position))
                .or_else(|| Self::find_record_field_name_at_position(else_branch, position)),
            Expr::Binary { left, right, .. } => {
                Self::find_record_field_name_at_position(left, position)
                    .or_else(|| Self::find_record_field_name_at_position(right, position))
            }
            Expr::CapabilityScope { body, .. } => {
                Self::find_record_field_name_at_position(body, position)
            }
            Expr::Block { items, .. } => items.iter().find_map(|item| match item {
                aivi::BlockItem::Bind { expr, .. }
                | aivi::BlockItem::Let { expr, .. }
                | aivi::BlockItem::Filter { expr, .. }
                | aivi::BlockItem::Yield { expr, .. }
                | aivi::BlockItem::Recurse { expr, .. }
                | aivi::BlockItem::Expr { expr, .. } => {
                    Self::find_record_field_name_at_position(expr, position)
                }
                aivi::BlockItem::When { cond, effect, .. }
                | aivi::BlockItem::Unless { cond, effect, .. } => {
                    Self::find_record_field_name_at_position(cond, position)
                        .or_else(|| Self::find_record_field_name_at_position(effect, position))
                }
                aivi::BlockItem::Given {
                    cond, fail_expr, ..
                } => Self::find_record_field_name_at_position(cond, position)
                    .or_else(|| Self::find_record_field_name_at_position(fail_expr, position)),
                aivi::BlockItem::On {
                    transition,
                    handler,
                    ..
                } => Self::find_record_field_name_at_position(transition, position)
                    .or_else(|| Self::find_record_field_name_at_position(handler, position)),
            }),
        }
    }

    pub(super) fn type_sig_for_value<'a>(
        module: &'a Module,
        value_name: &str,
    ) -> Option<&'a aivi::TypeSig> {
        for item in module.items.iter() {
            match item {
                aivi::ModuleItem::TypeSig(sig) if sig.name.name == value_name => return Some(sig),
                aivi::ModuleItem::DomainDecl(domain) => {
                    for domain_item in domain.items.iter() {
                        if let aivi::DomainItem::TypeSig(sig) = domain_item {
                            if sig.name.name == value_name {
                                return Some(sig);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(super) fn type_alias_named<'a>(
        module: &'a Module,
        type_name: &str,
    ) -> Option<&'a aivi::TypeAlias> {
        for item in module.items.iter() {
            match item {
                aivi::ModuleItem::TypeAlias(alias) if alias.name.name == type_name => {
                    return Some(alias);
                }
                _ => {}
            }
        }
        None
    }

    pub(crate) fn build_hover(
        text: &str,
        uri: &Url,
        position: Position,
        doc_index: &DocIndex,
    ) -> Option<Hover> {
        let started = Instant::now();
        let ident = match Self::extract_identifier(text, position) {
            Some(ident) => ident,
            None => {
                Self::hover_debug(format!(
                    "build_hover: no token at {}:{}",
                    position.line, position.character
                ));
                return None;
            }
        };
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        Self::hover_debug(format!(
            "build_hover: token={ident:?}, modules={}",
            modules.len()
        ));
        if let Some(contents) = Self::hover_for_gtk_app_field(&modules, position) {
            Self::hover_debug(format!(
                "build_hover: resolved gtkApp field {ident:?} after {:?}",
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }
        let (_, inferred, span_types) = infer_value_types(&modules);
        for module in modules.iter() {
            let doc = Self::doc_for_ident(text, module, &ident);
            let inferred = inferred.get(&module.name.name);
            if let Some(mut contents) =
                Self::hover_contents_for_module(module, &ident, inferred, doc.as_deref(), doc_index)
            {
                Self::append_gtk_architecture_docs(&mut contents, &ident);
                Self::append_source_tooling_docs(&mut contents, &ident);
                Self::hover_debug(format!(
                    "build_hover: resolved in module {} after {:?}",
                    module.name.name,
                    started.elapsed()
                ));
                return Some(Self::hover_markdown(contents));
            }
        }
        if let Some(module) = Self::module_at_position(&modules, position) {
            if let Some(mut contents) = Self::hover_contents_for_local_binding(
                module,
                &ident,
                position,
                inferred.get(&module.name.name),
                None,
            ) {
                Self::append_source_tooling_docs(&mut contents, &ident);
                Self::hover_debug(format!(
                    "build_hover: resolved as local binding in {} after {:?}",
                    module.name.name,
                    started.elapsed()
                ));
                return Some(Self::hover_markdown(contents));
            }
            // Fallback: look up the smallest span containing the cursor position.
            if let Some(mut contents) =
                Self::hover_from_span_types(&ident, position, &span_types, &module.name.name)
            {
                Self::append_gtk_architecture_docs(&mut contents, &ident);
                Self::append_source_tooling_docs(&mut contents, &ident);
                Self::hover_debug(format!(
                    "build_hover: resolved from span types in {} after {:?}",
                    module.name.name,
                    started.elapsed()
                ));
                return Some(Self::hover_markdown(contents));
            }
        }
        if let Some(contents) = Self::gtk_architecture_doc_for_ident(&ident) {
            Self::hover_debug(format!(
                "build_hover: resolved gtk architecture doc {ident:?} after {:?}",
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }
        if let Some(contents) = Self::hover_contents_for_primitive_value(&ident) {
            Self::hover_debug(format!(
                "build_hover: resolved primitive token {ident:?} after {:?}",
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }
        if let Some(mut contents) = Self::hover_contents_for_static_source(&ident) {
            Self::append_source_tooling_docs(&mut contents, &ident);
            Self::hover_debug(format!(
                "build_hover: resolved static source {ident:?} after {:?}",
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }
        if let Some(contents) = Self::source_tooling_doc_for_ident(&ident) {
            Self::hover_debug(format!(
                "build_hover: resolved source tooling doc {ident:?} after {:?}",
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }
        Self::hover_debug(format!(
            "build_hover: unresolved token {ident:?}; returning generic fallback after {:?}",
            started.elapsed()
        ));
        Some(Self::hover_markdown(
            Self::hover_fallback_for_unresolved_ident(&ident),
        ))
    }

    /// Resolve hover for dotted member access like `Heap.push`, `Map.empty`,
    /// Qualified value access — looks up the prefix as a type/domain name in imported
    /// modules and then finds the member in that module.
    fn hover_for_dotted_member(
        ident: &str,
        current_module: &Module,
        workspace_modules: &HashMap<String, IndexedModule>,
        inferred: &HashMap<String, HashMap<String, String>>,
        doc_index: &DocIndex,
    ) -> Option<Hover> {
        let dot_pos = ident.find('.')?;
        let prefix = &ident[..dot_pos];
        let member = &ident[dot_pos + 1..];
        if prefix.is_empty() || member.is_empty() {
            return None;
        }

        // Look through imported modules for one that exports or defines the prefix
        // as a type, domain, or type alias. Then look up the member in that module.
        let modules_to_search =
            Self::find_modules_exporting(prefix, current_module, workspace_modules);

        Self::hover_debug(format!(
            "hover_for_dotted_member: prefix={prefix:?}, member={member:?}, modules_found={}",
            modules_to_search.len()
        ));

        for indexed in &modules_to_search {
            let inf = inferred.get(&indexed.module.name.name);
            let doc_text = indexed
                .uri
                .to_file_path()
                .ok()
                .and_then(|p| fs::read_to_string(p).ok());
            let doc = doc_text
                .as_deref()
                .and_then(|text| Self::doc_for_ident(text, &indexed.module, member));

            Self::hover_debug(format!(
                "hover_for_dotted_member: checking module={} has_inferred={}",
                indexed.module.name.name,
                inf.is_some()
            ));

            // Check domain members with the member name.
            if let Some(contents) = Self::hover_contents_for_module(
                &indexed.module,
                member,
                inf,
                doc.as_deref(),
                doc_index,
            ) {
                let mut contents = contents;
                Self::append_source_tooling_docs(&mut contents, &format!("{prefix}.{member}"));
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: contents,
                    }),
                    range: None,
                });
            }
        }

        // Also check the current module itself (the prefix might be defined locally).
        let doc = Self::doc_for_ident("", current_module, member);
        let inf = inferred.get(&current_module.name.name);
        if let Some(contents) =
            Self::hover_contents_for_module(current_module, member, inf, doc.as_deref(), doc_index)
        {
            let mut contents = contents;
            Self::append_source_tooling_docs(&mut contents, &format!("{prefix}.{member}"));
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: contents,
                }),
                range: None,
            });
        }

        None
    }

    /// Find modules that export or define a name (type, domain, type alias, etc.)
    /// matching the given prefix. Searches the current module's `use` imports.
    fn find_modules_exporting<'a>(
        name: &str,
        current_module: &Module,
        workspace_modules: &'a HashMap<String, IndexedModule>,
    ) -> Vec<&'a IndexedModule> {
        let mut result = Vec::new();
        for use_decl in current_module.uses.iter() {
            let imports_name =
                use_decl.wildcard || resolve_import_name(&use_decl.items, name).is_some();
            if !imports_name {
                continue;
            }
            if let Some(indexed) = workspace_modules.get(&use_decl.module.name) {
                result.push(indexed);
            }
        }

        // Also check modules imported without item lists (bare `use aivi.collections`)
        // where the module itself may export the name.
        for use_decl in current_module.uses.iter() {
            if !use_decl.items.is_empty() || use_decl.wildcard {
                continue;
            }
            if let Some(indexed) = workspace_modules.get(&use_decl.module.name) {
                // Check if this module exports the name.
                let exports_name = indexed.module.exports.iter().any(|e| e.name.name == name);
                if exports_name && !result.iter().any(|r| r.uri == indexed.uri) {
                    result.push(indexed);
                }
            }
        }

        // Also check the prelude module if present.
        if let Some(prelude) = workspace_modules.get("aivi.prelude") {
            let exports_name = prelude.module.exports.iter().any(|e| e.name.name == name);
            if exports_name && !result.iter().any(|r| r.uri == prelude.uri) {
                result.push(prelude);
            }
        }

        // Finally check all workspace modules that define this as a domain/type,
        // since the name could come from the core module (e.g. `Heap` from `aivi`).
        if result.is_empty() {
            for indexed in workspace_modules.values() {
                let defines_name = indexed.module.items.iter().any(|item| match item {
                    ModuleItem::TypeDecl(decl) => decl.name.name == name,
                    ModuleItem::DomainDecl(domain) => {
                        // Domain's `over` type might match (e.g. domain MinHeap over Heap a)
                        domain.name.name == name
                    }
                    ModuleItem::TypeAlias(alias) => alias.name.name == name,
                    _ => false,
                });
                if defines_name {
                    result.push(indexed);
                }
            }
        }

        result
    }

    pub(crate) fn build_hover_with_workspace(
        text: &str,
        uri: &Url,
        position: Position,
        workspace_modules: &HashMap<String, IndexedModule>,
        doc_index: &DocIndex,
    ) -> Option<Hover> {
        let started = Instant::now();
        let ident = match Self::extract_identifier(text, position) {
            Some(ident) => ident,
            None => {
                Self::hover_debug(format!(
                    "build_hover_ws: no token at {}:{}",
                    position.line, position.character
                ));
                return None;
            }
        };
        let path = PathBuf::from(Self::path_from_uri(uri));
        let (modules, _) = parse_modules(&path, text);
        Self::hover_debug(format!(
            "build_hover_ws: token={ident:?}, file_modules={}, workspace_modules={}",
            modules.len(),
            workspace_modules.len()
        ));
        if let Some(contents) = Self::hover_for_gtk_app_field(&modules, position) {
            Self::hover_debug(format!(
                "build_hover_ws: resolved gtkApp field {ident:?} after {:?}",
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }
        let current_module = Self::module_at_position(&modules, position);
        let Some(current_module) = current_module else {
            Self::hover_debug("build_hover_ws: no module at cursor; skipping workspace hover");
            return None;
        };

        // Only infer types for the current file's modules + direct imports (not the
        // entire workspace) to keep hover responsive in large projects.
        let relevant_modules =
            Self::collect_relevant_modules(&modules, current_module, workspace_modules);
        let (_, inferred, span_types) = infer_value_types(&relevant_modules);
        Self::hover_debug(format!(
            "build_hover_ws: inferred over {} relevant modules",
            relevant_modules.len()
        ));

        // Handle dotted identifiers: first check if it's a full module name (e.g.
        // "aivi.collections"), then check Domain.method / Type.constructor patterns.
        if ident.contains('.') {
            // 0. Compile-time @static source names (e.g. type.jsonSchema, file.read).
            if let Some(mut contents) = Self::hover_contents_for_static_source(&ident) {
                Self::append_source_tooling_docs(&mut contents, &ident);
                Self::hover_debug(format!(
                    "build_hover_ws: resolved static source {} after {:?}",
                    ident,
                    started.elapsed()
                ));
                return Some(Self::hover_markdown(contents));
            }
            if let Some(contents) = Self::source_tooling_doc_for_ident(&ident) {
                Self::hover_debug(format!(
                    "build_hover_ws: resolved source tooling doc {} after {:?}",
                    ident,
                    started.elapsed()
                ));
                return Some(Self::hover_markdown(contents));
            }

            // 1. Exact module name match.
            if let Some(indexed) = workspace_modules.get(&ident) {
                let doc_text = indexed
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| fs::read_to_string(path).ok());
                let doc = doc_text
                    .as_deref()
                    .and_then(|text| Self::doc_for_ident(text, &indexed.module, &ident));
                let inferred = inferred.get(&indexed.module.name.name);
                if let Some(contents) = Self::hover_contents_for_module(
                    &indexed.module,
                    &ident,
                    inferred,
                    doc.as_deref(),
                    doc_index,
                ) {
                    let mut contents = contents;
                    Self::append_source_tooling_docs(&mut contents, &ident);
                    Self::hover_debug(format!(
                        "build_hover_ws: resolved dotted module {} after {:?}",
                        ident,
                        started.elapsed()
                    ));
                    return Some(Self::hover_markdown(contents));
                }
            }

            // 2. Domain.method or Type.constructor (e.g. "Heap.push", "Map.empty").
            if let Some(mut hover) = Self::hover_for_dotted_member(
                &ident,
                current_module,
                workspace_modules,
                &inferred,
                doc_index,
            ) {
                if let HoverContents::Markup(markup) = &mut hover.contents {
                    Self::append_source_tooling_docs(&mut markup.value, &ident);
                }
                Self::hover_debug(format!(
                    "build_hover_ws: resolved dotted member {} after {:?}",
                    ident,
                    started.elapsed()
                ));
                return Some(hover);
            }
        }

        let doc = Self::doc_for_ident(text, current_module, &ident);
        let inferred_current = inferred.get(&current_module.name.name);
        if let Some(mut contents) = Self::hover_contents_for_module(
            current_module,
            &ident,
            inferred_current,
            doc.as_deref(),
            doc_index,
        ) {
            Self::append_type_definitions(
                &mut contents,
                &ident,
                current_module,
                current_module,
                workspace_modules,
            );
            Self::append_gtk_architecture_docs(&mut contents, &ident);
            Self::append_source_tooling_docs(&mut contents, &ident);
            Self::hover_debug(format!(
                "build_hover_ws: resolved in current module {} after {:?}",
                current_module.name.name,
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }

        for use_decl in current_module.uses.iter() {
            let original = resolve_import_name(&use_decl.items, &ident);
            let imported = use_decl.wildcard || original.is_some();
            if !imported {
                continue;
            }
            let lookup = original.unwrap_or(&ident);
            let Some(indexed) = workspace_modules.get(&use_decl.module.name) else {
                continue;
            };
            let doc_text = indexed
                .uri
                .to_file_path()
                .ok()
                .and_then(|path| fs::read_to_string(path).ok());
            let doc = doc_text
                .as_deref()
                .and_then(|text| Self::doc_for_ident(text, &indexed.module, lookup));
            let inferred = inferred.get(&indexed.module.name.name);
            if let Some(mut contents) = Self::hover_contents_for_module(
                &indexed.module,
                lookup,
                inferred,
                doc.as_deref(),
                doc_index,
            ) {
                Self::append_type_definitions(
                    &mut contents,
                    lookup,
                    &indexed.module,
                    current_module,
                    workspace_modules,
                );
                Self::append_gtk_architecture_docs(&mut contents, lookup);
                Self::append_source_tooling_docs(&mut contents, &ident);
                Self::hover_debug(format!(
                    "build_hover_ws: resolved via import {} after {:?}",
                    use_decl.module.name,
                    started.elapsed()
                ));
                return Some(Self::hover_markdown(contents));
            }
        }

        if let Some(mut contents) = Self::hover_contents_for_local_binding(
            current_module,
            &ident,
            position,
            inferred_current,
            Some(workspace_modules),
        ) {
            Self::append_source_tooling_docs(&mut contents, &ident);
            Self::hover_debug(format!(
                "build_hover_ws: resolved local binding in {} after {:?}",
                current_module.name.name,
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }
        // Fallback: look up the smallest span containing the cursor position.
        if let Some(mut contents) =
            Self::hover_from_span_types(&ident, position, &span_types, &current_module.name.name)
        {
            Self::append_gtk_architecture_docs(&mut contents, &ident);
            Self::append_source_tooling_docs(&mut contents, &ident);
            Self::hover_debug(format!(
                "build_hover_ws: resolved from span types in {} after {:?}",
                current_module.name.name,
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }
        if let Some(contents) = Self::gtk_architecture_doc_for_ident(&ident) {
            Self::hover_debug(format!(
                "build_hover_ws: resolved gtk architecture doc {ident:?} after {:?}",
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }
        if let Some(contents) = Self::hover_contents_for_primitive_value(&ident) {
            Self::hover_debug(format!(
                "build_hover_ws: resolved primitive token {ident:?} after {:?}",
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }
        if let Some(contents) = Self::source_tooling_doc_for_ident(&ident) {
            Self::hover_debug(format!(
                "build_hover_ws: resolved source tooling doc {ident:?} after {:?}",
                started.elapsed()
            ));
            return Some(Self::hover_markdown(contents));
        }
        Self::hover_debug(format!(
            "build_hover_ws: unresolved token {ident:?}; returning generic fallback after {:?}",
            started.elapsed()
        ));
        Some(Self::hover_markdown(
            Self::hover_fallback_for_unresolved_ident(&ident),
        ))
    }
}
