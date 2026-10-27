use etcd_client::{Client, Member, MemberAddOptions};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtcdMember {
    pub id: u64,
    pub name: String,
    pub peer_urls: Vec<String>,
    pub client_urls: Vec<String>,
    pub is_learner: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinPlan {
    pub member_id: u64,
    pub initial_cluster: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MembershipError {
    #[error("etcd request failed: {0}")]
    Etcd(#[from] etcd_client::Error),
    #[error("etcd member {0} not found")]
    NotFound(u64),
    #[error("removal would leave {0} voting members, only 1 or at least 3 are allowed")]
    InvalidQuorum(usize),
    #[error("etcd returned no member for the added learner")]
    MissingMember,
}

impl From<&Member> for EtcdMember {
    fn from(member: &Member) -> Self {
        Self {
            id: member.id(),
            name: member.name().to_string(),
            peer_urls: member.peer_urls().to_vec(),
            client_urls: member.client_urls().to_vec(),
            is_learner: member.is_learner(),
        }
    }
}

pub fn voter_count(members: &[EtcdMember]) -> usize {
    members.iter().filter(|member| !member.is_learner).count()
}

pub fn has_loopback_voter(members: &[EtcdMember]) -> bool {
    members
        .iter()
        .filter(|member| !member.is_learner)
        .flat_map(|member| member.peer_urls.iter())
        .any(|url| url.contains("//127.0.0.1:") || url.contains("//localhost:"))
}

pub fn stale_learner_ids(members: &[EtcdMember], known_urls: &HashSet<String>) -> Vec<u64> {
    members
        .iter()
        .filter(|member| member.is_learner)
        .filter(|member| !member.peer_urls.iter().any(|url| known_urls.contains(url)))
        .map(|member| member.id)
        .collect()
}

pub fn check_removal(members: &[EtcdMember], id: u64) -> Result<(), MembershipError> {
    let member = members
        .iter()
        .find(|member| member.id == id)
        .ok_or(MembershipError::NotFound(id))?;
    if member.is_learner {
        return Ok(());
    }
    let remaining = voter_count(members) - 1;
    if remaining == 0 || remaining == 2 {
        return Err(MembershipError::InvalidQuorum(remaining));
    }
    Ok(())
}

pub fn initial_cluster(members: &[EtcdMember], new_id: u64, new_name: &str) -> String {
    members
        .iter()
        .flat_map(|member| {
            let name = if member.id == new_id {
                new_name
            } else {
                member.name.as_str()
            };
            member
                .peer_urls
                .iter()
                .filter(move |_| !name.is_empty())
                .map(move |url| format!("{name}={url}"))
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub async fn list_members(client: &mut Client) -> Result<Vec<EtcdMember>, MembershipError> {
    let response = client.member_list().await?;
    Ok(response.members().iter().map(EtcdMember::from).collect())
}

pub async fn add_learner(
    client: &mut Client,
    name: &str,
    peer_url: &str,
) -> Result<JoinPlan, MembershipError> {
    let existing = list_members(client).await?;
    if let Some(member) = existing
        .iter()
        .find(|member| member.peer_urls.iter().any(|url| url == peer_url))
    {
        return Ok(JoinPlan {
            member_id: member.id,
            initial_cluster: initial_cluster(&existing, member.id, name),
        });
    }

    let response = client
        .member_add(
            [peer_url.to_string()],
            Some(MemberAddOptions::new().with_is_learner()),
        )
        .await?;
    let added = response
        .member()
        .ok_or(MembershipError::MissingMember)?
        .id();
    let members: Vec<EtcdMember> = response
        .member_list()
        .iter()
        .map(EtcdMember::from)
        .collect();

    Ok(JoinPlan {
        member_id: added,
        initial_cluster: initial_cluster(&members, added, name),
    })
}

pub async fn promote(client: &mut Client, id: u64) -> Result<(), MembershipError> {
    client.member_promote(id).await?;
    Ok(())
}

pub async fn remove(client: &mut Client, id: u64) -> Result<(), MembershipError> {
    let members = list_members(client).await?;
    check_removal(&members, id)?;
    client.member_remove(id).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(id: u64, name: &str, url: &str, is_learner: bool) -> EtcdMember {
        EtcdMember {
            id,
            name: name.to_string(),
            peer_urls: vec![url.to_string()],
            client_urls: Vec::new(),
            is_learner,
        }
    }

    fn three_voters() -> Vec<EtcdMember> {
        vec![
            member(1, "a", "http://10.100.0.1:2380", false),
            member(2, "b", "http://10.100.0.2:2380", false),
            member(3, "c", "http://10.100.0.3:2380", false),
        ]
    }

    #[tokio::test]
    #[ignore]
    async fn learner_roundtrip_against_real_etcd() {
        let endpoint = std::env::var("ETCD_ENDPOINTS").expect("ETCD_ENDPOINTS");
        let mut client = Client::connect([endpoint.as_str()], None).await.unwrap();
        let peer_url = "http://10.100.250.1:2380";

        let plan = add_learner(&mut client, "test-node", peer_url)
            .await
            .unwrap();
        let again = add_learner(&mut client, "test-node", peer_url)
            .await
            .unwrap();
        let members = list_members(&mut client).await.unwrap();
        remove(&mut client, plan.member_id).await.unwrap();

        assert_eq!(plan.member_id, again.member_id);
        assert!(plan
            .initial_cluster
            .contains("test-node=http://10.100.250.1:2380"));
        assert!(members
            .iter()
            .any(|member| member.id == plan.member_id && member.is_learner));
        let after = list_members(&mut client).await.unwrap();
        assert!(after.iter().all(|member| member.id != plan.member_id));
    }

    #[test]
    fn removal_from_three_voters_is_refused() {
        assert!(matches!(
            check_removal(&three_voters(), 3),
            Err(MembershipError::InvalidQuorum(2))
        ));
    }

    #[test]
    fn removal_from_four_voters_is_allowed() {
        let mut members = three_voters();
        members.push(member(4, "d", "http://10.100.0.4:2380", false));
        assert!(check_removal(&members, 4).is_ok());
    }

    #[test]
    fn last_voter_cannot_be_removed() {
        let members = vec![member(1, "a", "http://10.100.0.1:2380", false)];
        assert!(matches!(
            check_removal(&members, 1),
            Err(MembershipError::InvalidQuorum(0))
        ));
    }

    #[test]
    fn learner_can_always_be_removed() {
        let mut members = three_voters();
        members.push(member(4, "", "http://10.100.0.4:2380", true));
        assert!(check_removal(&members, 4).is_ok());
    }

    #[test]
    fn loopback_voter_is_detected_but_learner_is_ignored() {
        let mut members = three_voters();
        members.push(member(4, "", "http://127.0.0.1:2380", true));
        assert!(!has_loopback_voter(&members));
        members[0] = member(1, "a", "http://127.0.0.1:2380", false);
        assert!(has_loopback_voter(&members));
    }

    #[test]
    fn stale_learners_exclude_the_joining_url() {
        let mut members = three_voters();
        members.push(member(4, "", "http://10.100.0.4:2380", true));
        members.push(member(5, "", "http://10.100.0.5:2380", true));
        members.push(member(6, "named", "http://10.100.0.6:2380", true));
        let known = HashSet::from([
            "http://10.100.0.5:2380".to_string(),
            "http://10.100.0.6:2380".to_string(),
        ]);
        assert_eq!(stale_learner_ids(&members, &known), vec![4]);
    }

    #[test]
    fn unknown_member_is_not_found() {
        assert!(matches!(
            check_removal(&three_voters(), 99),
            Err(MembershipError::NotFound(99))
        ));
    }

    #[test]
    fn initial_cluster_names_the_new_member_and_skips_unnamed_others() {
        let members = vec![
            member(1, "a", "http://10.100.0.1:2380", false),
            member(2, "", "http://10.100.0.2:2380", true),
            member(3, "", "http://10.100.0.3:2380", true),
        ];
        assert_eq!(
            initial_cluster(&members, 3, "c"),
            "a=http://10.100.0.1:2380,c=http://10.100.0.3:2380"
        );
    }
}
