(B) Product truth present.

Authoritative evidence:
- `docs/fork/recovery/RECOVERY_PLAN.md:286-290`: upstream five-tier schema, prices, budgets, and model floors are **not fork product truth**, and current two-tier handling is temporary fail-closed compatibility.
- `docs/fork/recovery/PROGRESS.md:181-192`: operator supplied W4 product truth, rejects upstream expanding tiers/prices/budgets/model floors, and treats `Plus`/`Flagship` as temporary compatibility.
- `docs/fork/recovery/seams/R02-config-provider-routing/ledger.md:278-283`: W4 catalog-schema sync is closed as documented **non-adoption**. Route half should reconcile current HEAD and close with targeted tests/docs if already satisfied.
- OpenRouter definitive-absence suppression is already test-pinned in `crates/jcode-base/src/provider/catalog_routes.rs:1341-1346` and asserted at `:1360-1376`.

Narrow exact compose/test path set:
- `docs/fork/recovery/RECOVERY_PLAN.md`
- `docs/fork/recovery/PROGRESS.md`
- `docs/fork/recovery/seams/R02-config-provider-routing/ledger.md`
- `crates/jcode-base/src/provider/catalog_routes.rs`
- Only if documenting current fail-closed compatibility: `crates/jcode-base/src/subscription_catalog.rs`, `crates/jcode-base/src/subscription_api.rs`, `crates/jcode-base/src/provider/tests/catalog_subscription.rs`

No reproduced regression found that justifies reopening tier governance. Earlier stale-cache regression was recorded at R02 ledger `:107-109`, but current HEAD has correction coverage. Residuals found are presentational/restart-window, not governance-reopening evidence.