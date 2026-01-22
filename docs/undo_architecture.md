# Architecture 6.0: Undo/Redo System Specification

## 0. Purpose

Undo/Redo in axiomterm is not a "convenience feature"—it is **proof of state machine completeness**.

**Goals**:
- State transitions are **reversible**
- Macro/Action/ScreenOperation hierarchy remains **intact**
- Truth Layer (Rust) remains **unpolluted**
- Future-proof for Headless/Remote/Sync execution

---

## 1. Core Principles (Non-Negotiable)

### Rule 1: Undo Reverses Transitions, Not State

Undo does **not** save/restore Screen snapshots.  
Only **changes** are recorded.

❌ **Forbidden**:
- Screen snapshots
- Renderer state
- Lua state

✅ **Allowed**:
- State transitions
- Operation sequences
- Intent-preserving units

### Rule 2: Undo is Truth Layer Responsibility

- Undo/Redo completes **within ShellState**
- Lua/Renderer/Adapter are **unaware** of Undo
- Lua **cannot** invoke Undo directly
- Lua remains an **Intent generator**

---

## 2. Undo Granularity

**Conclusion**: Undo unit = **Action Transaction**

| Candidate | Rejected Because |
|-----------|------------------|
| ScreenOperation | Too low-level (implementation-dependent) |
| Frame | Renderer-dependent |
| Key Input | Meaning varies by mode/macro |
| **Action** | ✅ **Stable Intent representation** |

---

## 3. Action Transaction Definition

### What is a Transaction?

A **single user intent** represented as a group of Actions.

### Transaction Sources

1. Single Action (e.g., `MoveCursor`)
2. Macro-expanded Action sequence
3. Future Composite Actions

### Transaction Boundaries

| Event | Transaction Scope |
|-------|-------------------|
| Normal key input | 1 Action = 1 Transaction |
| Macro execution | **Entire Macro = 1 Transaction** |
| External process output | **Not undoable** |
| Config reload | **Not undoable** |

---

## 4. Undo/Redo Stack Responsibilities

### Undo Stack Contents

- Transaction ID
- Executed Action sequence
- Generated ScreenOperation sequence (order-preserving)
- Minimal auxiliary metadata

**Forbidden**: Screen copies

### Redo Stack Rules

- Generated **only** when Undo executes
- **Completely cleared** when new Transaction executes
- No parallel execution or future history

---

## 5. Reversal Model

### ScreenOperation Must Be Reversible

From Architecture 6.0 onward, all ScreenOperations must have logical pairs:

```
forward() ⇄ reverse()
```

**Examples** (conceptual):

```
PushLine ⇄ PopLine
UpdateLine(old → new) ⇄ UpdateLine(new → old)
CursorMove(from → to) ⇄ CursorMove(to → from)
```

**Responsibility**: `ShellState`, not `Renderer`

---

## 6. Macro and Undo Relationship

### Absolute Rule

**Macro = 1 Undo**

- Action count within macro is **irrelevant**
- 100-action macro = **1 Undo**
- Preserves user **intent**

### Metrics Relationship

- `MacroMetrics` does **not** affect Undo
- Undo/Redo does **not** generate Metrics (observation-only)

---

## 7. Mode/Lifecycle Consistency

### Undo Execution Behavior

- If Mode was **Destroyed** → Fallback
- If Mode definition **changed** (via reload) → Honor original Action semantics

**Key**: Undo is **inverse application of transitions**, not replay of history.

---

## 8. Lua Constraints (Reconfirmed)

### Lua Cannot:

- Manipulate Undo Stack
- Manipulate Redo Stack
- Split/merge Transactions
- Directly reference State

### Lua Can:

- Return Actions
- Compose Macros

---

## 9. Future Extensions

This design naturally supports:

- Timeline scrubbing
- Network sync (Operation Stream)
- Collaborative editing
- Deterministic replay
- Headless Undo testing

---

## 10. Implementation Phases (Reference)

| Phase | Content |
|-------|---------|
| 8-1 | Reversible ScreenOperation definition |
| 8-2 | Transaction recording |
| 8-3 | Undo Stack implementation |
| 8-4 | Redo Stack |
| 8-5 | Undo/Redo Action addition |

**Note**: This document is **design-only**. Implementation is a separate phase.

---

## Final Declaration

This Undo Architecture elevates axiomterm from  
**"a convenient terminal"** to  
**a formally correct state machine**.

### Next Steps

Choose one:

1. **Phase 8-1**: Reversible ScreenOperation design
2. **External Process Boundary** definition with Undo in mind
