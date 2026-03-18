//! Browser session management: reuse Chrome instances across requests.
//!
//! The SessionPool maintains persistent browser sessions, avoiding the
//! overhead of launching and killing Chrome for each request. Sessions
//! preserve cookies and browser context across page navigations.

use std::collections::HashMap;
use std::sync::RwLock;

/// A persistent browser session with its Chrome instance.
#[cfg(feature = "chrome")]
pub struct BrowserSession {
    /// The Chrome browser process.
    pub browser: headless_chrome::Browser,
    /// Logical context identifier for this session.
    pub context_id: String,
    /// When this session was created.
    pub created_at: std::time::Instant,
    /// When this session was last used.
    pub last_used: std::time::Instant,
    /// Number of requests served by this session.
    pub request_count: u64,
}

/// Pool of browser sessions, keyed by domain or session name.
pub struct SessionPool {
    #[cfg(feature = "chrome")]
    sessions: RwLock<HashMap<String, BrowserSession>>,
    #[cfg(not(feature = "chrome"))]
    sessions: RwLock<HashMap<String, ()>>,
    /// Maximum concurrent sessions.
    pub max_sessions: usize,
    /// Maximum session age in seconds before expiry.
    pub max_age_secs: u64,
}

impl Default for SessionPool {
    fn default() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            max_sessions: 5,
            max_age_secs: 300, // 5 minutes
        }
    }
}

impl SessionPool {
    pub fn new(max_sessions: usize, max_age_secs: u64) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            max_sessions,
            max_age_secs,
        }
    }

    /// Get the number of active sessions.
    pub fn active_count(&self) -> usize {
        self.sessions.read().map(|s| s.len()).unwrap_or(0)
    }

    /// Remove expired sessions.
    #[cfg(feature = "chrome")]
    pub fn cleanup_expired(&self) {
        if let Ok(mut sessions) = self.sessions.write() {
            let max_age = std::time::Duration::from_secs(self.max_age_secs);
            sessions.retain(|_, session| session.last_used.elapsed() < max_age);
        }
    }

    #[cfg(not(feature = "chrome"))]
    pub fn cleanup_expired(&self) {
        if let Ok(mut sessions) = self.sessions.write() {
            sessions.clear();
        }
    }

    /// Register a new session for the given key.
    ///
    /// Returns Ok if registered, Err if pool is full or key already exists.
    #[cfg(feature = "chrome")]
    pub fn register(
        &self,
        key: &str,
        launch_opts: headless_chrome::LaunchOptions,
    ) -> Result<(), String> {
        self.cleanup_expired();

        let mut sessions = self.sessions.write().map_err(|e| format!("lock: {}", e))?;

        if sessions.contains_key(key) {
            return Ok(()); // Already exists
        }

        if sessions.len() >= self.max_sessions {
            return Err(format!(
                "session pool full ({}/{})",
                sessions.len(),
                self.max_sessions
            ));
        }

        let browser = headless_chrome::Browser::new(launch_opts)
            .map_err(|e| format!("failed to launch Chrome: {}", e))?;

        let now = std::time::Instant::now();
        sessions.insert(
            key.to_string(),
            BrowserSession {
                browser,
                context_id: format!("ctx-{}", key),
                created_at: now,
                last_used: now,
                request_count: 0,
            },
        );

        Ok(())
    }

    /// Execute a function with a session's browser.
    #[cfg(feature = "chrome")]
    pub fn with_session<F, R>(&self, key: &str, f: F) -> Result<R, String>
    where
        F: FnOnce(&headless_chrome::Browser) -> Result<R, String>,
    {
        let mut sessions = self.sessions.write().map_err(|e| format!("lock: {}", e))?;
        let session = sessions
            .get_mut(key)
            .ok_or_else(|| format!("no session for key: {}", key))?;

        session.last_used = std::time::Instant::now();
        session.request_count += 1;

        f(&session.browser)
    }

    /// Check if a session exists and is still valid.
    pub fn has_session(&self, key: &str) -> bool {
        #[cfg(feature = "chrome")]
        {
            self.sessions
                .read()
                .map(|sessions| {
                    sessions.get(key).map_or(false, |s| {
                        s.last_used.elapsed() < std::time::Duration::from_secs(self.max_age_secs)
                    })
                })
                .unwrap_or(false)
        }
        #[cfg(not(feature = "chrome"))]
        {
            let _ = key;
            false
        }
    }

    /// Release (remove) a session.
    pub fn release(&self, key: &str) {
        if let Ok(mut sessions) = self.sessions.write() {
            sessions.remove(key);
        }
    }

    /// Shut down and remove all sessions.
    pub fn shutdown_all(&self) {
        if let Ok(mut sessions) = self.sessions.write() {
            sessions.clear();
        }
    }

    /// Get session statistics as an s-expression.
    pub fn status_sexp(&self) -> String {
        let count = self.active_count();
        format!(
            "(:active {} :max {} :max-age-s {})",
            count, self.max_sessions, self.max_age_secs
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_defaults() {
        let pool = SessionPool::default();
        assert_eq!(pool.max_sessions, 5);
        assert_eq!(pool.max_age_secs, 300);
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn pool_new_custom() {
        let pool = SessionPool::new(10, 600);
        assert_eq!(pool.max_sessions, 10);
        assert_eq!(pool.max_age_secs, 600);
    }

    #[test]
    fn pool_release_nonexistent_is_noop() {
        let pool = SessionPool::default();
        pool.release("nonexistent");
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn pool_shutdown_all() {
        let pool = SessionPool::default();
        pool.shutdown_all();
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn pool_has_session_returns_false_when_empty() {
        let pool = SessionPool::default();
        assert!(!pool.has_session("test"));
    }

    #[test]
    fn pool_status_sexp() {
        let pool = SessionPool::new(3, 120);
        let status = pool.status_sexp();
        assert!(status.contains(":active 0"));
        assert!(status.contains(":max 3"));
        assert!(status.contains(":max-age-s 120"));
    }
}
