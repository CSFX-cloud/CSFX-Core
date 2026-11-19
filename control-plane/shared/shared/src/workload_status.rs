pub const ACTIVE_STATUSES: [&str; 5] = ["scheduled", "pulling", "creating", "starting", "running"];
pub const LIVE_STATUSES: [&str; 6] = [
    "pending",
    "scheduled",
    "pulling",
    "creating",
    "starting",
    "running",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_statuses_are_pending_plus_the_active_ones() {
        assert_eq!(LIVE_STATUSES[0], "pending");
        assert_eq!(LIVE_STATUSES[1..], ACTIVE_STATUSES);
    }
}
