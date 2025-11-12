# Spacedrive Normalized Cache Investigation - Complete Index

## Overview

Complete investigation of Spacedrive's normalized cache system, revealing the gap between intended design (trait-based, generic) and current implementation (special-cased with hardcoded logic).

**Status:** Investigation Complete | Ready for Implementation

---

## All Documents

### 1. START HERE: CACHE_QUICK_REFERENCE.md
**Reading time:** 2 minutes
**Purpose:** Get oriented quickly

Contains:
- Problem statement in one sentence
- 3 key files to know
- 4 hacks with exact line numbers
- 2-week fix overview
- Quick stats

**Read this if:** You have 2 minutes and want to know what's wrong

---

### 2. CACHE_INVESTIGATION_SUMMARY.md
**Reading time:** 15 minutes
**Purpose:** Understand the full picture

Contains:
- What I found (key findings 1-6)
- Comparison with Apollo/Relay
- Why the system evolved this way
- Technical debt summary with effort estimates
- Files to read with descriptions
- Key insights
- Recommendation to start with File::Identifiable

**Read this if:** You want to understand the problem and solution

---

### 3. NORMALIZED_CACHE_ANALYSIS.md
**Reading time:** 40 minutes
**Purpose:** Deep technical analysis

Contains:
- Part 1: Current state (4 major components)
- Part 2: Original intended design
- Part 3: Why custom logic was added (4 problems)
- Part 4: How Apollo, Relay, SWR handle it
- Part 5: Clean generic solution (5 phases)
- Part 6: 6-phase migration path (detailed steps + pseudocode)
- Part 7: Current blockers and technical debt
- Part 8: Summary comparison table
- Part 9: Recommended starting point with test cases

**Read this if:** You're implementing the fix or need deep understanding

---

### 4. CACHE_CODE_REFERENCES.md
**Reading time:** 30 minutes
**Purpose:** Precise code locations and analysis

Contains:
- Backend implementation details (6 sections with line numbers)
- Frontend implementation details (4 sections with line numbers)
- Detailed analysis of why each hack exists
- Scenario explanations with code examples
- Summary table of all code locations

**Read this if:** You're implementing changes and need exact line numbers

---

## Reading Paths

### Path 1: Quick Understanding (30 minutes)
→ Get the gist of what's wrong and why

1. CACHE_QUICK_REFERENCE.md (2 min)
2. CACHE_INVESTIGATION_SUMMARY.md (15 min)
3. Skim section summaries below (10 min)

**After this:** You know the problem and can discuss it

### Path 2: Implementation Ready (2 hours)
→ Understand the fix and be ready to implement

1. CACHE_INVESTIGATION_SUMMARY.md (15 min)
2. NORMALIZED_CACHE_ANALYSIS.md Part 6 only (30 min)
3. CACHE_CODE_REFERENCES.md (scan for your target sections) (15 min)
4. Look at actual code in IDE (40 min)

**After this:** You can start Phase 1 of the fix

### Path 3: Complete Mastery (1.5 hours)
→ Understand everything, be ready for any question

1. CACHE_QUICK_REFERENCE.md (2 min)
2. CACHE_INVESTIGATION_SUMMARY.md (15 min)
3. NORMALIZED_CACHE_ANALYSIS.md (all parts) (40 min)
4. CACHE_CODE_REFERENCES.md (detailed reading) (30 min)

**After this:** You're a cache system expert

---

## Key Sections by Topic

### If you want to understand...

**...the problem**
→ CACHE_QUICK_REFERENCE.md + CACHE_INVESTIGATION_SUMMARY.md (17 min total)

**...why it evolved this way**
→ NORMALIZED_CACHE_ANALYSIS.md Part 3 (20 min)

**...how to fix it**
→ NORMALIZED_CACHE_ANALYSIS.md Part 6 (30 min) + Part 5 (15 min)

**...the exact code locations**
→ CACHE_CODE_REFERENCES.md (30 min)

**...how other systems do it**
→ NORMALIZED_CACHE_ANALYSIS.md Part 4 (15 min)

**...what's the best starting point**
→ NORMALIZED_CACHE_ANALYSIS.md Part 9 (10 min)

**...technical debt estimates**
→ CACHE_INVESTIGATION_SUMMARY.md "Technical Debt Summary" table (5 min)

---

## Learning Objectives

After reading appropriate documents, you will know:

### Quick Understanding
- What is broken (File missing Identifiable)
- Where the hacks are (4 locations in deepMerge)
- Why they exist (virtual resource complexity)

### Implementation Ready
- How to implement File::Identifiable
- What to change in ResourceManager
- What to remove from deepMerge
- How to test the changes
- What to expect (50% complexity reduction)

### Complete Mastery
- How normalized caching works
- Why Apollo/Relay architecture is similar
- All 6 phases of the fix
- Every code location involved
- How to extend to other resource types

---

## Document Statistics

| Document | Size | Lines | Reading Time | Key Info |
|----------|------|-------|--------------|----------|
| CACHE_QUICK_REFERENCE.md | 4.7 KB | 160 | 2 min | Problem, 4 hacks, fix overview |
| CACHE_INVESTIGATION_SUMMARY.md | 6.0 KB | 210 | 15 min | Findings, Apollo/Relay, timeline |
| NORMALIZED_CACHE_ANALYSIS.md | 23 KB | 816 | 40 min | Deep analysis, 6-phase plan |
| CACHE_CODE_REFERENCES.md | 17 KB | 420 | 30 min | Line numbers, code analysis |
| **TOTAL** | **50.7 KB** | **1,606** | **1.5 hrs** | Complete knowledge base |

---

## What Each Document Delivers

### CACHE_QUICK_REFERENCE.md
Problem in one sentence
The 4 hacks with line numbers
Why they exist
2-week fix overview
Key insight about implementation vs architecture

### CACHE_INVESTIGATION_SUMMARY.md
6 key findings with explanations
Apollo/Relay comparison
Evolution timeline
Technical debt table with effort estimates
Recommended starting point with reasoning

### NORMALIZED_CACHE_ANALYSIS.md
Current state detailed (4 major components)
Intended design (what it was supposed to be)
Why custom logic was added (4 problems)
How other systems solve it
Clean solution design (5 phases)
6-phase migration path with pseudocode
Blockers and tech debt
Test cases for starting point

### CACHE_CODE_REFERENCES.md
Backend file-by-file analysis
Frontend file-by-file analysis
Exact line number references (100+)
Scenario explanations with code
Code summary table

---

## Implementation Timeline

Based on NORMALIZED_CACHE_ANALYSIS.md Part 6:

- **Phase 1 (1 day):** Implement File::Identifiable
- **Phase 2 (1 day):** Remove sd_path hack
- **Phase 3 (2 days):** Remove content UUID hacks
- **Phase 4 (1 day):** Fix single resource detection
- **Phase 5 (1 week):** Implement remaining traits (Tag, Device)
- **Phase 6 (later):** Add merge strategy metadata

**Total:** 2 weeks with existing team

---

## Code Files Referenced

### Backend
- `/core/src/domain/resource.rs` — Identifiable trait
- `/core/src/domain/resource_manager.rs` — Virtual resource mapping
- `/core/src/domain/file.rs` — File model
- `/core/src/infra/event/mod.rs` — Event definitions

### Frontend
- `/packages/ts-client/src/hooks/useNormalizedCache.ts` — Cache hook (466 lines, 4 hacks)

**Total:** 5 files, ~2000 lines analyzed

---

## Next Actions

### Immediate (Today)
1. Read CACHE_QUICK_REFERENCE.md (2 min)
2. Read CACHE_INVESTIGATION_SUMMARY.md (15 min)
3. Done! You understand the problem

### This Week
1. Read NORMALIZED_CACHE_ANALYSIS.md Part 6 (30 min)
2. Look at code locations in CACHE_CODE_REFERENCES.md
3. Review existing test coverage
4. Start Phase 1 implementation

### This Month
1. Implement all 6 phases
2. Add comprehensive tests
3. Remove all hacks
4. Extend to Tag, Device, etc.

---

## Investigation Metadata

- **Investigation Date:** November 11, 2025
- **Investigator:** Claude Code (Senior Software Engineer)
- **Codebase:** Spacedrive (Rust backend + TypeScript frontend)
- **Focus:** Normalized cache system (Identifiable trait pattern)
- **Status:** Complete and ready for implementation
- **Documents Created:** 4 (this index + 3 analysis docs)
- **Total Analysis:** ~1,600 lines, 50 KB
- **Code References:** 100+ line numbers
- **Implementation Ready:** Yes

---

## Quick Links

**Start here:** CACHE_QUICK_REFERENCE.md
**15-min read:** CACHE_INVESTIGATION_SUMMARY.md
**Deep dive:** NORMALIZED_CACHE_ANALYSIS.md
**Code reference:** CACHE_CODE_REFERENCES.md
**You are here:** CACHE_INVESTIGATION_INDEX.md (this file)

---

**Remember:** This is a well-designed system that hit complexity head-on. The fix is systematic, not architectural. Spacedrive is on the right track!
