//! In-memory SRM store with seeded data and derived analytics.
//!
//! Thread-safe via per-collection `Mutex`. IDs come from a monotonic sequence
//! (`PREFIX-{n}` from 1000). Every state-changing op appends to an audit trail.
//! Derived analytics (scorecards, risk scoring, sourcing) are computed on read.

use crate::types::*;
use chrono::{Duration, NaiveDate, Utc};
use std::collections::HashMap;
use std::sync::Mutex;

pub struct SupplierStore {
    suppliers: Mutex<HashMap<String, Supplier>>,
    contacts: Mutex<HashMap<String, Contact>>,
    certs: Mutex<HashMap<String, Certification>>,
    items: Mutex<HashMap<String, CatalogItem>>,
    supplier_items: Mutex<HashMap<String, SupplierItem>>,
    pos: Mutex<HashMap<String, PurchaseOrder>>,
    rfqs: Mutex<HashMap<String, Rfq>>,
    quotes: Mutex<HashMap<String, Quote>>,
    audits: Mutex<HashMap<String, QualityAudit>>,
    scars: Mutex<HashMap<String, Scar>>,
    events: Mutex<Vec<QualityEvent>>,
    risks: Mutex<HashMap<String, RiskAssessment>>,
    audit_log: Mutex<Vec<AuditEntry>>,
    seq: Mutex<u64>,
}

impl Default for SupplierStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SupplierStore {
    pub fn new() -> Self {
        let s = SupplierStore {
            suppliers: Mutex::new(HashMap::new()),
            contacts: Mutex::new(HashMap::new()),
            certs: Mutex::new(HashMap::new()),
            items: Mutex::new(HashMap::new()),
            supplier_items: Mutex::new(HashMap::new()),
            pos: Mutex::new(HashMap::new()),
            rfqs: Mutex::new(HashMap::new()),
            quotes: Mutex::new(HashMap::new()),
            audits: Mutex::new(HashMap::new()),
            scars: Mutex::new(HashMap::new()),
            events: Mutex::new(Vec::new()),
            risks: Mutex::new(HashMap::new()),
            audit_log: Mutex::new(Vec::new()),
            seq: Mutex::new(1000),
        };
        s.seed();
        s
    }

    fn next(&self, prefix: &str) -> String {
        let mut n = self.seq.lock().unwrap();
        *n += 1;
        format!("{prefix}-{n}")
    }

    fn audit(&self, actor: &str, action: &str, detail: impl Into<String>) {
        self.audit_log.lock().unwrap().push(AuditEntry {
            at: Utc::now(),
            actor: actor.to_string(),
            action: action.to_string(),
            detail: detail.into(),
        });
    }

    pub fn supplier_exists(&self, id: &str) -> bool {
        self.suppliers.lock().unwrap().contains_key(id)
    }

    // ─── suppliers ───────────────────────────────────────────────────────

    pub fn create_supplier(&self, name: &str, category: &str, country: &str, lead_time_days: u32, actor: &str) -> Supplier {
        let now = Utc::now();
        let s = Supplier {
            id: self.next("SUP"),
            name: name.to_string(),
            category: category.to_string(),
            country: country.to_string(),
            status: SupplierStatus::Prospect,
            qualification: QualificationStatus::Unqualified,
            approved_for_po: false,
            default_lead_time_days: lead_time_days,
            created_at: now,
            updated_at: now,
        };
        self.suppliers.lock().unwrap().insert(s.id.clone(), s.clone());
        self.audit(actor, "create_supplier", s.id.clone());
        s
    }

    pub fn get_supplier(&self, id: &str) -> Option<Supplier> {
        self.suppliers.lock().unwrap().get(id).cloned()
    }

    pub fn list_suppliers(&self, category: Option<&str>, status: Option<SupplierStatus>) -> Vec<Supplier> {
        let mut v: Vec<Supplier> = self.suppliers.lock().unwrap().values()
            .filter(|s| category.is_none_or(|c| s.category.eq_ignore_ascii_case(c)))
            .filter(|s| status.is_none_or(|st| s.status == st))
            .cloned().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    pub fn set_supplier_status(&self, id: &str, status: SupplierStatus, actor: &str) -> Result<Supplier, String> {
        let mut sup = self.suppliers.lock().unwrap();
        let s = sup.get_mut(id).ok_or_else(|| format!("Supplier not found: {id}"))?;
        s.status = status;
        // Suspended/disqualified/on-hold suppliers cannot receive new POs.
        if matches!(status, SupplierStatus::Suspended | SupplierStatus::Disqualified | SupplierStatus::OnHold) {
            s.approved_for_po = false;
        }
        s.updated_at = Utc::now();
        let out = s.clone();
        drop(sup);
        self.audit(actor, "set_supplier_status", format!("{id} -> {status:?}"));
        Ok(out)
    }

    /// Set qualification status; becoming Qualified approves the supplier for POs.
    pub fn set_qualification(&self, id: &str, q: QualificationStatus, actor: &str) -> Result<Supplier, String> {
        let mut sup = self.suppliers.lock().unwrap();
        let s = sup.get_mut(id).ok_or_else(|| format!("Supplier not found: {id}"))?;
        s.qualification = q;
        s.approved_for_po = matches!(q, QualificationStatus::Qualified | QualificationStatus::ConditionallyQualified)
            && matches!(s.status, SupplierStatus::Active | SupplierStatus::Prospect);
        if s.approved_for_po && s.status == SupplierStatus::Prospect {
            s.status = SupplierStatus::Active;
        }
        s.updated_at = Utc::now();
        let out = s.clone();
        drop(sup);
        self.audit(actor, "set_qualification", format!("{id} -> {q:?}"));
        Ok(out)
    }

    pub fn add_contact(&self, supplier_id: &str, name: &str, role: &str, email: &str, phone: Option<String>, primary: bool, actor: &str) -> Result<Contact, String> {
        if !self.supplier_exists(supplier_id) { return Err(format!("Supplier not found: {supplier_id}")); }
        let c = Contact { id: self.next("CON"), supplier_id: supplier_id.to_string(), name: name.to_string(), role: role.to_string(), email: email.to_string(), phone, primary };
        self.contacts.lock().unwrap().insert(c.id.clone(), c.clone());
        self.audit(actor, "add_contact", c.id.clone());
        Ok(c)
    }

    pub fn contacts_for(&self, supplier_id: &str) -> Vec<Contact> {
        self.contacts.lock().unwrap().values().filter(|c| c.supplier_id == supplier_id).cloned().collect()
    }

    // ─── certifications ──────────────────────────────────────────────────

    pub fn add_certification(&self, supplier_id: &str, standard: &str, cert_no: &str, issued_by: &str, issued_on: NaiveDate, expires_on: NaiveDate, actor: &str) -> Result<Certification, String> {
        if !self.supplier_exists(supplier_id) { return Err(format!("Supplier not found: {supplier_id}")); }
        let c = Certification { id: self.next("CRT"), supplier_id: supplier_id.to_string(), standard: standard.to_string(), certificate_number: cert_no.to_string(), issued_by: issued_by.to_string(), issued_on, expires_on };
        self.certs.lock().unwrap().insert(c.id.clone(), c.clone());
        self.audit(actor, "add_certification", format!("{supplier_id} {standard}"));
        Ok(c)
    }

    pub fn certifications_for(&self, supplier_id: &str) -> Vec<Certification> {
        let mut v: Vec<Certification> = self.certs.lock().unwrap().values().filter(|c| c.supplier_id == supplier_id).cloned().collect();
        v.sort_by(|a, b| a.expires_on.cmp(&b.expires_on));
        v
    }

    /// Certifications expiring on/before the horizon (default 90d), oldest first.
    pub fn expiring_certifications(&self, within_days: i64) -> Vec<Certification> {
        let cutoff = Utc::now().date_naive() + Duration::days(within_days);
        let mut v: Vec<Certification> = self.certs.lock().unwrap().values().filter(|c| c.expires_on <= cutoff).cloned().collect();
        v.sort_by(|a, b| a.expires_on.cmp(&b.expires_on));
        v
    }

    // ─── catalog & pricing ─────────────────────────────────────────────────

    pub fn create_item(&self, sku: &str, name: &str, category: &str, unit: &str, actor: &str) -> CatalogItem {
        let it = CatalogItem { id: self.next("ITM"), sku: sku.to_string(), name: name.to_string(), category: category.to_string(), unit: unit.to_string() };
        self.items.lock().unwrap().insert(it.id.clone(), it.clone());
        self.audit(actor, "create_item", it.id.clone());
        it
    }

    pub fn list_items(&self, category: Option<&str>) -> Vec<CatalogItem> {
        let mut v: Vec<CatalogItem> = self.items.lock().unwrap().values().filter(|i| category.is_none_or(|c| i.category.eq_ignore_ascii_case(c))).cloned().collect();
        v.sort_by(|a, b| a.sku.cmp(&b.sku));
        v
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_supplier_item(&self, supplier_id: &str, item_id: &str, supplier_sku: &str, unit_price: f64, currency: &str, min_order_qty: u32, lead_time_days: u32, available_qty: u32, preferred: bool, actor: &str) -> Result<SupplierItem, String> {
        if !self.supplier_exists(supplier_id) { return Err(format!("Supplier not found: {supplier_id}")); }
        if !self.items.lock().unwrap().contains_key(item_id) { return Err(format!("Item not found: {item_id}")); }
        let si = SupplierItem { id: self.next("SIT"), supplier_id: supplier_id.to_string(), item_id: item_id.to_string(), supplier_sku: supplier_sku.to_string(), unit_price, currency: currency.to_string(), min_order_qty, lead_time_days, available_qty, preferred };
        self.supplier_items.lock().unwrap().insert(si.id.clone(), si.clone());
        self.audit(actor, "set_supplier_item", format!("{supplier_id}/{item_id}"));
        Ok(si)
    }

    /// Source an item: rank supplier offers by price, honoring availability and
    /// PO-eligibility. Used by replenishment, shortage-resolver and cost-optimizer.
    pub fn find_item_sources(&self, item_id: &str, required_qty: u32, qualified_only: bool) -> serde_json::Value {
        let sis = self.supplier_items.lock().unwrap();
        let sups = self.suppliers.lock().unwrap();
        let mut offers: Vec<serde_json::Value> = sis.values()
            .filter(|si| si.item_id == item_id)
            .filter_map(|si| {
                let sup = sups.get(&si.supplier_id)?;
                if qualified_only && !sup.approved_for_po { return None; }
                let meets_qty = si.available_qty >= required_qty && required_qty >= si.min_order_qty;
                Some((si.clone(), sup.clone(), meets_qty))
            })
            .map(|(si, sup, meets_qty)| serde_json::json!({
                "supplier_id": si.supplier_id,
                "supplier_name": sup.name,
                "supplier_sku": si.supplier_sku,
                "unit_price": si.unit_price,
                "currency": si.currency,
                "available_qty": si.available_qty,
                "min_order_qty": si.min_order_qty,
                "lead_time_days": si.lead_time_days,
                "preferred": si.preferred,
                "meets_requirement": meets_qty,
                "approved_for_po": sup.approved_for_po,
                "extended_price": (si.unit_price * required_qty as f64 * 100.0).round() / 100.0,
            }))
            .collect();
        // Best first: meets requirement, then lower price, then shorter lead time.
        offers.sort_by(|a, b| {
            b["meets_requirement"].as_bool().cmp(&a["meets_requirement"].as_bool())
                .then(a["unit_price"].as_f64().partial_cmp(&b["unit_price"].as_f64()).unwrap())
                .then(a["lead_time_days"].as_u64().cmp(&b["lead_time_days"].as_u64()))
        });
        let best = offers.first().cloned();
        serde_json::json!({"item_id": item_id, "required_qty": required_qty, "source_count": offers.len(), "recommended": best, "sources": offers})
    }

    // ─── purchase orders ───────────────────────────────────────────────────

    /// Create a PO. Refuses suppliers not approved for PO (gate with real weight).
    pub fn create_po(&self, supplier_id: &str, currency: &str, lines: Vec<(String, String, u32, f64)>, need_by: Option<NaiveDate>, actor: &str) -> Result<PurchaseOrder, String> {
        let sup = self.get_supplier(supplier_id).ok_or_else(|| format!("Supplier not found: {supplier_id}"))?;
        if !sup.approved_for_po {
            return Err(format!("Supplier {supplier_id} is not approved for purchase orders (status {:?}, qualification {:?})", sup.status, sup.qualification));
        }
        if lines.is_empty() { return Err("PO must have at least one line".into()); }
        let po_lines: Vec<PoLine> = lines.into_iter().map(|(item_id, description, qty, unit_price)| PoLine { item_id, description, qty, unit_price, received_qty: 0 }).collect();
        let total = (po_lines.iter().map(|l| l.qty as f64 * l.unit_price).sum::<f64>() * 100.0).round() / 100.0;
        let now = Utc::now();
        let po = PurchaseOrder { id: self.next("PO"), supplier_id: supplier_id.to_string(), status: PoStatus::Issued, currency: currency.to_string(), lines: po_lines, total, need_by, created_by: actor.to_string(), created_at: now, updated_at: now };
        self.pos.lock().unwrap().insert(po.id.clone(), po.clone());
        self.audit(actor, "create_po", format!("{} {} {}", po.id, supplier_id, total));
        Ok(po)
    }

    pub fn get_po(&self, id: &str) -> Option<PurchaseOrder> {
        self.pos.lock().unwrap().get(id).cloned()
    }

    pub fn list_pos(&self, supplier_id: Option<&str>, status: Option<PoStatus>) -> Vec<PurchaseOrder> {
        let mut v: Vec<PurchaseOrder> = self.pos.lock().unwrap().values()
            .filter(|p| supplier_id.is_none_or(|s| p.supplier_id == s))
            .filter(|p| status.is_none_or(|st| p.status == st))
            .cloned().collect();
        v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        v
    }

    pub fn receive_po(&self, po_id: &str, receipts: Vec<(String, u32)>, actor: &str) -> Result<PurchaseOrder, String> {
        let mut pos = self.pos.lock().unwrap();
        let po = pos.get_mut(po_id).ok_or_else(|| format!("PO not found: {po_id}"))?;
        if matches!(po.status, PoStatus::Cancelled | PoStatus::Received) {
            return Err(format!("PO {po_id} is {:?}; cannot receive", po.status));
        }
        for (item_id, qty) in receipts {
            if let Some(line) = po.lines.iter_mut().find(|l| l.item_id == item_id) {
                line.received_qty = (line.received_qty + qty).min(line.qty);
            }
        }
        let all = po.lines.iter().all(|l| l.received_qty >= l.qty);
        let any = po.lines.iter().any(|l| l.received_qty > 0);
        po.status = if all { PoStatus::Received } else if any { PoStatus::PartiallyReceived } else { po.status };
        po.updated_at = Utc::now();
        let out = po.clone();
        drop(pos);
        self.audit(actor, "receive_po", format!("{po_id} -> {:?}", out.status));
        Ok(out)
    }

    pub fn cancel_po(&self, po_id: &str, reason: &str, actor: &str) -> Result<PurchaseOrder, String> {
        let mut pos = self.pos.lock().unwrap();
        let po = pos.get_mut(po_id).ok_or_else(|| format!("PO not found: {po_id}"))?;
        if matches!(po.status, PoStatus::Received) {
            return Err(format!("PO {po_id} already received; cannot cancel"));
        }
        po.status = PoStatus::Cancelled;
        po.updated_at = Utc::now();
        let out = po.clone();
        drop(pos);
        self.audit(actor, "cancel_po", format!("{po_id}: {reason}"));
        Ok(out)
    }

    // ─── RFQ / sourcing ──────────────────────────────────────────────────

    pub fn create_rfq(&self, item_id: &str, qty: u32, invited: Vec<String>, need_by: Option<NaiveDate>, actor: &str) -> Result<Rfq, String> {
        if !self.items.lock().unwrap().contains_key(item_id) { return Err(format!("Item not found: {item_id}")); }
        let r = Rfq { id: self.next("RFQ"), item_id: item_id.to_string(), qty, status: RfqStatus::Open, need_by, invited_suppliers: invited, awarded_supplier_id: None, created_by: actor.to_string(), created_at: Utc::now() };
        self.rfqs.lock().unwrap().insert(r.id.clone(), r.clone());
        self.audit(actor, "create_rfq", r.id.clone());
        Ok(r)
    }

    pub fn submit_quote(&self, rfq_id: &str, supplier_id: &str, unit_price: f64, currency: &str, lead_time_days: u32, valid_until: Option<NaiveDate>, actor: &str) -> Result<Quote, String> {
        if !self.rfqs.lock().unwrap().contains_key(rfq_id) { return Err(format!("RFQ not found: {rfq_id}")); }
        if !self.supplier_exists(supplier_id) { return Err(format!("Supplier not found: {supplier_id}")); }
        let q = Quote { id: self.next("QUO"), rfq_id: rfq_id.to_string(), supplier_id: supplier_id.to_string(), unit_price, currency: currency.to_string(), lead_time_days, valid_until, created_at: Utc::now() };
        self.quotes.lock().unwrap().insert(q.id.clone(), q.clone());
        self.audit(actor, "submit_quote", format!("{rfq_id} {supplier_id}"));
        Ok(q)
    }

    /// Quotes for an RFQ ranked cheapest first.
    pub fn compare_quotes(&self, rfq_id: &str) -> Vec<Quote> {
        let mut v: Vec<Quote> = self.quotes.lock().unwrap().values().filter(|q| q.rfq_id == rfq_id).cloned().collect();
        v.sort_by(|a, b| a.unit_price.partial_cmp(&b.unit_price).unwrap().then(a.lead_time_days.cmp(&b.lead_time_days)));
        v
    }

    pub fn award_rfq(&self, rfq_id: &str, supplier_id: &str, actor: &str) -> Result<Rfq, String> {
        if !self.supplier_exists(supplier_id) { return Err(format!("Supplier not found: {supplier_id}")); }
        let mut rfqs = self.rfqs.lock().unwrap();
        let r = rfqs.get_mut(rfq_id).ok_or_else(|| format!("RFQ not found: {rfq_id}"))?;
        if r.status != RfqStatus::Open { return Err(format!("RFQ {rfq_id} is {:?}", r.status)); }
        r.status = RfqStatus::Awarded;
        r.awarded_supplier_id = Some(supplier_id.to_string());
        let out = r.clone();
        drop(rfqs);
        self.audit(actor, "award_rfq", format!("{rfq_id} -> {supplier_id}"));
        Ok(out)
    }

    // ─── quality: audits, findings, SCARs, events ─────────────────────────

    pub fn record_audit(&self, supplier_id: &str, audit_type: &str, auditor: &str, conducted_on: NaiveDate, findings: Vec<AuditFinding>, actor: &str) -> Result<QualityAudit, String> {
        if !self.supplier_exists(supplier_id) { return Err(format!("Supplier not found: {supplier_id}")); }
        // Score: start at 100, subtract weighted by finding severity.
        let mut score = 100.0_f64;
        for f in &findings {
            score -= match f.severity.to_lowercase().as_str() {
                "critical" => 25.0,
                "major" => 10.0,
                _ => 3.0,
            };
        }
        score = score.max(0.0);
        let has_critical = findings.iter().any(|f| f.severity.eq_ignore_ascii_case("critical"));
        let result = if has_critical || score < 60.0 { AuditResult::Fail }
            else if score < 85.0 { AuditResult::ConditionalPass }
            else { AuditResult::Pass };
        let a = QualityAudit { id: self.next("AUD"), supplier_id: supplier_id.to_string(), audit_type: audit_type.to_string(), auditor: auditor.to_string(), conducted_on, score, result, findings, created_at: Utc::now() };
        self.audits.lock().unwrap().insert(a.id.clone(), a.clone());
        self.audit(actor, "record_audit", format!("{supplier_id} {result:?} score {score}"));

        // Auto-raise a SCAR for failed audits so corrective action is tracked.
        if result == AuditResult::Fail {
            let scar = Scar { id: self.next("SCAR"), supplier_id: supplier_id.to_string(), audit_id: Some(a.id.clone()), title: format!("Failed {audit_type} audit"), severity: "major".into(), status: ScarStatus::Open, root_cause: None, corrective_action: None, due_date: Some(conducted_on + Duration::days(30)), created_by: actor.to_string(), created_at: Utc::now(), closed_at: None };
            self.scars.lock().unwrap().insert(scar.id.clone(), scar.clone());
            self.audit("system", "auto_scar", format!("{} from {}", scar.id, a.id));
        }
        Ok(a)
    }

    pub fn audits_for(&self, supplier_id: &str) -> Vec<QualityAudit> {
        let mut v: Vec<QualityAudit> = self.audits.lock().unwrap().values().filter(|a| a.supplier_id == supplier_id).cloned().collect();
        v.sort_by(|a, b| b.conducted_on.cmp(&a.conducted_on));
        v
    }

    pub fn raise_scar(&self, supplier_id: &str, title: &str, severity: &str, audit_id: Option<String>, due_date: Option<NaiveDate>, actor: &str) -> Result<Scar, String> {
        if !self.supplier_exists(supplier_id) { return Err(format!("Supplier not found: {supplier_id}")); }
        let s = Scar { id: self.next("SCAR"), supplier_id: supplier_id.to_string(), audit_id, title: title.to_string(), severity: severity.to_string(), status: ScarStatus::Open, root_cause: None, corrective_action: None, due_date, created_by: actor.to_string(), created_at: Utc::now(), closed_at: None };
        self.scars.lock().unwrap().insert(s.id.clone(), s.clone());
        self.audit(actor, "raise_scar", s.id.clone());
        Ok(s)
    }

    pub fn update_scar(&self, scar_id: &str, status: Option<ScarStatus>, root_cause: Option<String>, corrective_action: Option<String>, actor: &str) -> Result<Scar, String> {
        let mut scars = self.scars.lock().unwrap();
        let s = scars.get_mut(scar_id).ok_or_else(|| format!("SCAR not found: {scar_id}"))?;
        if let Some(st) = status {
            s.status = st;
            if st == ScarStatus::Closed { s.closed_at = Some(Utc::now()); }
        }
        if root_cause.is_some() { s.root_cause = root_cause; }
        if corrective_action.is_some() { s.corrective_action = corrective_action; }
        let out = s.clone();
        drop(scars);
        self.audit(actor, "update_scar", format!("{scar_id} -> {:?}", out.status));
        Ok(out)
    }

    pub fn scars_for(&self, supplier_id: &str, open_only: bool) -> Vec<Scar> {
        let mut v: Vec<Scar> = self.scars.lock().unwrap().values().filter(|s| s.supplier_id == supplier_id && (!open_only || s.status != ScarStatus::Closed)).cloned().collect();
        v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        v
    }

    pub fn record_quality_event(&self, supplier_id: &str, item_id: Option<String>, po_id: Option<String>, kind: &str, qty: u32, defect_qty: u32, on_time: bool, note: &str, occurred_on: NaiveDate, actor: &str) -> Result<QualityEvent, String> {
        if !self.supplier_exists(supplier_id) { return Err(format!("Supplier not found: {supplier_id}")); }
        let e = QualityEvent { id: self.next("QE"), supplier_id: supplier_id.to_string(), item_id, po_id, kind: kind.to_string(), qty, defect_qty: defect_qty.min(qty), on_time, note: note.to_string(), occurred_on, created_at: Utc::now() };
        self.events.lock().unwrap().push(e.clone());
        self.audit(actor, "record_quality_event", format!("{supplier_id} {kind}"));
        Ok(e)
    }

    // ─── risk ──────────────────────────────────────────────────────────────

    pub fn assess_risk(&self, supplier_id: &str, category: &str, likelihood: u8, impact: u8, note: &str, assessed_on: NaiveDate, actor: &str) -> Result<RiskAssessment, String> {
        if !self.supplier_exists(supplier_id) { return Err(format!("Supplier not found: {supplier_id}")); }
        let r = RiskAssessment { id: self.next("RSK"), supplier_id: supplier_id.to_string(), category: category.to_string(), likelihood: likelihood.clamp(1, 5), impact: impact.clamp(1, 5), note: note.to_string(), assessed_by: actor.to_string(), assessed_on, created_at: Utc::now() };
        self.risks.lock().unwrap().insert(r.id.clone(), r.clone());
        self.audit(actor, "assess_risk", format!("{supplier_id} {category}"));
        Ok(r)
    }

    // ─── derived analytics ─────────────────────────────────────────────────

    /// Performance scorecard from quality events: on-time delivery, defect/PPM,
    /// quality rate, plus open SCAR count and the latest audit score.
    pub fn scorecard(&self, supplier_id: &str) -> Option<serde_json::Value> {
        let sup = self.get_supplier(supplier_id)?;
        let events = self.events.lock().unwrap();
        let sup_events: Vec<&QualityEvent> = events.iter().filter(|e| e.supplier_id == supplier_id).collect();
        let receipts: Vec<&&QualityEvent> = sup_events.iter().filter(|e| e.kind == "receipt" || e.kind == "nonconformance").collect();
        let deliveries = receipts.len() as f64;
        let on_time = receipts.iter().filter(|e| e.on_time).count() as f64;
        let total_qty: u64 = receipts.iter().map(|e| e.qty as u64).sum();
        let defect_qty: u64 = receipts.iter().map(|e| e.defect_qty as u64).sum();
        let otd = if deliveries > 0.0 { (on_time / deliveries * 1000.0).round() / 10.0 } else { 100.0 };
        let quality_rate = if total_qty > 0 { ((total_qty - defect_qty) as f64 / total_qty as f64 * 1000.0).round() / 10.0 } else { 100.0 };
        let ppm = if total_qty > 0 { (defect_qty as f64 / total_qty as f64 * 1_000_000.0).round() } else { 0.0 };
        let open_scars = self.scars.lock().unwrap().values().filter(|s| s.supplier_id == supplier_id && s.status != ScarStatus::Closed).count();
        let latest_audit = self.audits.lock().unwrap().values().filter(|a| a.supplier_id == supplier_id).map(|a| a.score).fold(None, |acc: Option<f64>, s| Some(acc.map_or(s, |m| m.max(s))));
        // Composite 0..100: weighted OTD (35), quality (45), audit (20), minus SCAR penalty.
        let audit_component = latest_audit.unwrap_or(85.0);
        let mut composite = otd * 0.35 + quality_rate * 0.45 + audit_component * 0.20;
        composite -= (open_scars as f64) * 3.0;
        composite = composite.clamp(0.0, 100.0);
        let rating = if composite >= 90.0 { "A" } else if composite >= 80.0 { "B" } else if composite >= 70.0 { "C" } else { "D" };
        Some(serde_json::json!({
            "supplier_id": supplier_id,
            "supplier_name": sup.name,
            "deliveries_measured": deliveries as u64,
            "on_time_delivery_pct": otd,
            "quality_rate_pct": quality_rate,
            "defect_ppm": ppm,
            "open_scars": open_scars,
            "latest_audit_score": latest_audit,
            "composite_score": (composite * 10.0).round() / 10.0,
            "rating": rating,
        }))
    }

    /// Risk profile: highest of (assessment likelihood×impact) plus signals from
    /// quality, qualification, certification expiry and single-source exposure.
    pub fn risk_profile(&self, supplier_id: &str) -> Option<serde_json::Value> {
        let sup = self.get_supplier(supplier_id)?;
        let risks = self.risks.lock().unwrap();
        let assessments: Vec<&RiskAssessment> = risks.values().filter(|r| r.supplier_id == supplier_id).collect();
        let max_score = assessments.iter().map(|r| (r.likelihood * r.impact) as u32).max().unwrap_or(0);
        let mut signals: Vec<String> = Vec::new();
        let mut level_num = match max_score {
            0..=4 => 1,
            5..=9 => 2,
            10..=14 => 3,
            15..=19 => 4,
            _ => 5,
        };
        // Qualification / status signals.
        if !sup.approved_for_po { signals.push("not approved for PO".into()); level_num = level_num.max(3); }
        if sup.status == SupplierStatus::Suspended || sup.status == SupplierStatus::Disqualified { signals.push("supplier suspended/disqualified".into()); level_num = 5; }
        if sup.qualification == QualificationStatus::Expired { signals.push("qualification expired".into()); level_num = level_num.max(3); }
        // Certification expiry.
        let today = Utc::now().date_naive();
        let expired_certs = self.certs.lock().unwrap().values().filter(|c| c.supplier_id == supplier_id && c.expires_on < today).count();
        if expired_certs > 0 { signals.push(format!("{expired_certs} expired certification(s)")); level_num = level_num.max(3); }
        // Open SCARs.
        let open_scars = self.scars.lock().unwrap().values().filter(|s| s.supplier_id == supplier_id && s.status != ScarStatus::Closed).count();
        if open_scars > 0 { signals.push(format!("{open_scars} open SCAR(s)")); level_num = level_num.max(2); }
        // Single-source exposure: items only this supplier offers.
        let sis = self.supplier_items.lock().unwrap();
        let this_items: std::collections::HashSet<&String> = sis.values().filter(|si| si.supplier_id == supplier_id).map(|si| &si.item_id).collect();
        let single_source: Vec<String> = this_items.iter().filter(|item| {
            sis.values().filter(|si| &&si.item_id == *item).map(|si| &si.supplier_id).collect::<std::collections::HashSet<_>>().len() == 1
        }).map(|s| s.to_string()).collect();
        if !single_source.is_empty() { signals.push(format!("single-source for {} item(s)", single_source.len())); level_num = level_num.max(3); }
        let level = match level_num { 1 => "low", 2 => "low", 3 => "medium", 4 => "high", _ => "critical" };
        Some(serde_json::json!({
            "supplier_id": supplier_id,
            "supplier_name": sup.name,
            "risk_level": level,
            "risk_score": level_num,
            "max_assessment_score": max_score,
            "signals": signals,
            "single_source_items": single_source,
            "assessments": assessments.iter().map(|r| serde_json::json!({"category": r.category, "likelihood": r.likelihood, "impact": r.impact, "score": r.likelihood*r.impact, "note": r.note})).collect::<Vec<_>>(),
        }))
    }

    /// Portfolio risk monitor: every supplier at or above the minimum level,
    /// most severe first. Powers the Supplier Risk Monitor agent.
    pub fn monitor_risks(&self, min_level: u8) -> serde_json::Value {
        let ids: Vec<String> = self.suppliers.lock().unwrap().keys().cloned().collect();
        let mut rows: Vec<serde_json::Value> = ids.iter().filter_map(|id| self.risk_profile(id)).filter(|p| p["risk_score"].as_u64().unwrap_or(0) >= min_level as u64).collect();
        rows.sort_by(|a, b| b["risk_score"].as_u64().cmp(&a["risk_score"].as_u64()));
        serde_json::json!({"min_level": min_level, "flagged_count": rows.len(), "suppliers": rows})
    }

    pub fn audit_log(&self, limit: usize) -> Vec<AuditEntry> {
        let log = self.audit_log.lock().unwrap();
        log.iter().rev().take(limit).cloned().collect()
    }

    // ─── seed ────────────────────────────────────────────────────────────

    fn seed(&self) {
        let today = Utc::now().date_naive();

        // Electronics supplier — qualified, with PCB items.
        let acme = self.create_supplier("Acme Electronics Co", "electronics", "Taiwan", 21, "system");
        self.set_qualification(&acme.id, QualificationStatus::Qualified, "system").ok();
        self.add_contact(&acme.id, "Wei Chen", "Account Manager", "wei@acme-elec.example", Some("+886-2-1234".into()), true, "system").ok();
        self.add_certification(&acme.id, "ISO 9001", "TW-9001-5521", "SGS", today - Duration::days(400), today + Duration::days(330), "system").ok();
        self.add_certification(&acme.id, "IATF 16949", "TW-16949-220", "SGS", today - Duration::days(700), today + Duration::days(40), "system").ok();

        // Food/beverage supplier — qualified with food-grade certs.
        let fresh = self.create_supplier("Fresh Valley Foods", "food-beverage", "USA", 7, "system");
        self.set_qualification(&fresh.id, QualificationStatus::Qualified, "system").ok();
        self.add_certification(&fresh.id, "ISO 22000", "US-22000-901", "NSF", today - Duration::days(200), today + Duration::days(165), "system").ok();
        self.add_contact(&fresh.id, "Maria Lopez", "Sales", "maria@freshvalley.example", None, true, "system").ok();

        // Manufacturing supplier — conditionally qualified, an open SCAR-worthy past.
        let bolt = self.create_supplier("BoltWorks Manufacturing", "manufacturing", "Mexico", 14, "system");
        self.set_qualification(&bolt.id, QualificationStatus::ConditionallyQualified, "system").ok();

        // Retail/wholesale distributor — qualified.
        let dist = self.create_supplier("Global Retail Distributors", "retail", "USA", 3, "system");
        self.set_qualification(&dist.id, QualificationStatus::Qualified, "system").ok();

        // A prospect not yet approved (for PO-gate negative tests).
        let _proto = self.create_supplier("Proto Parts LLC", "electronics", "China", 30, "system");

        // Catalog items.
        let pcb = self.create_item("PCB-100", "4-layer PCB", "electronics", "ea", "system");
        let cap = self.create_item("CAP-22UF", "22uF capacitor", "electronics", "ea", "system");
        let sugar = self.create_item("SUG-50", "Cane sugar 50kg", "food-beverage", "bag", "system");
        let bolt_item = self.create_item("BLT-M8", "M8 bolt", "manufacturing", "ea", "system");
        let sku42 = self.create_item("SKU-42", "Retail widget", "retail", "ea", "system");

        // Multi-supplier pricing — PCB sourced from two suppliers (Acme + BoltWorks).
        self.set_supplier_item(&acme.id, &pcb.id, "ACM-PCB4", 4.20, "USD", 100, 21, 5000, true, "system").ok();
        self.set_supplier_item(&bolt.id, &pcb.id, "BW-PCB", 3.95, "USD", 250, 28, 1200, false, "system").ok();
        self.set_supplier_item(&acme.id, &cap.id, "ACM-22UF", 0.04, "USD", 1000, 21, 200000, true, "system").ok();
        self.set_supplier_item(&fresh.id, &sugar.id, "FV-SUG50", 38.50, "USD", 10, 7, 800, true, "system").ok();
        self.set_supplier_item(&bolt.id, &bolt_item.id, "BW-M8", 0.12, "USD", 5000, 14, 100000, true, "system").ok();
        self.set_supplier_item(&dist.id, &sku42.id, "GRD-42", 9.99, "USD", 24, 3, 4000, true, "system").ok();

        // Quality history for scorecards.
        self.record_quality_event(&acme.id, Some(pcb.id.clone()), None, "receipt", 5000, 5, true, "lot A", today - Duration::days(40), "system").ok();
        self.record_quality_event(&acme.id, Some(pcb.id.clone()), None, "receipt", 4000, 2, true, "lot B", today - Duration::days(20), "system").ok();
        self.record_quality_event(&fresh.id, Some(sugar.id.clone()), None, "receipt", 800, 0, true, "delivery", today - Duration::days(10), "system").ok();
        self.record_quality_event(&bolt.id, Some(bolt_item.id.clone()), None, "receipt", 50000, 600, false, "late + defects", today - Duration::days(15), "system").ok();
        self.record_quality_event(&bolt.id, Some(bolt_item.id.clone()), None, "nonconformance", 10000, 400, false, "dimensional NC", today - Duration::days(5), "system").ok();

        // A past audit on BoltWorks with a major finding.
        self.record_audit(&bolt.id, "process", "qa.auditor", today - Duration::days(30), vec![
            AuditFinding { severity: "major".into(), clause: "8.5.1".into(), description: "Inadequate process control on thread rolling".into() },
            AuditFinding { severity: "minor".into(), clause: "7.1.5".into(), description: "Calibration records incomplete".into() },
        ], "qa.auditor").ok();

        // Risk assessments.
        self.assess_risk(&bolt.id, "capacity", 4, 4, "Single line, high utilization", today - Duration::days(10), "risk.analyst").ok();
        self.assess_risk(&acme.id, "geographic", 3, 3, "Concentration in one region", today - Duration::days(60), "risk.analyst").ok();
    }
}
