# Differential Review Report

**Review ID:** 2026-07-12-001
**Files Changed:** 3 (26 insertions, 9 deletions)
**Codebase Size:** SMALL (<20 files) — DEEP analysis
**Risk Level:** LOW–MEDIUM

---

## Summary

| File | Changes | Risk |
|------|---------|------|
| `src/readers.rs` | Fix header detection logic in `LogQueryReader` | MEDIUM |
| `src/query.rs` | Add `has_non_triggers` to skip trigger filter when all records are triggers | LOW |
| `static/index.html` | Remove copy icon, hide horizontal overflow, persist `showTriggers` across files | LOW |

All 14 tests pass. Build compiles cleanly for both CLI and desktop targets.

---

## File-by-File Analysis

### 1. `src/readers.rs` — Fix LogQueryReader header detection

**Lines:** 158–173

**What changed:**
The header detection loop now handles three cases:

| Line starts with | Line contains | Action |
|---|---|---|
| `/***` | `CONTEXT@` | Break — valid reu.out header |
| *(anything else)* | *(any)* | Break — bare CONTEXT header (test fixture format) |
| `/***` | *(no `CONTEXT@`)* | Skip — prefix marker only |

**Assessment:** Correct. The old code inverted the logic: it skipped `/***` lines (where the actual CONTEXT data lives) and broke on non-`/***` lines (the SQL statement). This produced garbled records where the SQL was treated as header and the next header as SQL, causing `view_reu.json` regex matching to fail.

**Blast radius:** Confined to `LogQueryReader`. Both test fixtures (`sample_reu.out`, `sample_reu_detect.out`) and the real `reu.out` file are handled correctly.

### 2. `src/query.rs` — has_non_triggers optimization

**Lines changed:** 14–17, 99, 144, 222, 244, 281, 343, 410–414

**What changed:**
- Added `has_non_triggers: bool` to `CachedDataSet`
- Populated in `load_records()` by checking if any record has a component NOT starting with `TRIGGER`
- Skip the trigger filter in both fast-path and filter-path when `has_non_triggers` is false

**Assessment:** Correct. When ALL records have `component` starting with `TRIGGER` (as in `reu.out`), the trigger filter would hide every record. The new mechanism detects this at load time and bypasses the filter.

**Performance note:** The `any()` check iterates all records once during cache population. For CORE.OUT (382K records), the first non-trigger is found early (short-circuit). For reu.out (~3.8K records), all records are checked — negligible cost.

**Behavior unchanged for mixed datasets:** When `has_non_triggers=true` (CORE.OUT), the trigger filter works exactly as before: `showTriggers=false` hides triggers, `showTriggers=true` shows all.

### 3. `static/index.html` — UI refinements

- **Removed clipboard icon** from "Copy" button (📋 → "") — cosmetic change
- **`overflow-x: hidden`** on `.table-wrap` — prevents horizontal scrollbar when total explicit column widths exceed container width. The `msg` column uses `text-overflow: ellipsis`; full content is available via click-to-expand modal.
- **Removed `showTriggers` reset** in `resetAndLoadFile()` — `showTriggers` now persists across file changes, matching the requirement in AGENTS.md ("Persiste al cambiar de archivo")

---

## Test Coverage

```
14 passed; 0 failed
```

All reader tests, query tests, and process tests pass. The fix handles both fixture formats and the real reu.out file correctly.

### CLI verification

```bash
./target/release/potatolog process view_reu.json examples/reu.out | wc -l
# 3770 records extracted
./target/release/potatolog process view_core.json examples/CORE.OUT | wc -l
# ~382K records extracted
```

---

## Blast Radius

| Change | Scope | Impact |
|--------|-------|--------|
| `readers.rs` header detection | `LogQueryReader` only | Affects reu.out parsing only; CORE.OUT uses `LogCoreReader`, plain files use `LogFile` |
| `query.rs` has_non_triggers | All queries | Only changes behavior when `has_non_triggers=false` (all records are triggers); otherwise identical |
| `index.html` CSS | Frontend layout | Hides horizontal scrollbar; vertical scrollbar unaffected |

---

## Findings

### No security issues
- No authentication, crypto, or access control code touched
- CSP configuration unchanged
- Path traversal protection in `query.rs:89-96` unaffected

### No regressions
- Fast path (no filters) still handles `showTriggers=true` correctly
- Mixed datasets (CORE.OUT) filter triggers as before
- Date filtering, user filtering, pagination unaffected

---

## Recommendations

None. All changes are low-risk, well-scoped, and verified by tests.

---

## Build Verification

- `cargo test` — 14/14 pass
- `cargo build --release` — CLI binary compiles
- `cargo build --release -p potatolog-desktop` — Desktop binary compiles
- `cargo tauri build --bundles "app,dmg"` — macOS bundle (.app + .dmg) created
