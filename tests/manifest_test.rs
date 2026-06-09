//! Validate mcp-server.toml parses, passes SDK validation, has the right tool
//! count, and gates the high-impact procurement writes.

use adk_mcp_sdk::manifest::ServerManifest;
use std::path::Path;

fn manifest() -> ServerManifest {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("mcp-server.toml");
    ServerManifest::from_file(&path).expect("manifest should parse")
}

#[test]
fn manifest_parses_and_validates() {
    let m = manifest();
    assert!(m.validate().is_empty(), "validation errors: {:?}", m.validate());
    assert_eq!(m.server_id, "mcp_supplier");
    assert_eq!(m.domain, "procurement");
    assert_eq!(m.tools.len(), 34, "expected 34 declared tools");
}

#[test]
fn high_impact_writes_are_gated() {
    let m = manifest();
    for name in ["set_supplier_status", "set_qualification", "create_po", "cancel_po", "award_rfq"] {
        let t = m.tools.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("{name} present"));
        assert!(t.requires_approval, "{name} must require approval");
    }
}

#[test]
fn po_ops_are_external_write() {
    use adk_mcp_sdk::risk::RiskClass;
    let m = manifest();
    for name in ["create_po", "cancel_po"] {
        let t = m.tools.iter().find(|t| t.name == name).unwrap();
        assert_eq!(t.risk_class, RiskClass::ExternalWrite, "{name} should be external_write");
    }
}

#[test]
fn reads_are_read_only() {
    use adk_mcp_sdk::risk::RiskClass;
    let m = manifest();
    for name in ["get_supplier", "list_suppliers", "find_item_sources", "scorecard", "risk_profile", "monitor_risks", "compare_quotes", "expiring_certifications", "audit_log"] {
        let t = m.tools.iter().find(|t| t.name == name).unwrap();
        assert_eq!(t.risk_class, RiskClass::ReadOnly, "{name} should be read_only");
    }
}
