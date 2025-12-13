use std::sync::Arc;

use crate::{api::can_telegram, cache::Cache, config::Rule};

use caramel::types::akari::Event;
use log::warn;

struct Match {
    matched: bool,
    excluded: bool
}

impl Match {
    fn new() -> Self {
        Self { matched: false, excluded: false }
    }

    fn match_if(&mut self, matched: bool) {
        self.matched |= matched;
    }

    fn exclude_if(&mut self, excluded: bool) {
        self.excluded |= excluded;
    }

    fn matches(&self) -> bool {
        self.matched && !self.excluded
    }
}

fn translate_event_category(
    event: &Event
) -> Vec<(String, Option<String>, Option<String>)> {
    match event.category.as_str() {
        "move" => vec![
                    ("move_from".into(), event.actor.clone(), event.origin.clone()),
                    ("move_to".into(), event.actor.clone(), event.destination.clone())
                  ],
        "wadmit" => vec![("admit".into(), event.actor.clone(), event.origin.clone())],
        "wresign" => vec![("resign".into(), event.actor.clone(), event.origin.clone())],
        "nfound" => vec![("found".into(), event.actor.clone(), event.origin.clone())],
        "nrefound" => vec![("refound".into(), event.actor.clone(), event.origin.clone())],
        _ => vec![]
    }
}

async fn matches_nation_impl(arg: &str, nation: &String, cache: Arc<Cache>) -> bool {
    if let Some(command) = arg.strip_prefix("$") {
        let mut regex_cache = cache.regex.write().await;
        if let Some(pattern) = command.strip_prefix("re:") {
            if let Ok(regex) = regex_cache.get_regex(pattern) {
                return regex.is_match(&nation);
            } else { 
                warn!("Invalid regex pattern in rule: '$re:{}'", pattern);
                return false; 
            }
        } else if command == "numbered_puppet" {
            let pattern = "^[0-9a-z_-]+[0-9]+$";
            if let Ok(regex) = regex_cache.get_regex(pattern) {
                return regex.is_match(&nation);
            } else { 
                return false;
            }
        } else if command == "roman_puppet" {
            let pattern = "^[0-9a-z_-]+_m{0,4}(cm|cd|d?c{0,3})(xc|xl|l?x{0,3})(ix|iv|v?i{0,3})$";
            if let Ok(regex) = regex_cache.get_regex(pattern) {
                return regex.is_match(&nation);
            } else { 
                return false; 
            }
        } else if command == "is_wa" {
            return cache.wa_nations.read().await.contains(nation);
        } else if command == "recruitment_disabled" {
            return !can_telegram(&cache.client, &nation).await;
        } else {
            warn!("Invalid command in rule: '${}'", command);
            return false;
        }
    } else {
        return nation == arg;
    }
}

async fn matches_nation(arg: &str, nation: &Option<String>, cache: Arc<Cache>, match_obj: &mut Match) {
    if let Some(nation) = nation {
        if arg == "*" {
            match_obj.match_if(true);
        } else if let Some(negated_arg) = arg.strip_prefix("!") {
            match_obj.exclude_if(matches_nation_impl(negated_arg, nation, cache).await)
        } else {
            match_obj.match_if(matches_nation_impl(arg, nation, cache).await);
        }
    }
}

async fn matches_region_impl(arg: &str, region: &String, cache: Arc<Cache>) -> bool {
    if let Some(command) = arg.strip_prefix("$") {
        if let Some(pattern) = command.strip_prefix("re:") {
            let mut regex_cache = cache.regex.write().await;
            if let Ok(regex) = regex_cache.get_regex(pattern) {
                return regex.is_match(&region);
            } else { 
                warn!("Invalid regex pattern in rule: '$re:{}'", pattern);
                return false; 
            }
        } else {
            warn!("Invalid command in rule: '${}'", command);
            return false;
        }
    } else {
        return region == arg;
    }
}

async fn matches_region(arg: &str, region: &Option<String>, cache: Arc<Cache>, match_obj: &mut Match) {
    if let Some(region) = region {
        if arg == "*" {
            match_obj.match_if(true);
        } else if let Some(negated_arg) = arg.strip_prefix("!") {
            match_obj.exclude_if(matches_region_impl(negated_arg, region, cache).await)
        } else {
            match_obj.match_if(matches_region_impl(arg, region, cache).await);
        }
    }
}

async fn match_rule_by_category(
    rule: &Rule, 
    cache: Arc<Cache>,
    category: String, nation: Option<String>, region: Option<String>
) -> bool {
    if !rule.event.contains(&category) { return false; }

    {
        let mut match_obj = Match::new();

        for arg in &rule.nations {
            matches_nation(arg, &nation, cache.clone(), &mut match_obj).await;
        }

        if !match_obj.matches() { return false; }
    }

    {
        let mut match_obj = Match::new();

        for arg in &rule.regions {
            matches_region(arg, &region, cache.clone(), &mut match_obj).await;
        }

        if !match_obj.matches() { return false; }
    }

    return true;
}

pub async fn match_rule(event: &Event, rule: &Rule, cache: Arc<Cache>) -> bool {
    for (category, nation, region) in translate_event_category(event) {
        if match_rule_by_category(rule, cache.clone(), category, nation, region).await {
            return true;
        }
    }

    false
}