//! MCP tool surface for the SRM platform.
//!
//! Reads are `read_only`. Writes with real-world weight — issuing/cancelling POs,
//! awarding sourcing, suspending/disqualifying suppliers — are gated
//! (`requires_approval = true` in the manifest).

use crate::store::SupplierStore;
use crate::types::*;
use adk_mcp_sdk::{HealthCheck, HealthStatus};
use chrono::NaiveDate;
use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use std::sync::Arc;

fn dactor() -> String { "agent".into() }
fn dusd() -> String { "USD".into() }
fn date(s: &Option<String>) -> Option<NaiveDate> { s.as_ref().and_then(|x| NaiveDate::parse_from_str(x, "%Y-%m-%d").ok()) }
fn today() -> NaiveDate { chrono::Utc::now().date_naive() }

// ─── inputs ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateSupplierInput { pub name: String, #[serde(default)] pub category: String, #[serde(default)] pub country: String, #[serde(default = "dlt")] pub lead_time_days: u32, #[serde(default = "dactor")] pub actor: String }
fn dlt() -> u32 { 14 }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SupplierIdInput { pub supplier_id: String }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListSuppliersInput { pub category: Option<String>, pub status: Option<SupplierStatus> }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetStatusInput { pub supplier_id: String, pub status: SupplierStatus, #[serde(default = "dactor")] pub actor: String }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetQualificationInput { pub supplier_id: String, pub qualification: QualificationStatus, #[serde(default = "dactor")] pub actor: String }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddContactInput { pub supplier_id: String, pub name: String, #[serde(default)] pub role: String, pub email: String, pub phone: Option<String>, #[serde(default)] pub primary: bool, #[serde(default = "dactor")] pub actor: String }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddCertInput { pub supplier_id: String, pub standard: String, pub certificate_number: String, #[serde(default)] pub issued_by: String, pub issued_on: String, pub expires_on: String, #[serde(default = "dactor")] pub actor: String }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExpiringCertsInput { #[serde(default = "d90")] pub within_days: i64 }
fn d90() -> i64 { 90 }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateItemInput { pub sku: String, pub name: String, #[serde(default)] pub category: String, #[serde(default = "dea")] pub unit: String, #[serde(default = "dactor")] pub actor: String }
fn dea() -> String { "ea".into() }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListItemsInput { pub category: Option<String> }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSupplierItemInput {
    pub supplier_id: String,
    pub item_id: String,
    #[serde(default)] pub supplier_sku: String,
    pub unit_price: f64,
    #[serde(default = "dusd")] pub currency: String,
    #[serde(default = "dmoq")] pub min_order_qty: u32,
    #[serde(default = "dlt")] pub lead_time_days: u32,
    #[serde(default)] pub available_qty: u32,
    #[serde(default)] pub preferred: bool,
    #[serde(default = "dactor")] pub actor: String,
}
fn dmoq() -> u32 { 1 }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FindSourcesInput { pub item_id: String, #[serde(default = "done")] pub required_qty: u32, #[serde(default = "dtrue")] pub qualified_only: bool }
fn done() -> u32 { 1 }
fn dtrue() -> bool { true }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PoLineInput { pub item_id: String, #[serde(default)] pub description: String, pub qty: u32, pub unit_price: f64 }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreatePoInput { pub supplier_id: String, #[serde(default = "dusd")] pub currency: String, pub lines: Vec<PoLineInput>, pub need_by: Option<String>, #[serde(default = "dactor")] pub actor: String }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PoIdInput { pub po_id: String }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListPosInput { pub supplier_id: Option<String>, pub status: Option<PoStatus> }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReceiptLineInput { pub item_id: String, pub qty: u32 }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReceivePoInput { pub po_id: String, pub receipts: Vec<ReceiptLineInput>, #[serde(default = "dactor")] pub actor: String }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CancelPoInput { pub po_id: String, #[serde(default)] pub reason: String, #[serde(default = "dactor")] pub actor: String }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateRfqInput { pub item_id: String, pub qty: u32, #[serde(default)] pub invited_suppliers: Vec<String>, pub need_by: Option<String>, #[serde(default = "dactor")] pub actor: String }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SubmitQuoteInput { pub rfq_id: String, pub supplier_id: String, pub unit_price: f64, #[serde(default = "dusd")] pub currency: String, #[serde(default = "dlt")] pub lead_time_days: u32, pub valid_until: Option<String>, #[serde(default = "dactor")] pub actor: String }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RfqIdInput { pub rfq_id: String }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AwardRfqInput { pub rfq_id: String, pub supplier_id: String, #[serde(default = "dactor")] pub actor: String }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AuditFindingInput { #[serde(default = "dminor")] pub severity: String, #[serde(default)] pub clause: String, pub description: String }
fn dminor() -> String { "minor".into() }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RecordAuditInput { pub supplier_id: String, #[serde(default = "dprocess")] pub audit_type: String, #[serde(default = "dactor")] pub auditor: String, pub conducted_on: Option<String>, #[serde(default)] pub findings: Vec<AuditFindingInput>, #[serde(default = "dactor")] pub actor: String }
fn dprocess() -> String { "process".into() }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RaiseScarInput { pub supplier_id: String, pub title: String, #[serde(default = "dmajor")] pub severity: String, pub audit_id: Option<String>, pub due_date: Option<String>, #[serde(default = "dactor")] pub actor: String }
fn dmajor() -> String { "major".into() }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateScarInput { pub scar_id: String, pub status: Option<ScarStatus>, pub root_cause: Option<String>, pub corrective_action: Option<String>, #[serde(default = "dactor")] pub actor: String }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ScarsForInput { pub supplier_id: String, #[serde(default)] pub open_only: bool }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RecordEventInput {
    pub supplier_id: String,
    pub item_id: Option<String>,
    pub po_id: Option<String>,
    #[serde(default = "dreceipt")] pub kind: String,
    pub qty: u32,
    #[serde(default)] pub defect_qty: u32,
    #[serde(default = "dtrue")] pub on_time: bool,
    #[serde(default)] pub note: String,
    pub occurred_on: Option<String>,
    #[serde(default = "dactor")] pub actor: String,
}
fn dreceipt() -> String { "receipt".into() }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AssessRiskInput { pub supplier_id: String, #[serde(default = "dgen")] pub category: String, pub likelihood: u8, pub impact: u8, #[serde(default)] pub note: String, pub assessed_on: Option<String>, #[serde(default = "dactor")] pub actor: String }
fn dgen() -> String { "general".into() }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MonitorRisksInput { #[serde(default = "dthree")] pub min_level: u8 }
fn dthree() -> u8 { 3 }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AuditLogInput { #[serde(default = "dfifty")] pub limit: usize }
fn dfifty() -> usize { 50 }

// ─── server ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SupplierServer { pub store: Arc<SupplierStore> }

#[tool_router]
impl SupplierServer {
    // suppliers
    #[tool(description = "Register a new supplier (starts as an unqualified prospect, not yet approved for POs).")]
    fn create_supplier(&self, Parameters(i): Parameters<CreateSupplierInput>) -> String {
        let s = self.store.create_supplier(&i.name, &i.category, &i.country, i.lead_time_days, &i.actor);
        serde_json::to_string_pretty(&s).unwrap()
    }

    #[tool(description = "Get a supplier by id.")]
    fn get_supplier(&self, Parameters(i): Parameters<SupplierIdInput>) -> String {
        match self.store.get_supplier(&i.supplier_id) {
            Some(s) => serde_json::to_string_pretty(&s).unwrap(), None => format!("Supplier not found: {}", i.supplier_id) }
    }

    #[tool(description = "List suppliers, optionally filtered by category and/or status.")]
    fn list_suppliers(&self, Parameters(i): Parameters<ListSuppliersInput>) -> String {
        let v = self.store.list_suppliers(i.category.as_deref(), i.status);
        serde_json::to_string_pretty(&serde_json::json!({"count": v.len(), "suppliers": v})).unwrap()
    }

    #[tool(description = "Set a supplier's lifecycle status (prospect/active/on_hold/suspended/disqualified). Suspending or disqualifying revokes PO approval. Gated.")]
    fn set_supplier_status(&self, Parameters(i): Parameters<SetStatusInput>) -> String {
        match self.store.set_supplier_status(&i.supplier_id, i.status, &i.actor) {
            Ok(s) => serde_json::to_string_pretty(&s).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "Set a supplier's qualification status. Becoming qualified/conditionally-qualified approves the supplier for POs. Gated.")]
    fn set_qualification(&self, Parameters(i): Parameters<SetQualificationInput>) -> String {
        match self.store.set_qualification(&i.supplier_id, i.qualification, &i.actor) {
            Ok(s) => serde_json::to_string_pretty(&s).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "Add a contact to a supplier.")]
    fn add_contact(&self, Parameters(i): Parameters<AddContactInput>) -> String {
        match self.store.add_contact(&i.supplier_id, &i.name, &i.role, &i.email, i.phone, i.primary, &i.actor) {
            Ok(c) => serde_json::to_string_pretty(&c).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "List a supplier's contacts.")]
    fn list_contacts(&self, Parameters(i): Parameters<SupplierIdInput>) -> String {
        let v = self.store.contacts_for(&i.supplier_id);
        serde_json::to_string_pretty(&serde_json::json!({"count": v.len(), "contacts": v})).unwrap()
    }

    // certifications
    #[tool(description = "Add a certification (e.g. ISO 9001, IATF 16949, ISO 22000) to a supplier.")]
    fn add_certification(&self, Parameters(i): Parameters<AddCertInput>) -> String {
        let (Some(issued), Some(expires)) = (date(&Some(i.issued_on.clone())), date(&Some(i.expires_on.clone()))) else {
            return "Error: issued_on and expires_on must be YYYY-MM-DD".into();
        };
        match self.store.add_certification(&i.supplier_id, &i.standard, &i.certificate_number, &i.issued_by, issued, expires, &i.actor) {
            Ok(c) => serde_json::to_string_pretty(&c).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "List a supplier's certifications (soonest expiry first).")]
    fn list_certifications(&self, Parameters(i): Parameters<SupplierIdInput>) -> String {
        let v = self.store.certifications_for(&i.supplier_id);
        serde_json::to_string_pretty(&serde_json::json!({"count": v.len(), "certifications": v})).unwrap()
    }

    #[tool(description = "List certifications expiring within N days (default 90) across all suppliers — compliance watchlist.")]
    fn expiring_certifications(&self, Parameters(i): Parameters<ExpiringCertsInput>) -> String {
        let v = self.store.expiring_certifications(i.within_days);
        serde_json::to_string_pretty(&serde_json::json!({"within_days": i.within_days, "count": v.len(), "certifications": v})).unwrap()
    }

    // catalog & pricing
    #[tool(description = "Create a catalog item (an internal part/SKU sources can offer).")]
    fn create_item(&self, Parameters(i): Parameters<CreateItemInput>) -> String {
        let it = self.store.create_item(&i.sku, &i.name, &i.category, &i.unit, &i.actor);
        serde_json::to_string_pretty(&it).unwrap()
    }

    #[tool(description = "List catalog items, optionally by category.")]
    fn list_items(&self, Parameters(i): Parameters<ListItemsInput>) -> String {
        let v = self.store.list_items(i.category.as_deref());
        serde_json::to_string_pretty(&serde_json::json!({"count": v.len(), "items": v})).unwrap()
    }

    #[tool(description = "Set/replace a supplier's offer (price, MOQ, lead time, availability) for a catalog item.")]
    fn set_supplier_item(&self, Parameters(i): Parameters<SetSupplierItemInput>) -> String {
        match self.store.set_supplier_item(&i.supplier_id, &i.item_id, &i.supplier_sku, i.unit_price, &i.currency, i.min_order_qty, i.lead_time_days, i.available_qty, i.preferred, &i.actor) {
            Ok(si) => serde_json::to_string_pretty(&si).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "Find and rank supplier sources for an item by price/availability/lead time. qualified_only (default true) restricts to PO-approved suppliers. Powers replenishment, shortage-resolution, and cost optimization.")]
    fn find_item_sources(&self, Parameters(i): Parameters<FindSourcesInput>) -> String {
        serde_json::to_string_pretty(&self.store.find_item_sources(&i.item_id, i.required_qty, i.qualified_only)).unwrap()
    }

    // purchase orders
    #[tool(description = "Create and issue a purchase order. Refused if the supplier is not approved for POs. External write — gated.")]
    fn create_po(&self, Parameters(i): Parameters<CreatePoInput>) -> String {
        let lines: Vec<(String, String, u32, f64)> = i.lines.into_iter().map(|l| (l.item_id, l.description, l.qty, l.unit_price)).collect();
        match self.store.create_po(&i.supplier_id, &i.currency, lines, date(&i.need_by), &i.actor) {
            Ok(po) => serde_json::to_string_pretty(&po).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "Get a purchase order by id.")]
    fn get_po(&self, Parameters(i): Parameters<PoIdInput>) -> String {
        match self.store.get_po(&i.po_id) {
            Some(po) => serde_json::to_string_pretty(&po).unwrap(), None => format!("PO not found: {}", i.po_id) }
    }

    #[tool(description = "List purchase orders, optionally by supplier and/or status.")]
    fn list_pos(&self, Parameters(i): Parameters<ListPosInput>) -> String {
        let v = self.store.list_pos(i.supplier_id.as_deref(), i.status);
        serde_json::to_string_pretty(&serde_json::json!({"count": v.len(), "purchase_orders": v})).unwrap()
    }

    #[tool(description = "Record receipt of PO lines (updates received quantities and PO status).")]
    fn receive_po(&self, Parameters(i): Parameters<ReceivePoInput>) -> String {
        let receipts: Vec<(String, u32)> = i.receipts.into_iter().map(|r| (r.item_id, r.qty)).collect();
        match self.store.receive_po(&i.po_id, receipts, &i.actor) {
            Ok(po) => serde_json::to_string_pretty(&po).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "Cancel a purchase order (not allowed once fully received). Gated.")]
    fn cancel_po(&self, Parameters(i): Parameters<CancelPoInput>) -> String {
        match self.store.cancel_po(&i.po_id, &i.reason, &i.actor) {
            Ok(po) => serde_json::to_string_pretty(&po).unwrap(), Err(e) => format!("Error: {e}") }
    }

    // RFQ / sourcing
    #[tool(description = "Open an RFQ (request for quote) for an item, inviting suppliers.")]
    fn create_rfq(&self, Parameters(i): Parameters<CreateRfqInput>) -> String {
        match self.store.create_rfq(&i.item_id, i.qty, i.invited_suppliers, date(&i.need_by), &i.actor) {
            Ok(r) => serde_json::to_string_pretty(&r).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "Submit a supplier quote against an open RFQ.")]
    fn submit_quote(&self, Parameters(i): Parameters<SubmitQuoteInput>) -> String {
        match self.store.submit_quote(&i.rfq_id, &i.supplier_id, i.unit_price, &i.currency, i.lead_time_days, date(&i.valid_until), &i.actor) {
            Ok(q) => serde_json::to_string_pretty(&q).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "Compare quotes for an RFQ, cheapest first.")]
    fn compare_quotes(&self, Parameters(i): Parameters<RfqIdInput>) -> String {
        let v = self.store.compare_quotes(&i.rfq_id);
        serde_json::to_string_pretty(&serde_json::json!({"rfq_id": i.rfq_id, "count": v.len(), "quotes": v})).unwrap()
    }

    #[tool(description = "Award an RFQ to a supplier (closes the RFQ). Gated.")]
    fn award_rfq(&self, Parameters(i): Parameters<AwardRfqInput>) -> String {
        match self.store.award_rfq(&i.rfq_id, &i.supplier_id, &i.actor) {
            Ok(r) => serde_json::to_string_pretty(&r).unwrap(), Err(e) => format!("Error: {e}") }
    }

    // quality
    #[tool(description = "Record a supplier quality audit with findings; auto-scores, sets pass/conditional/fail, and auto-raises a SCAR on failure.")]
    fn record_audit(&self, Parameters(i): Parameters<RecordAuditInput>) -> String {
        let findings: Vec<AuditFinding> = i.findings.into_iter().map(|f| AuditFinding { severity: f.severity, clause: f.clause, description: f.description }).collect();
        let when = date(&i.conducted_on).unwrap_or_else(today);
        match self.store.record_audit(&i.supplier_id, &i.audit_type, &i.auditor, when, findings, &i.actor) {
            Ok(a) => serde_json::to_string_pretty(&a).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "List a supplier's quality audits (most recent first).")]
    fn list_audits(&self, Parameters(i): Parameters<SupplierIdInput>) -> String {
        let v = self.store.audits_for(&i.supplier_id);
        serde_json::to_string_pretty(&serde_json::json!({"count": v.len(), "audits": v})).unwrap()
    }

    #[tool(description = "Raise a Supplier Corrective Action Request (SCAR).")]
    fn raise_scar(&self, Parameters(i): Parameters<RaiseScarInput>) -> String {
        match self.store.raise_scar(&i.supplier_id, &i.title, &i.severity, i.audit_id, date(&i.due_date), &i.actor) {
            Ok(s) => serde_json::to_string_pretty(&s).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "Update a SCAR (status, root cause, corrective action). Closing stamps closed_at.")]
    fn update_scar(&self, Parameters(i): Parameters<UpdateScarInput>) -> String {
        match self.store.update_scar(&i.scar_id, i.status, i.root_cause, i.corrective_action, &i.actor) {
            Ok(s) => serde_json::to_string_pretty(&s).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "List a supplier's SCARs (set open_only=true to exclude closed).")]
    fn list_scars(&self, Parameters(i): Parameters<ScarsForInput>) -> String {
        let v = self.store.scars_for(&i.supplier_id, i.open_only);
        serde_json::to_string_pretty(&serde_json::json!({"count": v.len(), "scars": v})).unwrap()
    }

    #[tool(description = "Record a quality/delivery event (receipt/nonconformance/return/complaint) — feeds the performance scorecard.")]
    fn record_quality_event(&self, Parameters(i): Parameters<RecordEventInput>) -> String {
        let when = date(&i.occurred_on).unwrap_or_else(today);
        match self.store.record_quality_event(&i.supplier_id, i.item_id, i.po_id, &i.kind, i.qty, i.defect_qty, i.on_time, &i.note, when, &i.actor) {
            Ok(e) => serde_json::to_string_pretty(&e).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "Performance scorecard: on-time delivery, quality rate, defect PPM, open SCARs, latest audit score, composite rating (A–D).")]
    fn scorecard(&self, Parameters(i): Parameters<SupplierIdInput>) -> String {
        match self.store.scorecard(&i.supplier_id) {
            Some(v) => serde_json::to_string_pretty(&v).unwrap(), None => format!("Supplier not found: {}", i.supplier_id) }
    }

    // risk
    #[tool(description = "Record a risk assessment (likelihood 1–5 × impact 1–5) for a supplier.")]
    fn assess_risk(&self, Parameters(i): Parameters<AssessRiskInput>) -> String {
        let when = date(&i.assessed_on).unwrap_or_else(today);
        match self.store.assess_risk(&i.supplier_id, &i.category, i.likelihood, i.impact, &i.note, when, &i.actor) {
            Ok(r) => serde_json::to_string_pretty(&r).unwrap(), Err(e) => format!("Error: {e}") }
    }

    #[tool(description = "Risk profile for a supplier: composite level from assessments plus signals (qualification, expired certs, open SCARs, single-source exposure).")]
    fn risk_profile(&self, Parameters(i): Parameters<SupplierIdInput>) -> String {
        match self.store.risk_profile(&i.supplier_id) {
            Some(v) => serde_json::to_string_pretty(&v).unwrap(), None => format!("Supplier not found: {}", i.supplier_id) }
    }

    #[tool(description = "Portfolio risk monitor: all suppliers at/above a minimum risk level (default 3=medium), most severe first. Powers the Supplier Risk Monitor.")]
    fn monitor_risks(&self, Parameters(i): Parameters<MonitorRisksInput>) -> String {
        serde_json::to_string_pretty(&self.store.monitor_risks(i.min_level)).unwrap()
    }

    #[tool(description = "Recent audit-trail entries across the platform (most recent first).")]
    fn audit_log(&self, Parameters(i): Parameters<AuditLogInput>) -> String {
        let v = self.store.audit_log(i.limit);
        serde_json::to_string_pretty(&serde_json::json!({"count": v.len(), "entries": v})).unwrap()
    }
}

#[async_trait::async_trait]
impl HealthCheck for SupplierServer {
    async fn check_health(&self) -> HealthStatus {
        HealthStatus { healthy: true, message: Some("operational".into()), latency_ms: Some(1) }
    }
}

adk_mcp_sdk::mcp_2026_server! {
    server: SupplierServer,
    task_tools: [],
    approval_tools: ["set_supplier_status", "set_qualification", "create_po", "cancel_po", "award_rfq"],
    cache_ttl_ms: 60_000,
}
