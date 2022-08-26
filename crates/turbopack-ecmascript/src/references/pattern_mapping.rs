use std::collections::HashMap;

use anyhow::Result;
use swc_ecma_ast::{Expr, Lit};
use swc_ecma_quote::quote;
use turbo_tasks::{debug::ValueDebug, primitives::StringVc, Value, ValueToString};
use turbo_tasks_fs::FileSystemPathVc;
use turbopack_core::{
    chunk::ModuleId,
    issue::{code_gen::CodeGenerationIssue, IssueSeverity},
    resolve::{ResolveResult, ResolveResultVc, SpecialType},
};

use crate::{utils::module_id_to_lit, EcmascriptChunkContextVc, EcmascriptChunkPlaceableVc};

/// A mapping from a request pattern (e.g. "./module", `./images/${name}.png`)
/// to corresponding module ids. The same pattern can map to multiple module ids
/// at runtime when using variable interpolation.
#[turbo_tasks::value]
pub(crate) enum PatternMapping {
    /// Invalid request.
    Invalid,
    /// Constant request that always maps to the same module.
    ///
    /// ### Example
    /// ```js
    /// require("./module")
    /// ```
    Single(ModuleId),
    /// Variable request that can map to different modules at runtime.
    ///
    /// ### Example
    /// ```js
    /// require(`./images/${name}.png`)
    /// ```
    Map(HashMap<String, ModuleId>),
    /// Original reference
    OriginalReferenceExternal,
    /// Original reference with different request
    OriginalReferenceTypeExternal(String),
}

#[derive(PartialOrd, Ord, Hash, Debug, Copy, Clone)]
#[turbo_tasks::value(serialization = "auto_for_input")]
pub(crate) enum ResolveType {
    EsmAsync,
    Cjs,
}

impl PatternMapping {
    pub fn is_internal_import(&self) -> bool {
        match self {
            PatternMapping::Invalid | PatternMapping::Single(_) | PatternMapping::Map(_) => true,
            PatternMapping::OriginalReferenceExternal
            | PatternMapping::OriginalReferenceTypeExternal(_) => false,
        }
    }

    pub fn create(&self) -> Expr {
        match self {
            PatternMapping::Invalid => {
                // TODO improve error message
                quote!("(() => {throw new Error(\"Invalid\")})()" as Expr)
            }
            PatternMapping::Single(module_id) => module_id_to_lit(module_id),
            PatternMapping::Map(_) => {
                todo!("emit an error for this case: Complex expression can't be transformed");
            }
            PatternMapping::OriginalReferenceExternal => {
                todo!("emit an error for this case: apply need to be used");
            }
            PatternMapping::OriginalReferenceTypeExternal(s) => {
                Expr::Lit(Lit::Str(s.as_str().into()))
            }
        }
    }

    pub fn apply(&self, key_expr: Expr) -> Expr {
        match self {
            PatternMapping::OriginalReferenceExternal => key_expr,
            _ => self.create(),
        }
        // TODO handle PatternMapping::Map
    }
}

#[turbo_tasks::value_impl]
impl PatternMappingVc {
    /// Resolves a request into a pattern mapping.
    // NOTE(alexkirsz) I would rather have used `resolve` here but it's already reserved by the Vc
    // impl.
    #[turbo_tasks::function]
    pub async fn resolve_request(
        issue_context_path: FileSystemPathVc,
        chunk_context: EcmascriptChunkContextVc,
        resolve_result: ResolveResultVc,
        resolve_type: Value<ResolveType>,
    ) -> Result<PatternMappingVc> {
        let result = resolve_result.await?;
        let asset = match &*result {
            ResolveResult::Alternatives(assets, _) => {
                if let Some(asset) = assets.first() {
                    asset
                } else {
                    return Ok(PatternMappingVc::cell(PatternMapping::Invalid));
                }
            }
            ResolveResult::Single(asset, _) => asset,
            ResolveResult::Special(SpecialType::OriginalReferenceExternal, _) => {
                return Ok(PatternMapping::OriginalReferenceExternal.cell())
            }
            ResolveResult::Special(SpecialType::OriginalReferenceTypeExternal(s), _) => {
                return Ok(PatternMapping::OriginalReferenceTypeExternal(s.clone()).cell())
            }
            _ => {
                // TODO implement mapping
                CodeGenerationIssue {
                    severity: IssueSeverity::Bug.into(),
                    code: None,
                    title: StringVc::cell("not implemented result for pattern mapping".to_string()),
                    message: StringVc::cell(format!(
                        "the reference resolves to a non-trivial result, which is not supported \
                         yet: {:?}",
                        resolve_result.dbg().await?
                    )),
                    path: issue_context_path,
                }
                .cell()
                .as_issue()
                .emit();
                return Ok(PatternMappingVc::cell(PatternMapping::Invalid));
            }
        };

        if let Some(placeable) = EcmascriptChunkPlaceableVc::resolve_from(asset).await? {
            let name = if *resolve_type == ResolveType::EsmAsync {
                chunk_context.manifest_loader_id(*asset)
            } else {
                chunk_context.id(placeable)
            }
            .await?;
            Ok(PatternMappingVc::cell(PatternMapping::Single(name.clone())))
        } else {
            CodeGenerationIssue {
                severity: IssueSeverity::Bug.into(),
                code: None,
                title: StringVc::cell("non-ecmascript placeable asset".to_string()),
                message: StringVc::cell(format!(
                    "asset {} is not placeable in ESM chunks, so it doesn't have a module id",
                    asset.path().to_string().await?
                )),
                path: issue_context_path,
            }
            .cell()
            .as_issue()
            .emit();
            Ok(PatternMappingVc::cell(PatternMapping::Invalid))
        }
    }
}
