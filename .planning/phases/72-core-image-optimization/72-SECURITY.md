---
phase: 72
slug: core-image-optimization
status: verified
threats_open: 0
asvs_level: 1
created: 2026-04-11
---

# Phase 72 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| Dockerfile base image | Switching from debian:bookworm-slim to distroless changes the trust surface | Container runtime binaries, CA certificates |
| ghcr.io image consumers | Downstream users COPY --from= the core image; removing binaries is a breaking change | Binary artifacts |

---

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation | Status |
|-----------|----------|-----------|-------------|------------|--------|
| T-72-01 | Tampering | Distroless base image | accept | Deferred to Phase 74 — digest pinning tracked via TODO comments in Dockerfile lines 25, 32 | closed |
| T-72-02 | Denial of Service | Missing binaries in core | accept | Intentional per D-03 — Phase 74 builder image provides ext/ binaries via COPY --from= | closed |
| T-72-03 | Information Disclosure | Strip symbols | accept | strip=true removes debug symbols — reduces info leak surface (positive) | closed |

*Status: open · closed*
*Disposition: mitigate (implementation required) · accept (documented risk) · transfer (third-party)*

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-72-01 | T-72-01 | Distroless image not pinned by digest — tag-only reference acceptable short-term. Phase 74 will add reproducible digest pinning. Risk: tag could be updated upstream between builds. | User (approved) | 2026-04-11 |
| AR-72-02 | T-72-02 | Extension binaries intentionally removed from core image per D-03. Phase 74 builder image will provide them. | Plan decision D-03 | 2026-04-11 |
| AR-72-03 | T-72-03 | Strip symbols reduces information disclosure surface — positive security outcome. | Plan decision | 2026-04-11 |

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-04-11 | 3 | 3 | 0 | gsd-secure-phase (orchestrator) |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-04-11
