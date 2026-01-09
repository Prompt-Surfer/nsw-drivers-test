# Frameworks & Approaches for Bug Prevention

Analysis of tools and patterns that would have caught the scheduler/worker bugs earlier.

---

## 1. Type System Enforcement

**Rust's strength, but underutilized in this project.**

```rust
// Instead of: interval_hours: u32
// Use:
use std::num::NonZeroU32;

struct SchedulerConfig {
    interval_hours: NonZeroU32,  // Compile-time guarantee: never 0
}

// Or custom newtype:
struct Hours(u32);
impl Hours {
    pub fn new(h: u32) -> Self {
        Self(h.max(1))  // Enforced at construction
    }
}
```

**Would have caught**: Infinite loop from `interval = 0`

---

## 2. Property-Based Testing

**Tools**: `proptest`, `quickcheck`

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn scheduler_never_spins(interval in 0u32..1000) {
        let sleep_duration = calculate_sleep(interval);
        prop_assert!(sleep_duration >= Duration::from_secs(3600));
    }
    
    #[test]
    fn worker_handles_any_location_count(count in 0usize..1000) {
        let locations: Vec<String> = (0..count).map(|i| i.to_string()).collect();
        let result = distribute_work(locations, 3);
        prop_assert!(result.is_ok());
    }
}
```

**Would have caught**: Edge case `interval = 0` automatically tested

---

## 3. Actor Model

**Tools**: `actix`, `bastion`, `xactor`

```rust
use actix::prelude::*;

// Actors have supervised lifecycles - no orphaned tasks
impl Actor for Scheduler {
    type Context = Context<Self>;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        // Only ONE scheduler actor exists
        ctx.run_interval(self.interval, |act, _ctx| {
            act.trigger_scrape();
        });
    }
}

// To change interval: send message, actor handles it
scheduler_addr.do_send(UpdateInterval(new_hours));
```

**Would have caught**: Multiple scheduler tasks, lifecycle management

---

## 4. State Machines

**Tools**: `enum` states, `sm` crate, `state_machine_future`

```rust
enum SchedulerState {
    Stopped,
    Running { generation: u64, interval: Duration },
    Paused,
}

impl SchedulerState {
    fn start(&self, interval: Duration) -> Result<Self, Error> {
        match self {
            Self::Running { .. } => Err(Error::AlreadyRunning),
            _ => Ok(Self::Running { 
                generation: next_gen(), 
                interval 
            })
        }
    }
    
    fn stop(&self) -> Result<Self, Error> {
        match self {
            Self::Stopped => Err(Error::AlreadyStopped),
            _ => Ok(Self::Stopped)
        }
    }
}
```

**Would have caught**: Invalid state transitions, multiple starts

---

## 5. Supervision Trees

**Tools**: `bastion`, `tokio-graceful`, Erlang/Elixir OTP

```rust
use bastion::prelude::*;

// Supervisor automatically:
// - Restarts failed tasks with backoff
// - Ensures only one instance runs
// - Propagates shutdown signals

Bastion::supervisor(|supervisor| {
    supervisor
        .children(|children| {
            children
                .with_redundancy(1)  // Only one instance
                .with_restart_strategy(RestartStrategy::default())
                .with_exec(move |ctx| async move {
                    scheduler_loop(ctx).await
                })
        })
})
```

**Would have caught**: Task lifecycle, restart loops, orphaned tasks

---

## 6. Reactive State

**Tools**: `leptos` signals (used but not fully leveraged), `tokio::watch`

```rust
use tokio::sync::watch;

// Config changes automatically propagate
let (tx, rx) = watch::channel(SchedulerConfig::default());

// In scheduler task:
tokio::spawn(async move {
    loop {
        let config = rx.borrow().clone();
        // Automatically gets latest config
        tokio::time::sleep(config.interval).await;
        do_scrape().await;
    }
});

// To update (from UI):
tx.send(new_config).unwrap();
```

**Would have caught**: Interval not updating in real-time

---

## 7. Design by Contract

**Tools**: `contracts` crate, `pre`/`post` macros

```rust
use contracts::*;

#[requires(interval_hours > 0, "interval must be positive")]
#[ensures(ret.is_ok() -> "scheduler task spawned")]
pub fn start_scheduler(interval_hours: u32) -> Result<(), Error> {
    // Function body...
}

#[invariant(self.workers.len() <= self.max_workers)]
impl WorkerPool {
    // Methods maintain invariant...
}
```

**Would have caught**: Zero interval at function boundary

---

## Summary Table

| Bug | Framework/Approach | How It Helps |
|-----|-------------------|--------------|
| `interval = 0` infinite loop | `NonZeroU32`, Property Testing, DbC | Type-level or test-level enforcement |
| Multiple scheduler tasks | Actor Model, State Machine | Single instance guarantee, explicit transitions |
| Interval not updating | Reactive State, Message Passing | Changes propagate automatically |
| Task lifecycle issues | Supervision Trees | Managed spawning, graceful shutdown |
| Worker failures | Circuit Breakers, Retry Policies | Structured error handling |

---

## Recommended Stack Changes

### Immediate (Low Effort)

1. Add `NonZeroU32` for interval config
2. Add `proptest` for edge case testing
3. Use `tokio::sync::watch` for config propagation

### Medium-Term

1. Consider `actix` or `bastion` for background task management
2. Implement explicit state machine for scheduler states
3. Add circuit breaker for external API calls

### Long-Term (Architecture)

If complexity grows, consider:

- **Elixir/Phoenix**: Built-in supervision trees, LiveView for real-time UI
- **Temporal.io**: Workflow orchestration with built-in retry/timeout handling
- **Actor frameworks**: Natural fit for concurrent scraping with supervision

---

## The Meta-Lesson

> **Rust's type system is powerful but we used `u32` when we needed `NonZeroU32`.** 
> 
> The bugs weren't "Rust bugs" - they were "we didn't leverage Rust's strengths" bugs. The language gave us tools; we just didn't use them.

The pattern repeats across languages:
- TypeScript: Use `branded types` not `string`
- Python: Use `pydantic` validation not raw dicts
- Go: Use custom types with constructor validation

**The framework isn't magic - it's about encoding invariants in types and tests.**
