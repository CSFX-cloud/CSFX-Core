use chrono::NaiveDateTime;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: Uuid,
    pub branch: String,
    pub created_at: NaiveDateTime,
    pub ready: bool,
    pub bound_to_workload: bool,
    pub has_payload: bool,
}

pub fn expired(candidates: &[Candidate], current: Option<Uuid>, keep: usize) -> Vec<Uuid> {
    let mut stored: Vec<&Candidate> = candidates
        .iter()
        .filter(|candidate| candidate.ready && candidate.has_payload)
        .collect();
    stored.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let mut retained: HashSet<Uuid> = stored
        .iter()
        .take(keep)
        .map(|candidate| candidate.id)
        .collect();
    retained.extend(current);
    retained.extend(
        candidates
            .iter()
            .filter(|candidate| candidate.bound_to_workload)
            .map(|candidate| candidate.id),
    );
    let mut seen_branches = HashSet::new();
    for candidate in &stored {
        if seen_branches.insert(candidate.branch.as_str()) {
            retained.insert(candidate.id);
        }
    }
    stored
        .into_iter()
        .map(|candidate| candidate.id)
        .filter(|id| !retained.contains(id))
        .collect()
}

pub fn image_tag(image_ref: &str) -> Option<&str> {
    let (_, tag) = image_ref.rsplit_once(':')?;
    (!tag.is_empty() && !tag.contains('/')).then_some(tag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn candidate(id: u128, branch: &str, day: u32) -> Candidate {
        Candidate {
            id: Uuid::from_u128(id),
            branch: branch.to_string(),
            created_at: NaiveDate::from_ymd_opt(2026, 11, day)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            ready: true,
            bound_to_workload: false,
            has_payload: true,
        }
    }

    #[test]
    fn only_the_newest_deployments_are_kept() {
        let all: Vec<Candidate> = (1..=6)
            .map(|day| candidate(u128::from(day), "main", day))
            .collect();
        let dropped = expired(&all, None, 2);
        assert_eq!(dropped.len(), 4);
        assert!(!dropped.contains(&Uuid::from_u128(6)));
        assert!(!dropped.contains(&Uuid::from_u128(5)));
    }

    #[test]
    fn current_workload_bound_and_newest_per_branch_survive() {
        let mut all = vec![
            candidate(1, "main", 1),
            candidate(2, "main", 2),
            candidate(3, "feature", 3),
            candidate(4, "main", 4),
        ];
        all[0].bound_to_workload = true;
        let dropped = expired(&all, Some(Uuid::from_u128(2)), 0);
        assert_eq!(dropped, Vec::<Uuid>::new());
        let dropped = expired(&all, None, 0);
        assert_eq!(dropped, vec![Uuid::from_u128(2)]);
    }

    #[test]
    fn unfinished_and_empty_deployments_are_never_selected() {
        let mut building = candidate(1, "main", 1);
        building.ready = false;
        let mut empty = candidate(2, "main", 2);
        empty.has_payload = false;
        assert!(expired(&[building, empty], None, 0).is_empty());
    }

    #[test]
    fn tags_are_read_from_image_references() {
        assert_eq!(
            image_tag("sites.csfx.invalid/abc:12345678"),
            Some("12345678")
        );
        assert_eq!(image_tag("host:5060/abc"), None);
        assert_eq!(image_tag("abc"), None);
    }
}
