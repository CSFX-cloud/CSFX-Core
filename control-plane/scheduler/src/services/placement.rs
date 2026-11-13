use std::collections::HashSet;
use uuid::Uuid;

use crate::models::workload::{AgentResources, TopologySpread};

const SPREAD_PENALTY: f64 = 30.0;
const PREFERRED_BONUS: f64 = 10.0;
const BALANCE_WEIGHT: f64 = 0.5;
const ZONE_PENALTY: f64 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    Spread,
    Binpack,
}

impl Strategy {
    pub fn from_name(name: &str) -> Self {
        match name {
            "binpack" => Strategy::Binpack,
            _ => Strategy::Spread,
        }
    }
}

pub struct Demand<'a> {
    pub cpu_millicores: i32,
    pub memory_bytes: i64,
    pub disk_bytes: i64,
    pub requires_kvm: bool,
    pub spread_key: Option<&'a str>,
    pub topology_spread: TopologySpread,
    pub preferred_agents: Option<&'a HashSet<Uuid>>,
}

pub fn pick(demand: &Demand, agents: &[AgentResources], strategy: Strategy) -> Option<Uuid> {
    let candidates: Vec<&AgentResources> =
        agents.iter().filter(|agent| fits(demand, agent)).collect();
    let lightest_zone = candidates
        .iter()
        .map(|agent| zone_replicas(demand, agents, &agent.zone))
        .min()
        .unwrap_or(0);
    candidates
        .into_iter()
        .filter(|agent| {
            demand.topology_spread != TopologySpread::Hard
                || zone_replicas(demand, agents, &agent.zone) == lightest_zone
        })
        .map(|agent| (agent.agent_id, score(demand, agents, agent, strategy)))
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(agent_id, _)| agent_id)
}

fn zone_replicas(demand: &Demand, agents: &[AgentResources], zone: &str) -> u32 {
    let Some(key) = demand.spread_key else {
        return 0;
    };
    agents
        .iter()
        .filter(|agent| agent.zone == zone)
        .filter_map(|agent| agent.spread_counts.get(key))
        .sum()
}

pub fn commit(agents: &mut [AgentResources], agent_id: Uuid, demand: &Demand) {
    let Some(agent) = agents.iter_mut().find(|agent| agent.agent_id == agent_id) else {
        return;
    };
    agent.free_cpu_millicores -= demand.cpu_millicores;
    agent.free_memory_bytes -= demand.memory_bytes;
    agent.free_disk_bytes -= demand.disk_bytes;
    if let Some(key) = demand.spread_key {
        *agent.spread_counts.entry(key.to_string()).or_default() += 1;
    }
}

fn fits(demand: &Demand, agent: &AgentResources) -> bool {
    agent.free_cpu_millicores >= demand.cpu_millicores
        && agent.free_memory_bytes >= demand.memory_bytes
        && agent.free_disk_bytes >= demand.disk_bytes
        && (!demand.requires_kvm || agent.kvm_capable)
}

fn utilization(allocatable: f64, free_after: f64) -> f64 {
    if allocatable <= 0.0 {
        return 1.0;
    }
    1.0 - free_after / allocatable
}

fn score(
    demand: &Demand,
    agents: &[AgentResources],
    agent: &AgentResources,
    strategy: Strategy,
) -> f64 {
    let cpu = utilization(
        agent.allocatable_cpu_millicores as f64,
        (agent.free_cpu_millicores - demand.cpu_millicores) as f64,
    );
    let memory = utilization(
        agent.allocatable_memory_bytes as f64,
        (agent.free_memory_bytes - demand.memory_bytes) as f64,
    );
    let mean = (cpu + memory) / 2.0;
    let packing = match strategy {
        Strategy::Spread => 1.0 - mean,
        Strategy::Binpack => mean,
    } * 100.0;
    let balance = (1.0 - (cpu - memory).abs()) * 100.0;
    let colocated = demand
        .spread_key
        .and_then(|key| agent.spread_counts.get(key))
        .copied()
        .unwrap_or(0) as f64;
    let preferred = demand
        .preferred_agents
        .is_some_and(|agents| agents.contains(&agent.agent_id));

    let zone_penalty = if demand.topology_spread == TopologySpread::Soft {
        zone_replicas(demand, agents, &agent.zone) as f64 * ZONE_PENALTY
    } else {
        0.0
    };

    packing * (1.0 - BALANCE_WEIGHT) + balance * BALANCE_WEIGHT
        - colocated * SPREAD_PENALTY
        - zone_penalty
        + if preferred { PREFERRED_BONUS } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const GIB: i64 = 1024 * 1024 * 1024;

    fn agent(free_cpu: i32, free_memory_gib: i64, kvm_capable: bool) -> AgentResources {
        AgentResources {
            agent_id: Uuid::new_v4(),
            allocatable_cpu_millicores: 8000,
            allocatable_memory_bytes: 16 * GIB,
            free_cpu_millicores: free_cpu,
            free_memory_bytes: free_memory_gib * GIB,
            free_disk_bytes: 100 * GIB,
            kvm_capable,
            zone: "default".to_string(),
            spread_counts: HashMap::new(),
        }
    }

    fn demand<'a>(spread_key: Option<&'a str>) -> Demand<'a> {
        Demand {
            cpu_millicores: 1000,
            memory_bytes: GIB,
            disk_bytes: GIB,
            requires_kvm: true,
            spread_key,
            topology_spread: TopologySpread::None,
            preferred_agents: None,
        }
    }

    #[test]
    fn spread_prefers_emptier_agent_and_binpack_the_fuller_one() {
        let agents = vec![agent(2000, 4, true), agent(7000, 14, true)];
        assert_eq!(
            pick(&demand(None), &agents, Strategy::Spread),
            Some(agents[1].agent_id)
        );
        assert_eq!(
            pick(&demand(None), &agents, Strategy::Binpack),
            Some(agents[0].agent_id)
        );
    }

    #[test]
    fn skips_agents_without_capacity_or_kvm() {
        let agents = vec![agent(500, 14, true), agent(7000, 14, false)];
        assert_eq!(pick(&demand(None), &agents, Strategy::Spread), None);
    }

    #[test]
    fn spread_key_moves_replicas_to_other_agents() {
        let mut agents = vec![agent(7000, 14, true), agent(7000, 14, true)];
        let first = pick(&demand(Some("web")), &agents, Strategy::Spread).unwrap();
        commit(&mut agents, first, &demand(Some("web")));
        let second = pick(&demand(Some("web")), &agents, Strategy::Spread).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn preferred_agent_wins_between_equal_agents() {
        let agents = vec![agent(7000, 14, true), agent(7000, 14, true)];
        let preferred: HashSet<Uuid> = [agents[1].agent_id].into_iter().collect();
        let demand = Demand {
            preferred_agents: Some(&preferred),
            ..demand(None)
        };
        assert_eq!(
            pick(&demand, &agents, Strategy::Spread),
            Some(agents[1].agent_id)
        );
    }

    #[test]
    fn spreading_replicas_outweighs_the_resource_group_preference() {
        let mut agents = vec![agent(7000, 14, true), agent(7000, 14, true)];
        let preferred: HashSet<Uuid> = [agents[0].agent_id].into_iter().collect();
        let first = agents[0].agent_id;
        commit(&mut agents, first, &demand(Some("web")));
        let demand = Demand {
            preferred_agents: Some(&preferred),
            ..demand(Some("web"))
        };
        assert_eq!(
            pick(&demand, &agents, Strategy::Spread),
            Some(agents[1].agent_id)
        );
    }

    fn zoned(zone: &str) -> AgentResources {
        AgentResources {
            zone: zone.to_string(),
            ..agent(7000, 14, true)
        }
    }

    fn spread_demand(topology_spread: TopologySpread) -> Demand<'static> {
        Demand {
            topology_spread,
            ..demand(Some("web"))
        }
    }

    #[test]
    fn hard_topology_places_replicas_in_distinct_zones() {
        let mut agents = vec![zoned("a"), zoned("a"), zoned("b")];
        let demand = spread_demand(TopologySpread::Hard);
        let first = agents[0].agent_id;
        commit(&mut agents, first, &demand);
        assert_eq!(
            pick(&demand, &agents, Strategy::Spread),
            Some(agents[2].agent_id)
        );
    }

    #[test]
    fn hard_topology_falls_back_when_no_other_zone_has_capacity() {
        let mut agents = vec![zoned("a"), zoned("a"), zoned("b")];
        agents[2].free_cpu_millicores = 0;
        let demand = spread_demand(TopologySpread::Hard);
        let first = agents[0].agent_id;
        commit(&mut agents, first, &demand);
        assert_eq!(
            pick(&demand, &agents, Strategy::Spread),
            Some(agents[1].agent_id)
        );
    }

    #[test]
    fn soft_topology_prefers_empty_zone_without_excluding_others() {
        let mut agents = vec![zoned("a"), zoned("b")];
        let demand = spread_demand(TopologySpread::Soft);
        let first = agents[0].agent_id;
        commit(&mut agents, first, &demand);
        assert_eq!(
            pick(&demand, &agents, Strategy::Spread),
            Some(agents[1].agent_id)
        );
        agents[1].free_cpu_millicores = 0;
        assert_eq!(pick(&demand, &agents, Strategy::Spread), Some(first));
    }

    #[test]
    fn none_topology_ignores_zones() {
        let mut agents = vec![zoned("a"), zoned("b")];
        agents[1].free_cpu_millicores = 1500;
        let demand = spread_demand(TopologySpread::None);
        assert_eq!(
            pick(&demand, &agents, Strategy::Spread),
            Some(agents[0].agent_id)
        );
    }
}
