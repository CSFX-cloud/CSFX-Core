const REPOSITORY: &str = "repository";
const PULL: &str = "pull";
const PUSH: &str = "push";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub kind: String,
    pub name: String,
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Pull,
    PullPush,
}

pub fn parse(raw: &str) -> Option<Scope> {
    let (rest, actions) = raw.rsplit_once(':')?;
    let (kind, name) = rest.split_once(':')?;
    if kind.is_empty() || name.is_empty() {
        return None;
    }
    Some(Scope {
        kind: kind.to_string(),
        name: name.to_string(),
        actions: actions
            .split(',')
            .filter(|action| !action.is_empty())
            .map(str::to_string)
            .collect(),
    })
}

pub fn parse_query(query: &str) -> Vec<Scope> {
    serde_urlencoded::from_str::<Vec<(String, String)>>(query)
        .unwrap_or_default()
        .into_iter()
        .filter(|(key, _)| key == "scope")
        .flat_map(|(_, value)| value.split(' ').filter_map(parse).collect::<Vec<_>>())
        .collect()
}

pub fn query_value(query: &str, name: &str) -> Option<String> {
    serde_urlencoded::from_str::<Vec<(String, String)>>(query)
        .ok()?
        .into_iter()
        .find_map(|(key, value)| (key == name).then_some(value))
}

pub fn granted_actions(scope: &Scope, allowed: Access) -> Vec<String> {
    let permitted: &[&str] = match allowed {
        Access::Pull => &[PULL],
        Access::PullPush => &[PULL, PUSH],
    };
    scope
        .actions
        .iter()
        .filter(|action| permitted.contains(&action.as_str()))
        .cloned()
        .collect()
}

pub fn is_repository(scope: &Scope) -> bool {
    scope.kind == REPOSITORY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_split_type_name_and_actions() {
        let scope = parse("repository:site-1:pull,push").unwrap();
        assert_eq!(scope.kind, "repository");
        assert_eq!(scope.name, "site-1");
        assert_eq!(scope.actions, vec!["pull", "push"]);
    }

    #[test]
    fn scope_names_may_contain_colons_and_slashes() {
        let scope = parse("repository:rg-1/app:pull").unwrap();
        assert_eq!(scope.name, "rg-1/app");
        let hosted = parse("repository:host:5060/app:pull").unwrap();
        assert_eq!(hosted.name, "host:5060/app");
    }

    #[test]
    fn malformed_scopes_are_dropped() {
        assert!(parse("repository").is_none());
        assert!(parse(":name:pull").is_none());
        assert!(parse("repository::pull").is_none());
    }

    #[test]
    fn queries_collect_repeated_and_space_separated_scopes() {
        let query =
            "service=csfx&scope=repository:a:pull&scope=repository:b:pull,push%20repository:c:pull";
        let names: Vec<String> = parse_query(query)
            .into_iter()
            .map(|scope| scope.name)
            .collect();
        assert_eq!(names, vec!["a", "b", "c"]);
        assert_eq!(query_value(query, "service").as_deref(), Some("csfx"));
    }

    #[test]
    fn pull_access_never_grants_push() {
        let scope = parse("repository:x:pull,push,delete").unwrap();
        assert_eq!(granted_actions(&scope, Access::Pull), vec!["pull"]);
        assert_eq!(
            granted_actions(&scope, Access::PullPush),
            vec!["pull", "push"]
        );
    }
}
