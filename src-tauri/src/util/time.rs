use chrono::{offset::TimeZone, DateTime, NaiveDateTime, Utc};
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn system_time_to_date_time(system_time: SystemTime) -> io::Result<DateTime<Utc>> {
  let std_duration = system_time.duration_since(UNIX_EPOCH).unwrap();
  let chrono_duration = chrono::Duration::from_std(std_duration).unwrap();
  let unix = NaiveDateTime::from_timestamp(0, 0);
  let naive = unix + chrono_duration;
  let date_time: DateTime<Utc> = Utc.from_local_datetime(&naive).unwrap();
  Ok(date_time)
}
