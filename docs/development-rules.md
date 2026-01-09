# Generalized Development Rules

Rules derived from debugging scheduler and parallel scraping issues.

---

## 1. Async Task Management

- Never spawn fire-and-forget tasks for long-running operations
- Always provide cancellation mechanism (generation counter, CancellationToken)
- Track task identity to detect "stale" tasks

```rust
// Bad: No way to cancel or track
tokio::spawn(async move {
    loop {
        do_work().await;
        sleep(interval).await;
    }
});

// Good: Generation-based cancellation
let my_gen = GENERATION.fetch_add(1, Ordering::SeqCst);
tokio::spawn(async move {
    loop {
        if my_gen != GENERATION.load(Ordering::SeqCst) {
            break; // Superseded by newer task
        }
        do_work().await;
        sleep(interval).await;
    }
});
```

---

## 2. Configuration Validation

- Validate at load time with clear error messages
- Validate again at use site with safe defaults (`.max(1)`, `.unwrap_or()`)
- Zero is almost never a valid value for intervals/counts

```rust
// Bad: Trust user input
let interval_secs = config.interval_hours * 3600;

// Good: Defensive at every level
let interval_secs = config.interval_hours.max(1) * 3600;
```

---

## 3. Loops with External State

- If behavior should change based on config, read config INSIDE the loop
- Don't capture values at spawn time for long-lived tasks
- Consider: should this restart on config change?

```rust
// Bad: Captured at spawn time
let interval = config.interval;
tokio::spawn(async move {
    loop {
        sleep(interval).await;  // Never changes
    }
});

// Good: Read each iteration
tokio::spawn(async move {
    loop {
        let interval = get_config().interval;  // Hot reload
        sleep(interval).await;
    }
});
```

---

## 4. Work Distribution

- Prefer pull-based (shared queue) over push-based (pre-assigned chunks)
- Allows dynamic load balancing
- Failed items can be re-queued for retry

```rust
// Bad: Pre-assigned, uneven distribution
let chunks = locations.chunks(num_workers);
for chunk in chunks {
    spawn_worker(chunk);  // Worker stuck with slow locations
}

// Good: Shared queue, workers pull dynamically
let queue = Arc::new(Mutex::new(locations));
for _ in 0..num_workers {
    spawn_worker(queue.clone());  // Workers grab next available
}
```

---

## 5. Defensive Logging

- Log the SOURCE of actions, not just that they happened
- Include identifiers (generation, worker_id) in all related logs
- Makes debugging async race conditions tractable

```rust
// Bad: No context
println!("Scraping started");

// Good: Full context
println!("INFO: [Worker {}] Scraping started (triggered by: {})", 
    worker_id, trigger_source);
```

---

## 6. Idempotency

- "Start scheduler" called twice shouldn't create two schedulers
- Use state checks OR invalidation patterns (generation counter)

```rust
// Bad: Creates duplicate
pub fn start_scheduler() {
    tokio::spawn(scheduler_loop());
}

// Good: Single instance guaranteed
pub fn start_scheduler() {
    let gen = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    // Old tasks will see wrong generation and exit
    tokio::spawn(scheduler_loop(gen));
}
```

---

## 7. Minimum Viable Safety

Always enforce sensible bounds:

```rust
interval.max(1)       // Never allow 0 intervals
retries.min(10)       // Cap retry attempts  
timeout.max(1000)     // Minimum timeout
workers.clamp(1, 10)  // Bounded parallelism
```

---

## Summary Checklist

- [ ] Can this task be cancelled?
- [ ] What happens if config value is 0?
- [ ] Does the loop re-read mutable state?
- [ ] Is work distribution balanced?
- [ ] Can I trace where this action came from in logs?
- [ ] What happens if this function is called twice?
- [ ] Are all numeric inputs bounded?
