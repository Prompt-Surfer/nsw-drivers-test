# Debugging Session Learnings

## Issues Encountered

| Issue | Root Cause | Fix |
|-------|-----------|-----|
| Scheduler infinite loop | `interval_hours = 0` → `0 * 3600 = 0` seconds sleep | Enforce `.max(1)` minimum |
| Multiple scheduler tasks | No tracking of spawned async tasks | Generation counter pattern |
| Interval not updating | Fixed value at startup vs dynamic state read | Read from state each loop iteration |
| Scheduler spam messages | Same as infinite loop - 0 interval | Same fix |
| Worker failures | No retry on login errors like `checkTerms` not found | Re-login logic + re-queue failed work |

---

## Architectural Issues Identified

### 1. Async Task Lifecycle Management

- Spawned `tokio::spawn` tasks had no way to be cancelled or tracked
- Boolean flags (`enabled`) aren't sufficient - task might be sleeping when flag changes
- **Solution**: Generation counters or cancellation tokens

### 2. Configuration Validation

- Settings were used directly without validation (0 hours accepted)
- Edge cases caused catastrophic behavior (infinite loop)
- **Solution**: Validate at boundary (load time) AND at use site

### 3. Mutable State in Long-Running Tasks

- Scheduler captured `interval_secs` at spawn time, never re-read
- UI changes didn't propagate to running task
- **Solution**: Read shared state each iteration for "hot reload" behavior

### 4. Work Distribution

- Pre-chunking work to workers caused uneven load if some locations were faster
- **Solution**: Shared queue with dynamic work pulling

---

## Development Process Issues

### 1. Testing with Edge Cases

- Setting `interval_hours: 0` was never tested
- Should have unit tests for boundary values (0, 1, MAX)

### 2. Logging for Debugging

- Initially hard to trace WHERE scraping was triggered from
- Added source logging (`from UI button`, `from scheduler gen X`)
- **Should have been there from the start**

### 3. Settings File in Git

- `settings.yaml` with `interval_hours: 0` was committed
- Caused bug to persist across restarts
- Consider: `.example` files + `.gitignore` actual config

---

## Key Takeaway

> **The bugs weren't in complex logic - they were in missing boundary checks and lifecycle management of async tasks.** Most production bugs come from edge cases (0, null, timeout) and resource lifecycle (task still running, file still open), not from core algorithm errors.
