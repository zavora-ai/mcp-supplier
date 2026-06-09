# Supplier MCP Server

[![Crates.io](https://img.shields.io/crates/v/mcp-supplier.svg)](https://crates.io/crates/mcp-supplier)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![ADK-Rust Enterprise](https://img.shields.io/badge/ADK--Rust-Enterprise-purple.svg)](https://enterprise.adk-rust.com)
[![Registry Ready](https://img.shields.io/badge/ADK_Registry-Ready-green.svg)](https://www.zavora.ai)

A Supplier Relationship Management (SRM) platform for [ADK-Rust Enterprise](https://enterprise.adk-rust.com) procurement and supply-chain agents. 34 MCP tools spanning the full supply base lifecycle: suppliers & contacts, certifications & qualification, a product catalog with multi-supplier pricing, purchase orders, RFQ/sourcing, quality audits + SCARs, performance scorecards, and supply-risk monitoring — with a **full audit trail** and **governance gates on high-impact writes**.

## A platform, not a point solution

This is modeled as a general SRM/procurement backbone (à la SAP Ariba / Coupa / Jaggaer), so a wide range of agents across industries are simply clients of the same supply base:

| Industry | Agent | Uses |
|----------|-------|------|
| Electronics | Supplier Quality Audit Agent | `record_audit`, `raise_scar`, `update_scar`, `scorecard` |
| Food & Beverage | Recipe Cost Optimizer | `find_item_sources`, `compare_quotes`, `list_items` |
| Food & Beverage | Supplier Risk Monitor | `monitor_risks`, `risk_profile`, `expiring_certifications` |
| Manufacturing | Inventory Shortage Resolver | `find_item_sources`, `create_rfq`, `create_po` |
| Manufacturing | Supplier Quality Agent | `record_quality_event`, `scorecard`, `record_audit` |
| Retail / e-commerce | Inventory Replenishment Agent | `find_item_sources`, `create_po`, `receive_po` |

## Architecture

<p align="center">
  <img src="https://raw.githubusercontent.com/zavora-ai/mcp-supplier/main/docs/architecture.svg" alt="Supplier MCP Architecture" width="780"/>
</p>

## Capabilities

- **Suppliers** — lifecycle status (prospect → active → on-hold/suspended/disqualified) and qualification (unqualified → in-qualification → qualified/conditional/expired). Qualification controls **PO approval**.
- **Contacts & certifications** — supplier contacts; ISO 9001 / IATF 16949 / ISO 22000 / HACCP certs with expiry tracking and a cross-supplier expiry watchlist.
- **Catalog & multi-source pricing** — internal items with per-supplier offers (price, MOQ, lead time, availability). `find_item_sources` ranks sources by price/availability/lead time.
- **Purchase orders** — issue, receive (partial/full), cancel. **A PO cannot be issued to a supplier that isn't approved.**
- **RFQ / sourcing** — open RFQs, collect quotes, compare cheapest-first, award.
- **Quality** — audits with auto-scoring and pass/conditional/fail; **failed audits auto-raise a SCAR**; SCAR lifecycle; quality/delivery events feeding the scorecard.
- **Scorecards** — on-time delivery, quality rate, defect PPM, open-SCAR penalty, latest audit score → composite rating (A–D).
- **Risk** — likelihood×impact assessments plus derived signals (qualification, expired certs, open SCARs, **single-source exposure**); portfolio-wide `monitor_risks`.

## Governance posture

- **High-impact writes are gated** (`requires_approval`): `create_po` and `cancel_po` (classed `external_write` — they commit spend with a supplier), plus `set_supplier_status`, `set_qualification`, and `award_rfq`.
- **The PO gate has teeth** — `create_po` refuses any supplier not approved for POs (wrong status or qualification), so an agent cannot accidentally buy from an unqualified or suspended source.
- **Everything is audited** — every state change appends to an audit trail (`audit_log`).
- **Reads are `read_only`** — sourcing, scorecards, risk profiles, and watchlists never mutate state.
- Sample data is fictitious.

## Tools (34)

### Suppliers (7)
`create_supplier` · `get_supplier` · `list_suppliers` · `set_supplier_status` (gated) · `set_qualification` (gated) · `add_contact` · `list_contacts`

### Certifications (3)
`add_certification` · `list_certifications` · `expiring_certifications`

### Catalog & Sourcing (4)
`create_item` · `list_items` · `set_supplier_item` · `find_item_sources`

### Purchase Orders (5)
`create_po` (gated, external) · `get_po` · `list_pos` · `receive_po` · `cancel_po` (gated, external)

### RFQ / Quotes (4)
`create_rfq` · `submit_quote` · `compare_quotes` · `award_rfq` (gated)

### Quality (7)
`record_audit` · `list_audits` · `raise_scar` · `update_scar` · `list_scars` · `record_quality_event` · `scorecard`

### Risk & Audit (4)
`assess_risk` · `risk_profile` · `monitor_risks` · `audit_log`

## Example

```jsonc
// Replenishment: find the best approved source, then issue a PO
{"name": "find_item_sources", "arguments": {"item_id": "ITM-1021", "required_qty": 500}}
{"name": "create_po", "arguments": {"supplier_id": "SUP-1003",
  "lines": [{"item_id": "ITM-1021", "qty": 500, "unit_price": 3.95}]}}

// Quality: a failing audit auto-raises a SCAR
{"name": "record_audit", "arguments": {"supplier_id": "SUP-1003", "audit_type": "on-site",
  "findings": [{"severity": "critical", "clause": "8.5.1", "description": "no process control"}]}}

// Risk: scan the portfolio
{"name": "monitor_risks", "arguments": {"min_level": 3}}
```

## Install & run

```bash
cargo install mcp-supplier
mcp-supplier            # serves MCP over stdio
```

Or build from source:

```bash
git clone https://github.com/zavora-ai/mcp-supplier
cd mcp-supplier && cargo build --release
./target/release/mcp-supplier
```

## Registry manifest

```toml
server_id = "mcp_supplier"
display_name = "Supplier (SRM)"
version = "1.0.0"
domain = "procurement"
risk_level = "high"
writes_allowed = "gated"
```

The full [`mcp-server.toml`](mcp-server.toml) declares all 34 tools with risk classes and approval gates for registry onboarding.

## License

Apache-2.0
