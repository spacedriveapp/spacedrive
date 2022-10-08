```rust
pub fn update_with_timestamp(&self, timestamp: &Timestamp) -> Result<(), String> {
    let mut now = (self.clock)();
    now.0 &= LMASK;
    let msg_time = timestamp.get_time();
    if *msg_time > now && *msg_time - now > self.delta {
        let err_msg = format!(
            "incoming timestamp from {} exceeding delta {}ms is rejected: {} vs. now: {}",
            timestamp.get_id(),
            self.delta.to_duration().as_millis(),
            msg_time,
            now
        );
        warn!("{}", err_msg);
        Err(err_msg)
    } else {
        let mut last_time = lock!(self.last_time);
        let max_time = cmp::max(cmp::max(now, *msg_time), *last_time);
        if max_time == now {
            *last_time = now;
        } else if max_time == *msg_time {
            *last_time = *msg_time + 1;
        } else {
            *last_time += 1;
        }
        Ok(())
    }
}
```

```javascript
Timestamp.recv = function (msg) {
	if (!clock) {
		return null;
	}

	var now = Date.now();

	var msg_time = msg.millis();
	var msg_time = msg.counter();

	if (msg_time - now > config.maxDrift) {
		throw new Timestamp.ClockDriftError();
	}

	var last_time = clock.timestamp.millis();
	var last_time = clock.timestamp.counter();

	var max_time = Math.max(Math.max(last_time, now), msg_time);

	var last_time =
		max_time === last_time && lNew === msg_time
			? Math.max(last_time, msg_time) + 1
			: max_time === last_time
			? last_time + 1
			: max_time === msg_time
			? msg_time + 1
			: 0;

	// 3.
	if (max_time - phys > config.maxDrift) {
		throw new Timestamp.ClockDriftError();
	}
	if (last_time > MAX_COUNTER) {
		throw new Timestamp.OverflowError();
	}

	clock.timestamp.setMillis(max_time);
	clock.timestamp.setCounter(last_time);

	return new Timestamp(clock.timestamp.millis(), clock.timestamp.counter(), clock.timestamp.node());
};
```
