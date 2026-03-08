# harmonia-cron-scheduler

## Purpose

In-process cron/heartbeat scheduler. Manages named jobs with interval-based scheduling and reports which jobs are due for execution at any given time.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_cron_scheduler_version` | `() -> *const c_char` | Version string |
| `harmonia_cron_scheduler_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_cron_scheduler_add_job` | `(name: *const c_char, interval_secs: i32) -> i32` | Register a recurring job |
| `harmonia_cron_scheduler_due_jobs` | `(now: i64) -> *mut c_char` | Get jobs due at timestamp (S-expression list) |
| `harmonia_cron_scheduler_reset` | `() -> i32` | Clear all jobs |
| `harmonia_cron_scheduler_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_cron_scheduler_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

No env vars. All state is in-memory (resets on process restart).

## Usage from Lisp

```lisp
(cffi:foreign-funcall "harmonia_cron_scheduler_add_job"
  :string "memory-sync" :int 300 :int)  ;; every 5 minutes
(cffi:foreign-funcall "harmonia_cron_scheduler_add_job"
  :string "health-check" :int 60 :int)  ;; every minute
;; In main loop:
(let ((due (cffi:foreign-funcall "harmonia_cron_scheduler_due_jobs"
             :int64 (get-universal-time) :pointer)))
  (dolist (job (parse-sexp due)) (run-job job)))
```

## Self-Improvement Notes

- Jobs stored in a `Vec<Job>` behind `OnceLock<RwLock>`. Each job tracks `last_run` timestamp.
- `due_jobs` checks `now >= last_run + interval` and updates `last_run` for returned jobs.
- No persistence; if the process restarts, jobs must be re-registered.
- To add persistence: serialize job list to config-store or a file.
- To add cron expressions: replace `interval_secs` with a cron parser crate.
