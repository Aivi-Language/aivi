#[derive(Clone)]
enum CapabilityScopeOrigin {
    Signature { def_name: String, span: Span },
    WithScope { span: Span },
}

#[derive(Clone)]
struct CapabilityScopeFrame {
    capabilities: CapabilitySet,
    origin: CapabilityScopeOrigin,
}

impl TypeChecker {
    fn normalize_capabilities(&self, capabilities: &[SpannedName]) -> CapabilitySet {
        capabilities.iter().map(|cap| cap.name.clone()).collect()
    }

    fn collect_expr_capabilities(
        &mut self,
        expr: &Expr,
        env: &TypeEnv,
        scopes: &[CapabilityScopeFrame],
        emit_diags: bool,
    ) -> CapabilitySet {
        match expr {
            Expr::Ident(_) | Expr::Literal(_) | Expr::FieldSection { .. } | Expr::Raw { .. } => {
                CapabilitySet::default()
            }
            Expr::UnaryNeg { expr, .. } => {
                self.collect_expr_capabilities(expr, env, scopes, emit_diags)
            }
            Expr::Suffixed { base, .. } => {
                self.collect_expr_capabilities(base, env, scopes, emit_diags)
            }
            Expr::TextInterpolate { parts, .. } => {
                let mut caps = CapabilitySet::default();
                for part in parts {
                    if let TextPart::Expr { expr, .. } = part {
                        caps.extend(self.collect_expr_capabilities(expr, env, scopes, emit_diags));
                    }
                }
                caps
            }
            Expr::List { items, .. } => {
                items
                    .iter()
                    .fold(CapabilitySet::default(), |mut caps, item| {
                        caps.extend(
                            self.collect_expr_capabilities(&item.expr, env, scopes, emit_diags),
                        );
                        caps
                    })
            }
            Expr::Tuple { items, .. } => {
                items
                    .iter()
                    .fold(CapabilitySet::default(), |mut caps, item| {
                        caps.extend(self.collect_expr_capabilities(item, env, scopes, emit_diags));
                        caps
                    })
            }
            Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
                let mut caps = CapabilitySet::default();
                for field in fields {
                    for segment in &field.path {
                        if let PathSegment::Index(index, _) = segment {
                            caps.extend(
                                self.collect_expr_capabilities(index, env, scopes, emit_diags),
                            );
                        }
                    }
                    caps.extend(self.collect_expr_capabilities(
                        &field.value,
                        env,
                        scopes,
                        emit_diags,
                    ));
                }
                caps
            }
            Expr::FieldAccess { base, .. } => {
                self.collect_expr_capabilities(base, env, scopes, emit_diags)
            }
            Expr::Index { base, index, .. } => {
                let mut caps = self.collect_expr_capabilities(base, env, scopes, emit_diags);
                caps.extend(self.collect_expr_capabilities(index, env, scopes, emit_diags));
                caps
            }
            Expr::Call { func, args, .. } => {
                let mut caps = self.collect_expr_capabilities(func, env, scopes, emit_diags);
                for arg in args {
                    caps.extend(self.collect_expr_capabilities(arg, env, scopes, emit_diags));
                }
                let call_caps = self.capabilities_for_call(func, args, env);
                if emit_diags {
                    self.emit_missing_capability_diags(expr_span(expr), &call_caps, scopes);
                }
                caps.extend(call_caps);
                caps
            }
            Expr::Lambda { body, .. } => {
                self.collect_expr_capabilities(body, env, scopes, emit_diags)
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                let mut caps = CapabilitySet::default();
                if let Some(scrutinee) = scrutinee {
                    caps.extend(self.collect_expr_capabilities(scrutinee, env, scopes, emit_diags));
                }
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        caps.extend(self.collect_expr_capabilities(guard, env, scopes, emit_diags));
                    }
                    caps.extend(self.collect_expr_capabilities(&arm.body, env, scopes, emit_diags));
                }
                caps
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                let mut caps = self.collect_expr_capabilities(cond, env, scopes, emit_diags);
                caps.extend(self.collect_expr_capabilities(then_branch, env, scopes, emit_diags));
                caps.extend(self.collect_expr_capabilities(else_branch, env, scopes, emit_diags));
                caps
            }
            Expr::Binary { left, right, .. } => {
                let mut caps = self.collect_expr_capabilities(left, env, scopes, emit_diags);
                caps.extend(self.collect_expr_capabilities(right, env, scopes, emit_diags));
                caps
            }
            Expr::CapabilityScope {
                capabilities,
                handlers,
                body,
                span,
            } => {
                let mut caps = CapabilitySet::default();
                for handler in handlers {
                    caps.extend(self.collect_expr_capabilities(
                        &handler.handler,
                        env,
                        scopes,
                        emit_diags,
                    ));
                }
                let mut nested = scopes.to_vec();
                nested.push(CapabilityScopeFrame {
                    capabilities: self.normalize_capabilities(capabilities),
                    origin: CapabilityScopeOrigin::WithScope { span: span.clone() },
                });
                caps.extend(self.collect_expr_capabilities(body, env, &nested, emit_diags));
                caps
            }
            Expr::Block { items, .. } => {
                let mut caps = CapabilitySet::default();
                for item in items {
                    match item {
                        BlockItem::Bind { expr, .. }
                        | BlockItem::Let { expr, .. }
                        | BlockItem::Yield { expr, .. }
                        | BlockItem::Recurse { expr, .. }
                        | BlockItem::Expr { expr, .. } => {
                            caps.extend(
                                self.collect_expr_capabilities(expr, env, scopes, emit_diags),
                            );
                        }
                        BlockItem::Filter { expr, .. } => {
                            caps.extend(
                                self.collect_expr_capabilities(expr, env, scopes, emit_diags),
                            );
                        }
                        BlockItem::When { cond, effect, .. }
                        | BlockItem::Unless { cond, effect, .. } => {
                            caps.extend(
                                self.collect_expr_capabilities(cond, env, scopes, emit_diags),
                            );
                            caps.extend(
                                self.collect_expr_capabilities(effect, env, scopes, emit_diags),
                            );
                        }
                        BlockItem::Given {
                            cond, fail_expr, ..
                        } => {
                            caps.extend(
                                self.collect_expr_capabilities(cond, env, scopes, emit_diags),
                            );
                            caps.extend(
                                self.collect_expr_capabilities(fail_expr, env, scopes, emit_diags),
                            );
                        }
                        BlockItem::On {
                            transition,
                            handler,
                            ..
                        } => {
                            caps.extend(
                                self.collect_expr_capabilities(transition, env, scopes, emit_diags),
                            );
                            caps.extend(
                                self.collect_expr_capabilities(handler, env, scopes, emit_diags),
                            );
                        }
                    }
                }
                caps
            }
            Expr::Mock {
                substitutions,
                body,
                ..
            } => {
                let mut caps = CapabilitySet::default();
                for substitution in substitutions {
                    if let Some(value) = &substitution.value {
                        caps.extend(self.collect_expr_capabilities(value, env, scopes, emit_diags));
                    }
                }
                caps.extend(self.collect_expr_capabilities(body, env, scopes, emit_diags));
                caps
            }
        }
    }

    fn capabilities_for_call(&self, func: &Expr, args: &[Expr], env: &TypeEnv) -> CapabilitySet {
        let Some(path) = self.resolve_expr_path(func) else {
            return CapabilitySet::default();
        };
        let mut caps = self.lookup_path_capabilities(&path, env);
        if path == "load" {
            if let Some(source_expr) = args.first() {
                caps.extend(self.capabilities_for_loaded_source(source_expr, env));
            }
        }
        caps
    }

    fn capabilities_for_loaded_source(&self, expr: &Expr, env: &TypeEnv) -> CapabilitySet {
        if let Some(path) = self.resolve_expr_path(expr) {
            let caps = self.lookup_builtin_source_capabilities(&path);
            if !caps.is_empty() {
                return caps;
            }
            let env_caps = self.lookup_env_capabilities(&path, env);
            if !env_caps.is_empty() {
                return env_caps;
            }
        }
        if let Expr::Call { func, .. } = expr {
            if let Some(path) = self.resolve_expr_path(func) {
                let caps = self.lookup_builtin_source_capabilities(&path);
                if !caps.is_empty() {
                    return caps;
                }
            }
        }
        CapabilitySet::default()
    }

    fn lookup_path_capabilities(&self, path: &str, env: &TypeEnv) -> CapabilitySet {
        let builtin = self.lookup_builtin_call_capabilities(path);
        if !builtin.is_empty() {
            return builtin;
        }
        self.lookup_env_capabilities(path, env)
    }

    fn lookup_env_capabilities(&self, path: &str, env: &TypeEnv) -> CapabilitySet {
        let mut caps = CapabilitySet::default();
        if let Some(schemes) = env.get_all(path) {
            for scheme in schemes {
                caps.extend(scheme.capabilities.iter().cloned());
            }
        }
        caps
    }

    fn lookup_builtin_source_capabilities(&self, path: &str) -> CapabilitySet {
        self.capability_set(match path {
            "file.read" | "file.json" | "file.csv" | "file.image" | "file.imageMeta" => {
                &["file.read"]
            }
            "http.get" | "http.post" | "http.fetch" | "https.get" | "https.post"
            | "https.fetch" | "rest.get" | "rest.post" | "rest.fetch" | "__openapi_call" => {
                &["network.http"]
            }
            "env.get" | "env.decode" | "system.env.get" | "system.env.decode" => {
                &["process.env.read"]
            }
            "email.imap" => &["network"],
            _ => &[],
        })
    }

    fn lookup_builtin_call_capabilities(&self, path: &str) -> CapabilitySet {
        self.capability_set(match path {
            "file.open" | "file.readAll" => &["file.read"],
            "file.close" | "file.write_text" | "file.delete" => &["file.write"],
            "file.exists" | "file.stat" => &["file.metadata"],
            "http.get" | "http.post" | "http.fetch" | "https.get" | "https.post"
            | "https.fetch" | "rest.get" | "rest.post" | "rest.fetch" | "__openapi_call" => {
                &["network.http"]
            }
            "sockets.listen" => &["network.socket.listen"],
            "sockets.connect" => &["network.socket.connect"],
            "sockets.accept"
            | "sockets.send"
            | "sockets.recv"
            | "sockets.close"
            | "sockets.closeListener"
            | "email.imap"
            | "email.smtpSend" => &["network"],
            "env.set" | "env.remove" | "system.env.set" | "system.env.remove" => {
                &["process.env.write"]
            }
            "system.args" => &["process.args"],
            "system.exit" => &["process.exit"],
            "clock.now" => &["clock.now"],
            "concurrent.sleep" => &["clock.sleep"],
            "concurrent.timeoutWith" => &["clock.sleep", "cancellation.propagate"],
            "concurrent.scope"
            | "concurrent.par"
            | "concurrent.race"
            | "concurrent.spawnDetached"
            | "concurrent.fork"
            | "concurrent.cancelToken" => &["cancellation.propagate"],
            "random.int" => &["randomness.pseudo"],
            "crypto.randomUuid" | "crypto.randomBytes" => &["randomness.secure"],
            "gtk4.init"
            | "gtk4.appNew"
            | "gtk4.windowNew"
            | "gtk4.windowSetTitle"
            | "gtk4.windowSetChild"
            | "gtk4.windowPresent"
            | "gtk4.windowSetHideOnClose"
            | "gtk4.windowSetDefaultSize"
            | "gtk4.windowShow"
            | "gtk4.buttonNew"
            | "gtk4.entryNew"
            | "gtk4.boxNew"
            | "gtk4.boxAppend"
            | "gtk4.labelNew"
            | "gtk4.reconcileNode"
            | "gtk4.signalStream"
            | "gtk4.signalPoll"
            | "gtk4.clipboardSetText"
            | "gtk4.notificationShow" => &["ui"],
            "database.configure"
            | "database.connect"
            | "database.close"
            | "database.configureSqlite"
            | "database.configureSqliteOn" => &["db.connect"],
            "database.load" | "database.loadOn" => &["db.query"],
            "database.applyDelta"
            | "database.applyDeltaOn"
            | "database.beginTx"
            | "database.commitTx"
            | "database.rollbackTx"
            | "database.beginTxOn"
            | "database.commitTxOn"
            | "database.rollbackTxOn"
            | "database.savepoint"
            | "database.savepointOn"
            | "database.releaseSavepoint"
            | "database.releaseSavepointOn"
            | "database.rollbackToSavepoint"
            | "database.rollbackToSavepointOn" => &["db.mutate"],
            "database.runMigrations"
            | "database.runMigrationsOn"
            | "database.runMigrationSql"
            | "database.runMigrationSqlOn" => &["db.migrate"],
            _ => &[],
        })
    }

    fn capability_set(&self, caps: &[&str]) -> CapabilitySet {
        caps.iter().map(|cap| (*cap).to_string()).collect()
    }

    fn resolve_expr_path(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(name) => Some(name.name.clone()),
            Expr::FieldAccess { base, field, .. } => {
                Some(format!("{}.{}", self.resolve_expr_path(base)?, field.name))
            }
            Expr::CapabilityScope { body, .. } => self.resolve_expr_path(body),
            _ => None,
        }
    }

    fn emit_missing_capability_diags(
        &mut self,
        span: Span,
        required: &CapabilitySet,
        scopes: &[CapabilityScopeFrame],
    ) {
        for capability in required {
            if scopes
                .iter()
                .all(|scope| Self::scope_allows(&scope.capabilities, capability))
            {
                continue;
            }
            let origin = scopes
                .iter()
                .rev()
                .find(|scope| !Self::scope_allows(&scope.capabilities, capability))
                .map(|scope| scope.origin.clone());
            let (message, hint_span, label_message) = match origin {
                Some(CapabilityScopeOrigin::Signature { def_name, span }) => (
                    format!(
                        "`{def_name}` uses capability `{capability}` here, but its signature does not declare it"
                    ),
                    span,
                    Some(format!(
                        "signature for `{def_name}` is missing capability `{capability}`"
                    )),
                ),
                Some(CapabilityScopeOrigin::WithScope { span }) => (
                    format!(
                        "capability `{capability}` is not available inside this `with {{ ... }} in` scope"
                    ),
                    span,
                    Some(format!(
                        "this `with {{ ... }} in` clause does not include `{capability}`"
                    )),
                ),
                None => (
                    format!("capability `{capability}` is not available in this scope"),
                    span.clone(),
                    None,
                ),
            };
            let mut hints = vec![format!(
                "add `{capability}` to the enclosing function signature or `with {{ ... }}` clause"
            )];
            let mut labels = Vec::new();
            if hint_span != span {
                if let Some(message) = label_message {
                    labels.push(DiagnosticLabel {
                        message,
                        span: hint_span.clone(),
                    });
                }
                hints.push("the restrictive scope is highlighted separately".to_string());
            }
            self.extra_diagnostics.push(FileDiagnostic {
                path: self.current_module_path.clone(),
                diagnostic: Diagnostic {
                    code: "E3310".to_string(),
                    severity: crate::diagnostics::DiagnosticSeverity::Error,
                    message,
                    span: span.clone(),
                    labels,
                    hints,
                    suggestion: None,
                },
            });
        }
    }

    fn scope_allows(scope: &CapabilitySet, required: &str) -> bool {
        scope
            .iter()
            .any(|available| Self::capability_covers(available, required))
    }

    fn capability_covers(available: &str, required: &str) -> bool {
        available == required
            || required
                .strip_prefix(available)
                .is_some_and(|suffix| suffix.starts_with('.'))
    }
}
