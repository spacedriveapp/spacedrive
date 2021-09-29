use chrono::{offset::TimeZone, DateTime, NaiveDateTime, Utc};
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn system_time_to_date_time(
  system_time: io::Result<SystemTime>,
) -> Result<DateTime<Utc>, Box<dyn std::error::Error>> {
  // extract system time or resort to current time if failure
  let system_time = system_time.unwrap_or(SystemTime::now());
  let std_duration = system_time.duration_since(UNIX_EPOCH)?;
  let chrono_duration = chrono::Duration::from_std(std_duration)?;
  let unix = NaiveDateTime::from_timestamp(0, 0);
  let naive = unix + chrono_duration;
  let date_time: DateTime<Utc> = Utc.from_local_datetime(&naive).unwrap();
  Ok(date_time)
}
