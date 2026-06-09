//! Integration tests for the SRM store: qualification → PO gating, sourcing,
//! receiving, RFQ award, audit scoring + auto-SCAR, scorecard, and risk.

use chrono::Utc;
use mcp_supplier::store::SupplierStore;
use mcp_supplier::types::*;

fn store() -> SupplierStore {
    SupplierStore::new()
}

#[test]
fn seed_has_suppliers_and_items() {
    let s = store();
    assert!(s.list_suppliers(None, None).len() >= 5);
    assert!(!s.list_items(None).is_empty());
}

#[test]
fn po_gate_blocks_unapproved_supplier() {
    let s = store();
    // Proto Parts LLC is seeded as an unqualified prospect.
    let proto = s.list_suppliers(None, None).into_iter().find(|x| x.name.contains("Proto")).unwrap();
    assert!(!proto.approved_for_po);
    let item = s.list_items(None)[0].clone();
    let err = s.create_po(&proto.id, "USD", vec![(item.id.clone(), "x".into(), 10, 1.0)], None, "buyer").unwrap_err();
    assert!(err.contains("not approved"), "got: {err}");
}

#[test]
fn qualifying_enables_po() {
    let s = store();
    let sup = s.create_supplier("New Co", "electronics", "USA", 10, "t");
    assert!(!sup.approved_for_po);
    let q = s.set_qualification(&sup.id, QualificationStatus::Qualified, "t").unwrap();
    assert!(q.approved_for_po);
    assert_eq!(q.status, SupplierStatus::Active);
    let item = s.create_item("NEW-1", "thing", "electronics", "ea", "t");
    let po = s.create_po(&sup.id, "USD", vec![(item.id, "thing".into(), 100, 2.5)], None, "buyer").unwrap();
    assert_eq!(po.status, PoStatus::Issued);
    assert!((po.total - 250.0).abs() < 0.001);
}

#[test]
fn suspending_revokes_po_approval() {
    let s = store();
    let acme = s.list_suppliers(Some("electronics"), None).into_iter().find(|x| x.name.contains("Acme")).unwrap();
    assert!(acme.approved_for_po);
    let sus = s.set_supplier_status(&acme.id, SupplierStatus::Suspended, "qa").unwrap();
    assert!(!sus.approved_for_po);
}

#[test]
fn find_sources_ranks_and_filters() {
    let s = store();
    // PCB-100 is offered by Acme (4.20, approved) and BoltWorks (3.95, conditional/approved).
    let pcb = s.list_items(None).into_iter().find(|i| i.sku == "PCB-100").unwrap();
    let res = s.find_item_sources(&pcb.id, 500, true);
    assert!(res["source_count"].as_u64().unwrap() >= 2);
    // Cheapest that meets the requirement should be recommended.
    let rec = &res["recommended"];
    assert_eq!(rec["meets_requirement"], true);
}

#[test]
fn receive_po_progresses_status() {
    let s = store();
    let fresh = s.list_suppliers(Some("food-beverage"), None)[0].clone();
    let sugar = s.list_items(Some("food-beverage")).into_iter().find(|i| i.sku == "SUG-50").unwrap();
    let po = s.create_po(&fresh.id, "USD", vec![(sugar.id.clone(), "sugar".into(), 100, 38.5)], None, "buyer").unwrap();
    let partial = s.receive_po(&po.id, vec![(sugar.id.clone(), 40)], "warehouse").unwrap();
    assert_eq!(partial.status, PoStatus::PartiallyReceived);
    let full = s.receive_po(&po.id, vec![(sugar.id.clone(), 60)], "warehouse").unwrap();
    assert_eq!(full.status, PoStatus::Received);
    // cannot cancel a received PO
    assert!(s.cancel_po(&po.id, "x", "buyer").is_err());
}

#[test]
fn rfq_award_flow() {
    let s = store();
    let item = s.list_items(None)[0].clone();
    let a = s.list_suppliers(None, None)[0].clone();
    let rfq = s.create_rfq(&item.id, 1000, vec![a.id.clone()], None, "sourcing").unwrap();
    s.submit_quote(&rfq.id, &a.id, 1.10, "USD", 20, None, "sourcing").unwrap();
    let b = s.list_suppliers(None, None)[1].clone();
    s.submit_quote(&rfq.id, &b.id, 0.95, "USD", 25, None, "sourcing").unwrap();
    let ranked = s.compare_quotes(&rfq.id);
    assert_eq!(ranked[0].supplier_id, b.id, "cheapest first");
    let awarded = s.award_rfq(&rfq.id, &b.id, "sourcing").unwrap();
    assert_eq!(awarded.status, RfqStatus::Awarded);
}

#[test]
fn failed_audit_auto_raises_scar() {
    let s = store();
    let sup = s.create_supplier("RiskCo", "manufacturing", "X", 30, "t");
    let before = s.scars_for(&sup.id, false).len();
    let a = s.record_audit(&sup.id, "process", "auditor", Utc::now().date_naive(), vec![
        AuditFinding { severity: "critical".into(), clause: "8.5".into(), description: "no process control".into() },
    ], "auditor").unwrap();
    assert_eq!(a.result, AuditResult::Fail);
    let after = s.scars_for(&sup.id, false).len();
    assert_eq!(after, before + 1, "a SCAR should be auto-raised on failure");
}

#[test]
fn audit_scoring_tiers() {
    let s = store();
    let sup = s.create_supplier("ScoreCo", "manufacturing", "X", 30, "t");
    let pass = s.record_audit(&sup.id, "desk", "a", Utc::now().date_naive(), vec![], "a").unwrap();
    assert_eq!(pass.result, AuditResult::Pass);
    assert!((pass.score - 100.0).abs() < 0.001);
    let cond = s.record_audit(&sup.id, "desk", "a", Utc::now().date_naive(), vec![
        AuditFinding { severity: "major".into(), clause: "x".into(), description: "y".into() },
        AuditFinding { severity: "major".into(), clause: "x".into(), description: "y".into() },
    ], "a").unwrap();
    assert_eq!(cond.result, AuditResult::ConditionalPass); // 100-20 = 80
}

#[test]
fn scorecard_reflects_quality() {
    let s = store();
    let bolt = s.list_suppliers(Some("manufacturing"), None).into_iter().find(|x| x.name.contains("Bolt")).unwrap();
    let sc = s.scorecard(&bolt.id).unwrap();
    // BoltWorks seeded with late + defective receipts -> below perfect.
    assert!(sc["quality_rate_pct"].as_f64().unwrap() < 100.0);
    assert!(sc["on_time_delivery_pct"].as_f64().unwrap() < 100.0);
    assert!(sc["defect_ppm"].as_f64().unwrap() > 0.0);
}

#[test]
fn risk_profile_and_monitor() {
    let s = store();
    let bolt = s.list_suppliers(Some("manufacturing"), None).into_iter().find(|x| x.name.contains("Bolt")).unwrap();
    let rp = s.risk_profile(&bolt.id).unwrap();
    // capacity 4x4=16 -> level 4 (high) at minimum.
    assert!(rp["risk_score"].as_u64().unwrap() >= 4);
    let mon = s.monitor_risks(3);
    assert!(mon["flagged_count"].as_u64().unwrap() >= 1);
}

#[test]
fn single_source_signal() {
    let s = store();
    // SUG-50 only offered by Fresh Valley -> single-source signal on that supplier.
    let fresh = s.list_suppliers(Some("food-beverage"), None)[0].clone();
    let rp = s.risk_profile(&fresh.id).unwrap();
    let singles = rp["single_source_items"].as_array().unwrap();
    assert!(!singles.is_empty(), "fresh valley is single-source for sugar");
}

#[test]
fn expiring_certs_watchlist() {
    let s = store();
    // Acme's IATF cert expires in ~40 days (seeded).
    let soon = s.expiring_certifications(90);
    assert!(soon.iter().any(|c| c.standard == "IATF 16949"));
}
