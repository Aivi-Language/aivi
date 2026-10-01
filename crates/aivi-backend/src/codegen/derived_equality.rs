impl<'a, M: Module> CraneliftCompiler<'a, M> {
    /// Resolve a bounded compiler graph against the native representation.
    /// Recursive representations retain the existing unsupported-layout boundary.
    fn resolve_derived_native_equality(
        &self,
        location: (KernelId, KernelExprId),
        shape: &aivi_hir::EqualityShape,
        id: aivi_hir::EqualityNodeId,
        layout: LayoutId,
        evidence: &[LayoutId],
        active: &mut HashSet<(aivi_hir::EqualityNodeId, LayoutId)>,
    ) -> Result<NativeEqualityShape, CodegenError> {
        let (kernel, expr) = location;
        use aivi_hir::EqualityShapeNode as Node;
        if active.len() >= 128 || !active.insert((id, layout)) {
            return Err(self.unsupported_expression(
                kernel,
                expr,
                "recursive derived equality requires a compiled representation bridge",
            ));
        }
        let invalid = || {
            self.unsupported_expression(
                kernel,
                expr,
                "derived equality shape differs from its closed layout",
            )
        };
        let result = (|| match shape.node(id).ok_or_else(invalid)? {
            Node::Structural => {
                self.resolve_native_equality_shape(kernel, expr, layout, &mut HashSet::new())
            }
            Node::Evidence(slot) => {
                let slot = slot.as_raw() as usize;
                let callable = *evidence.get(slot).ok_or_else(invalid)?;
                let (parameters, result) = self.callable_signature(callable);
                if parameters.as_slice() != [layout, layout] {
                    return Err(invalid());
                }
                self.require_bool_expression(
                    kernel,
                    expr,
                    result,
                    "derived equality evidence result",
                )?;
                Ok(NativeEqualityShape::Evidence {
                    slot,
                    callable_layout: callable,
                })
            }
            Node::Carrier(child) => self
                .resolve_derived_native_equality(location, shape, *child, layout, evidence, active),
            Node::Tuple(nodes) => {
                let LayoutKind::Tuple(fields) = &self.program.layouts()[layout].kind else {
                    return Err(invalid());
                };
                self.resolve_derived_equality_fields(
                    location, shape, nodes, fields, evidence, active,
                )
            }
            Node::Record(nodes) => {
                let LayoutKind::Record(fields) = &self.program.layouts()[layout].kind else {
                    return Err(invalid());
                };
                if fields.len() != nodes.len() {
                    return Err(invalid());
                }
                let ids = fields
                    .iter()
                    .map(|field| {
                        nodes
                            .iter()
                            .find(|node| node.name == field.name)
                            .map(|node| node.node)
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(invalid)?;
                let layouts = fields.iter().map(|field| field.layout).collect::<Vec<_>>();
                self.resolve_derived_equality_fields(
                    location, shape, &ids, &layouts, evidence, active,
                )
            }
            Node::Option(child) => {
                let LayoutKind::Option { element } = self.program.layouts()[layout].kind else {
                    return Err(invalid());
                };
                let payload = Box::new(self.resolve_derived_native_equality(
                    location, shape, *child, element, evidence, active,
                )?);
                match self.option_codegen_contract(layout) {
                    Some(OptionCodegenContract::InlineScalar(_)) => {
                        Ok(NativeEqualityShape::DerivedScalarOption { layout, payload })
                    }
                    Some(OptionCodegenContract::NicheReference) => {
                        Ok(NativeEqualityShape::NicheOption { payload })
                    }
                    None => Err(invalid()),
                }
            }
            Node::Sum(nodes) => {
                let variants = match &self.program.layouts()[layout].kind {
                    LayoutKind::Opaque { variants, .. } | LayoutKind::Sum(variants) => variants,
                    _ => return Err(invalid()),
                };
                if variants.len() != nodes.len() {
                    return Err(invalid());
                }
                let mut compiled = Vec::with_capacity(variants.len());
                for variant in variants {
                    let node = nodes
                        .iter()
                        .find(|node| node.name == variant.name)
                        .ok_or_else(invalid)?;
                    if variant.field_count != node.fields.len() {
                        return Err(invalid());
                    }
                    let payload_shape = match (variant.payload, node.fields.as_slice()) {
                        (None, []) => None,
                        (Some(layout), [id]) => {
                            Some(Box::new(self.resolve_derived_native_equality(
                                location, shape, *id, layout, evidence, active,
                            )?))
                        }
                        (Some(layout), ids) => {
                            let LayoutKind::Tuple(fields) = &self.program.layouts()[layout].kind
                            else {
                                return Err(invalid());
                            };
                            Some(Box::new(self.resolve_derived_equality_fields(
                                location, shape, ids, fields, evidence, active,
                            )?))
                        }
                        _ => return Err(invalid()),
                    };
                    compiled.push(NativeEqualityVariant {
                        tag: crate::layout::opaque_variant_tag(&variant.name),
                        payload_layout: variant.payload,
                        payload_shape,
                    });
                }
                Ok(NativeEqualityShape::TaggedPayloadSum(compiled))
            }
            Node::List(_) | Node::Result { .. } | Node::Validation { .. } => Err(self
                .unsupported_expression(
                    kernel,
                    expr,
                    "derived equality for this carrier requires its compiled representation bridge",
                )),
        })();
        active.remove(&(id, layout));
        result
    }

    fn resolve_derived_equality_fields(
        &self,
        location: (KernelId, KernelExprId),
        shape: &aivi_hir::EqualityShape,
        nodes: &[aivi_hir::EqualityNodeId],
        layouts: &[LayoutId],
        evidence: &[LayoutId],
        active: &mut HashSet<(aivi_hir::EqualityNodeId, LayoutId)>,
    ) -> Result<NativeEqualityShape, CodegenError> {
        let (kernel, expr) = location;
        if nodes.len() != layouts.len() {
            return Err(self.unsupported_expression(
                kernel,
                expr,
                "derived equality aggregate arity differs",
            ));
        }
        let mut fields = Vec::with_capacity(nodes.len());
        let mut offset = 0_u32;
        for (node, layout) in nodes.iter().zip(layouts) {
            let abi = self.field_abi_shape(kernel, *layout, "derived equality field")?;
            offset = align_to(offset, abi.align);
            let shape = self.resolve_derived_native_equality(
                location, shape, *node, *layout, evidence, active,
            )?;
            fields.push(NativeEqualityField {
                offset: i32::try_from(offset).map_err(|_| {
                    self.unsupported_expression(
                        kernel,
                        expr,
                        "derived equality field offset exceeds native limits",
                    )
                })?,
                layout: *layout,
                shape: Box::new(shape),
            });
            offset = offset.checked_add(abi.size).ok_or_else(|| {
                self.unsupported_expression(
                    kernel,
                    expr,
                    "derived equality aggregate size overflow",
                )
            })?;
        }
        Ok(NativeEqualityShape::Aggregate(fields))
    }
}
