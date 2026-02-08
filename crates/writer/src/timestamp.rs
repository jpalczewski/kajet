use anyhow::Result;
use kajet_core::config::TimestampConfig;

/// Format the current time according to the timestamp config.
pub fn format_now(config: &TimestampConfig) -> Result<String> {
    let format_str = match config.format.as_str() {
        "iso8601" => "%Y-%m-%dT%H:%M:%S",
        "date_only" => "%Y-%m-%d",
        "obsidian" => "%Y-%m-%d %H:%M",
        other => other, // custom strftime
    };

    match config.timezone.as_str() {
        "local" => {
            let now = chrono::Local::now();
            Ok(now.format(format_str).to_string())
        }
        "UTC" | "utc" => {
            let now = chrono::Utc::now();
            Ok(now.format(format_str).to_string())
        }
        tz_name => {
            let tz: chrono_tz::Tz = tz_name
                .parse()
                .map_err(|_| anyhow::anyhow!("Unknown timezone: '{tz_name}'"))?;
            let now = chrono::Utc::now().with_timezone(&tz);
            Ok(now.format(format_str).to_string())
        }
    }
}

/// Validate that a timezone string is recognizable.
pub fn validate_timezone(tz: &str) -> Result<()> {
    match tz {
        "local" | "UTC" | "utc" => Ok(()),
        other => {
            let _: chrono_tz::Tz = other
                .parse()
                .map_err(|_| anyhow::anyhow!("Unknown timezone: '{other}'"))?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> TimestampConfig {
        TimestampConfig::default()
    }

    #[test]
    fn format_now_local() {
        let result = format_now(&default_config()).unwrap();
        // ISO 8601-like format
        assert!(result.contains('T'), "Should contain T separator: {result}");
        assert!(result.len() >= 19, "Should be at least 19 chars: {result}");
    }

    #[test]
    fn format_now_utc() {
        let mut config = default_config();
        config.timezone = "UTC".into();
        let result = format_now(&config).unwrap();
        assert!(result.contains('T'));
    }

    #[test]
    fn format_now_iana_timezone() {
        let mut config = default_config();
        config.timezone = "Europe/Warsaw".into();
        let result = format_now(&config).unwrap();
        assert!(result.contains('T'));
    }

    #[test]
    fn format_now_date_only() {
        let mut config = default_config();
        config.format = "date_only".into();
        let result = format_now(&config).unwrap();
        assert!(!result.contains('T'));
        assert_eq!(result.len(), 10); // YYYY-MM-DD
    }

    #[test]
    fn format_now_obsidian() {
        let mut config = default_config();
        config.format = "obsidian".into();
        let result = format_now(&config).unwrap();
        assert!(result.contains(' '));
        assert!(!result.contains('T'));
    }

    #[test]
    fn format_now_invalid_timezone() {
        let mut config = default_config();
        config.timezone = "Fake/Zone".into();
        assert!(format_now(&config).is_err());
    }

    #[test]
    fn validate_known_timezones() {
        assert!(validate_timezone("local").is_ok());
        assert!(validate_timezone("UTC").is_ok());
        assert!(validate_timezone("Europe/Warsaw").is_ok());
        assert!(validate_timezone("America/New_York").is_ok());
        assert!(validate_timezone("Fake/Zone").is_err());
    }
}
