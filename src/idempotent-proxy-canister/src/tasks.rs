use std::collections::BTreeMap;

use crate::{agent::Agent, ecdsa, store};

const SECONDS: u64 = 1_000_000_000;

pub async fn refresh_proxy_token() {
    let (ecdsa_key_name, proxy_token_refresh_interval, agents) = store::state::with(|s| {
        (
            s.ecdsa_key_name.clone(),
            s.proxy_token_refresh_interval,
            s.agents.clone(),
        )
    });
    update_proxy_token(ecdsa_key_name, proxy_token_refresh_interval, agents).await;
}

pub async fn update_proxy_token(
    ecdsa_key_name: String,
    proxy_token_refresh_interval: u64,
    mut agents: Vec<Agent>,
) {
    if agents.is_empty() {
        return;
    }

    let mut tokens: BTreeMap<String, String> = BTreeMap::new();
    for agent in agents.iter_mut() {
        if let Some(token) = tokens.get(&agent.name) {
            agent.proxy_token = Some(token.clone());
            continue;
        }

        let token = ecdsa::sign_proxy_token(
            &ecdsa_key_name,
            (ic_cdk::api::time() / SECONDS) + proxy_token_refresh_interval + 120,
            &agent.name,
        )
        .await
        .expect("failed to sign proxy token");
        tokens.insert(agent.name.clone(), token.clone());
        agent.proxy_token = Some(token);
    }

    store::state::with_mut(|r| r.agents = agents);
}
