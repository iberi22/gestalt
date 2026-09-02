//! Bus event replay filter module.
//!
//! Provides `ReplayFilter` for filtering timeline events by project, sequence number, and duration (`since`).

use chrono::{DateTime, Duration, Utc};

/// Filter options for replaying or querying bus events.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReplayFilter {
    /// Filter events belonging to a specific project.
    pub project: Option<String>,
    /// Filter events strictly newer than this sequence number (`seq > after_seq`).
    pub after_seq: Option<i64>,
    /// Filter events created within the last duration.
    pub since: Option<Duration>,
}

impl ReplayFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    pub fn with_after_seq(mut self, after_seq: i64) -> Self {
        self.after_seq = Some(after_seq);
        self
    }

    pub fn with_since(mut self, since: Duration) -> Self {
        self.since = Some(since);
        self
    }

    /// Check if an event matches the filter criteria.
    pub fn matches(
        &self,
        seq: Option<i64>,
        event_project: Option<&str>,
        ts_rfc3339: Option<&str>,
        now: DateTime<Utc>,
    ) -> bool {
        if let Some(after) = self.after_seq {
            let event_seq = seq.unwrap_or(0);
            if event_seq <= after {
                return false;
            }
        }

        if let Some(ref req_proj) = self.project {
            match event_project {
                Some(proj) if proj.eq_ignore_ascii_case(req_proj) => {},
                _ => return false,
            }
        }

        if let Some(since_dur) = self.since {
            let cutoff = now - since_dur;
            if let Some(ts_str) = ts_rfc3339 {
                if let Ok(event_time) = DateTime::parse_from_rfc3339(ts_str) {
                    if event_time.with_timezone(&Utc) < cutoff {
                        return false;
                    }
                }
            }
        }

        true
    }
}

/// Parse a human-readable duration string into `chrono::Duration`.
///
/// Supported formats: `30s`, `10m`, `1h`, `2d`.
pub fn parse_since_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Duration string cannot be empty".into());
    }

    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: i64 = num_str
        .parse()
        .map_err(|_| format!("Invalid duration number in '{}'", s))?;

    match unit {
        "s" | "S" => Ok(Duration::seconds(num)),
        "m" | "M" => Ok(Duration::minutes(num)),
        "h" | "H" => Ok(Duration::hours(num)),
        "d" | "D" => Ok(Duration::days(num)),
        _ => Err(format!(
            "Unknown duration unit '{}' in '{}'. Expected s, m, h, or d",
            unit, s
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_since_duration() {
        assert_eq!(parse_since_duration("30s").unwrap(), Duration::seconds(30));
        assert_eq!(parse_since_duration("10m").unwrap(), Duration::minutes(10));
        assert_eq!(parse_since_duration("1h").unwrap(), Duration::hours(1));
        assert_eq!(parse_since_duration("2d").unwrap(), Duration::days(2));
        assert!(parse_since_duration("invalid").is_err());
    }

    #[test]
    fn test_replay_filter_matches() {
        let now = Utc::now();
        let filter = ReplayFilter::new()
            .with_project("gara-g")
            .with_after_seq(10)
            .with_since(Duration::hours(1));

        let recent_ts = (now - Duration::minutes(30)).to_rfc3339();
        let old_ts = (now - Duration::hours(2)).to_rfc3339();

        assert!(filter.matches(Some(15), Some("gara-g"), Some(&recent_ts), now));
        assert!(!filter.matches(Some(5), Some("gara-g"), Some(&recent_ts), now));
        assert!(!filter.matches(Some(15), Some("other"), Some(&recent_ts), now));
        assert!(!filter.matches(Some(15), Some("gara-g"), Some(&old_ts), now));
    }
}
