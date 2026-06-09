//! Supplier Relationship Management (SRM) domain model.
//!
//! Broad procurement/supply-base platform: suppliers and contacts, certifications
//! and qualification status, a product catalog with multi-supplier pricing,
//! purchase orders, RFQ/sourcing with quotes, quality audits + findings + SCARs
//! (Supplier Corrective Action Requests), performance scorecards, and supply-risk
//! assessments. The named agents (quality audit, risk monitor, replenishment,
//! cost optimizer, shortage resolver) are clients of this platform.

use chrono::{DateTime, NaiveDate, Utc};
use rmcp::schemars;
use serde::{Deserialize, Serialize};

// ─── suppliers ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SupplierStatus {
    Prospect,
    Active,
    OnHold,
    Suspended,
    Disqualified,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualificationStatus {
    Unqualified,
    InQualification,
    Qualified,
    ConditionallyQualified,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Supplier {
    pub id: String,
    pub name: String,
    pub category: String,
    pub country: String,
    pub status: SupplierStatus,
    pub qualification: QualificationStatus,
    /// Whether the supplier is approved to receive purchase orders.
    pub approved_for_po: bool,
    pub default_lead_time_days: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Contact {
    pub id: String,
    pub supplier_id: String,
    pub name: String,
    pub role: String,
    pub email: String,
    pub phone: Option<String>,
    pub primary: bool,
}

// ─── certifications ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Certification {
    pub id: String,
    pub supplier_id: String,
    /// e.g. ISO 9001, IATF 16949, ISO 22000, HACCP, ISO 14001.
    pub standard: String,
    pub certificate_number: String,
    pub issued_by: String,
    pub issued_on: NaiveDate,
    pub expires_on: NaiveDate,
}

// ─── catalog & pricing ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CatalogItem {
    pub id: String,
    pub sku: String,
    pub name: String,
    pub category: String,
    pub unit: String,
}

/// A supplier's offer for a catalog item — the multi-source pricing record.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SupplierItem {
    pub id: String,
    pub supplier_id: String,
    pub item_id: String,
    pub supplier_sku: String,
    pub unit_price: f64,
    pub currency: String,
    pub min_order_qty: u32,
    pub lead_time_days: u32,
    pub available_qty: u32,
    pub preferred: bool,
}

// ─── purchase orders ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PoStatus {
    Draft,
    Issued,
    Acknowledged,
    PartiallyReceived,
    Received,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PoLine {
    pub item_id: String,
    pub description: String,
    pub qty: u32,
    pub unit_price: f64,
    pub received_qty: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PurchaseOrder {
    pub id: String,
    pub supplier_id: String,
    pub status: PoStatus,
    pub currency: String,
    pub lines: Vec<PoLine>,
    pub total: f64,
    pub need_by: Option<NaiveDate>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ─── RFQ / sourcing ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RfqStatus {
    Open,
    Awarded,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Rfq {
    pub id: String,
    pub item_id: String,
    pub qty: u32,
    pub status: RfqStatus,
    pub need_by: Option<NaiveDate>,
    pub invited_suppliers: Vec<String>,
    pub awarded_supplier_id: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Quote {
    pub id: String,
    pub rfq_id: String,
    pub supplier_id: String,
    pub unit_price: f64,
    pub currency: String,
    pub lead_time_days: u32,
    pub valid_until: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
}

// ─── quality: audits, findings, SCARs ──────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditResult {
    Pass,
    ConditionalPass,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AuditFinding {
    pub severity: String, // minor | major | critical
    pub clause: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct QualityAudit {
    pub id: String,
    pub supplier_id: String,
    pub audit_type: String, // process | system | product | on-site | desk
    pub auditor: String,
    pub conducted_on: NaiveDate,
    pub score: f64, // 0..100
    pub result: AuditResult,
    pub findings: Vec<AuditFinding>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScarStatus {
    Open,
    ContainmentProvided,
    RootCauseAccepted,
    Closed,
    Escalated,
}

/// Supplier Corrective Action Request — issued against a quality problem.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Scar {
    pub id: String,
    pub supplier_id: String,
    pub audit_id: Option<String>,
    pub title: String,
    pub severity: String,
    pub status: ScarStatus,
    pub root_cause: Option<String>,
    pub corrective_action: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

// ─── quality events / incoming inspection ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct QualityEvent {
    pub id: String,
    pub supplier_id: String,
    pub item_id: Option<String>,
    pub po_id: Option<String>,
    /// receipt | nonconformance | return | complaint
    pub kind: String,
    pub qty: u32,
    pub defect_qty: u32,
    pub on_time: bool,
    pub note: String,
    pub occurred_on: NaiveDate,
    pub created_at: DateTime<Utc>,
}

// ─── risk ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RiskAssessment {
    pub id: String,
    pub supplier_id: String,
    /// financial | geographic | capacity | compliance | single-source | cyber
    pub category: String,
    pub likelihood: u8, // 1..5
    pub impact: u8,     // 1..5
    pub note: String,
    pub assessed_by: String,
    pub assessed_on: NaiveDate,
    pub created_at: DateTime<Utc>,
}

// ─── audit trail ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AuditEntry {
    pub at: DateTime<Utc>,
    pub actor: String,
    pub action: String,
    pub detail: String,
}
