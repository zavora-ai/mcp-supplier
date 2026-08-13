# Changelog

## [1.1.0] - 2026-08-13

### Changed
- Upgraded to rmcp 3.1.2 and raised the minimum supported Rust version to 1.94.1.
- Added MCP 2026-07-28 stateless request handling while retaining MCP 2025-11-25 initialization compatibility.

### Added
- Per-request identity and protocol metadata, on-demand discovery/cache hints, and the configured Tasks and sealed MRTR approval policies.

## [1.0.0] - 2026-06-09

Initial release — a broad Supplier Relationship Management (SRM) platform.

### Added
- **Suppliers** — lifecycle status + qualification; qualification gates PO approval
  (`create_supplier`, `get_supplier`, `list_suppliers`, `set_supplier_status`, `set_qualification`, `add_contact`, `list_contacts`)
- **Certifications** — per-supplier certs with expiry tracking + cross-supplier expiry watchlist
  (`add_certification`, `list_certifications`, `expiring_certifications`)
- **Catalog & multi-source pricing** — items, per-supplier offers, ranked sourcing
  (`create_item`, `list_items`, `set_supplier_item`, `find_item_sources`)
- **Purchase orders** — issue/receive/cancel with a hard PO-approval gate
  (`create_po`, `get_po`, `list_pos`, `receive_po`, `cancel_po`)
- **RFQ / sourcing** — RFQs, quotes, comparison, award
  (`create_rfq`, `submit_quote`, `compare_quotes`, `award_rfq`)
- **Quality** — auto-scored audits with auto-SCAR on failure, SCAR lifecycle, quality events, scorecards
  (`record_audit`, `list_audits`, `raise_scar`, `update_scar`, `list_scars`, `record_quality_event`, `scorecard`)
- **Risk** — likelihood×impact assessments with derived signals (single-source, expired certs, open SCARs) and a portfolio monitor
  (`assess_risk`, `risk_profile`, `monitor_risks`, `audit_log`)
- 34 tools total; high-impact writes (`create_po`, `cancel_po`, `set_supplier_status`, `set_qualification`, `award_rfq`) gated; full audit trail.
- 17 tests (13 integration + 4 manifest); verified end-to-end over MCP stdio.
