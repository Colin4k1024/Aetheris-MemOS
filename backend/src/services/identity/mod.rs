//! Principal identity + event stream service (#126).
//!
//! Thin facade over [`crate::db::principal::PrincipalRepository`] and
//! [`crate::db::memory_event::MemoryEventRepository`] giving callers the three
//! flows #126 promises:
//!
//! 1. **Authenticated mapping** — [`IdentityService::ensure_person_from_jwt`]
//!    maps a JWT `uid` claim to (at most) one person principal per tenant, so
//!    the same user resolving across sessions or devices always lands on the
//!    same identity. Callers wire this into authenticated request paths as they
//!    consume identities (#128); it is deliberately NOT invoked inside the global
//!    auth middleware, which must stay DB-free on the hot path.
//! 2. **Anonymous bucketing** — every pre-auth visitor gets a fresh anonymous
//!    principal ([`IdentityService::create_anonymous_principal`]); nothing links
//!    it anywhere until an explicit, audited merge.
//! 3. **Explicit reversible merges** — [`IdentityService::merge_anonymous_into_person`]
//!    / [`IdentityService::unmerge_anonymous`]. Both write lifecycle rows into
//!    the immutable event stream AND audit rows into `memory_audit_events`, in
//!    the same transaction as the mutation itself.
//!
//! Shared-device rule: device identifiers live ONLY on device-kind principals
//! ([`IdentityService::ensure_device_principal`]). Recording which device a
//! person used belongs in the event metadata — never as an alias on the person.
//!
//! NOTE: this is human/workload *identity*, distinct from
//! `services::agent_identity`, which manages an agent's own self-model.

use crate::db::memory_event::{content_hash_for, AppendOutcome, MemoryEventRepository};
use crate::db::principal::{EnsureOutcome, MemoryPrincipalRow, PrincipalRepository};
use crate::error::AppError;
use crate::models::memory_event::AppendMemoryEventRequest;
use crate::models::principal::{PrincipalAliasType, PrincipalKind};
use crate::tenant::TenantId;
use sqlx::PgPool;

pub use crate::db::principal::{AUDIT_EVENT_MERGED, AUDIT_EVENT_UNMERGED};

pub struct IdentityService {
    principals: PrincipalRepository,
    events: MemoryEventRepository,
}

impl IdentityService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            principals: PrincipalRepository::new(pool.clone()),
            events: MemoryEventRepository::new(pool),
        }
    }

    /// Repo-level access for read paths (resolution helpers live there).
    pub fn principals(&self) -> &PrincipalRepository {
        &self.principals
    }

    /// Append an event with a deterministic idempotency key derived from its
    /// content (`principal | session | type | payload`). Accidental double-sends
    /// collapse onto one row instead of duplicating history; producers that want
    /// explicit keys can bypass this via `events()` + `append`.
    pub async fn record_event(
        &self,
        tenant_id: &TenantId,
        mut req: AppendMemoryEventRequest,
    ) -> Result<AppendOutcome, AppError> {
        if req.idempotency_key.is_none() {
            let basis = serde_json::to_string(&req.payload_json)
                .map_err(|e| AppError::BadRequest(format!("payload not serializable: {e}")))?;
            req.idempotency_key = Some(content_hash_for(&format!(
                "{}|{}|{}|{}",
                req.principal_id,
                req.session_id.as_deref().unwrap_or("-"),
                req.event_type.as_str(),
                basis
            )));
        }
        self.events.append(tenant_id, req).await
    }

    /// Map an authenticated JWT user onto this tenant's person principal.
    ///
    /// Same `(jwt_sub, uid)` always resolves to the same principal regardless of
    /// session or device — the cross-device continuity guarantee.
    pub async fn ensure_person_from_jwt(
        &self,
        tenant_id: &TenantId,
        jwt_uid: &str,
        display_name: Option<&str>,
    ) -> Result<MemoryPrincipalRow, AppError> {
        let uid = jwt_uid.trim();
        if uid.is_empty() {
            return Err(AppError::BadRequest(
                "ensure_person_from_jwt: empty JWT uid".to_string(),
            ));
        }
        EnsureOutcome::into_row_if_valid_kind(
            self.principals
                .ensure_with_alias(
                    tenant_id,
                    PrincipalKind::Person,
                    display_name,
                    PrincipalAliasType::JwtSub,
                    uid,
                )
                .await?,
        )
    }

    /// Map a managed/hardware device id to ITS OWN device principal. Never links
    /// to any person — two users sharing the tablet stay two separate persons.
    pub async fn ensure_device_principal(
        &self,
        tenant_id: &TenantId,
        device_id: &str,
    ) -> Result<MemoryPrincipalRow, AppError> {
        let did = device_id.trim();
        if did.is_empty() {
            return Err(AppError::BadRequest(
                "ensure_device_principal: empty device id".to_string(),
            ));
        }
        EnsureOutcome::into_row_if_valid_kind(
            self.principals
                .ensure_with_alias(
                    tenant_id,
                    PrincipalKind::Device,
                    None,
                    PrincipalAliasType::DeviceId,
                    did,
                )
                .await?,
        )
    }

    /// Mint a fresh anonymous principal (one per pre-auth visitor/session family).
    ///
    /// Deliberately NO lookup-by-name here: anonymity means "no durable handle",
    /// so nothing may auto-link two anonymous buckets together.
    pub async fn create_anonymous_principal(
        &self,
        tenant_id: &TenantId,
        display_name: Option<&str>,
    ) -> Result<MemoryPrincipalRow, AppError> {
        self.principals
            .create(tenant_id, PrincipalKind::Anonymous, display_name)
            .await
    }

    /// Merge an anonymous principal into a person (explicit + audited + reversible).
    /// Returns the person root resolution afterwards.
    pub async fn merge_anonymous_into_person(
        &self,
        tenant_id: &TenantId,
        anonymous_id: &str,
        person_id: &str,
        actor: Option<&str>,
    ) -> Result<MemoryPrincipalRow, AppError> {
        self.principals
            .merge_anonymous(tenant_id, anonymous_id, person_id, actor)
            .await?;
        let anon = self
            .principals
            .get(tenant_id, anonymous_id)
            .await?
            .ok_or_else(|| {
                AppError::Internal("merged principal vanished immediately after commit".to_string())
            })?;
        let (root, _hops) = self.principals.follow_merge_chain(tenant_id, anon).await?;
        Ok(root)
    }

    /// Undo an explicit merge. Returns the previously-attached person id.
    pub async fn unmerge_anonymous(
        &self,
        tenant_id: &TenantId,
        anonymous_id: &str,
        actor: Option<&str>,
    ) -> Result<String, AppError> {
        self.principals
            .unmerge_anonymous(tenant_id, anonymous_id, actor)
            .await
    }
}

impl EnsureOutcome {
    /// Collapse into the row while asserting creation semantics stay attached to
    /// a kind the guard accepted (the repo already validated kinds; this is a
    /// last-line destructure helper so call sites never rebuild rows by hand).
    fn into_row_if_valid_kind(outcome: Self) -> Result<MemoryPrincipalRow, AppError> {
        outcome
            .principal
            .kind()
            .map_err(|raw| {
                AppError::Internal(format!("corrupt principal kind '{raw}' post-insert"))
            })
            .map(|_| outcome.principal)
    }
}
