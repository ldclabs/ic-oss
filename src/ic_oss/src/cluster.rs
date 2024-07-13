use candid::Principal;
use ic_agent::Agent;
use ic_oss_types::{cluster::*, cwt::Token};
use serde_bytes::ByteBuf;
use std::{collections::BTreeSet, sync::Arc};

use crate::agent::{query_call, update_call};

#[derive(Clone)]
pub struct Client {
    agent: Arc<Agent>,
    cluster: Principal,
}

impl Client {
    pub fn new(agent: Arc<Agent>, cluster: Principal) -> Client {
        Client { agent, cluster }
    }

    /// the caller of agent should be canister controller
    pub async fn admin_set_managers(&self, args: BTreeSet<Principal>) -> Result<(), String> {
        update_call(&self.agent, &self.cluster, "admin_set_managers", (args,)).await?
    }

    /// the caller of agent should be canister manager
    pub async fn admin_sign_access_token(&self, args: Token) -> Result<ByteBuf, String> {
        update_call(
            &self.agent,
            &self.cluster,
            "admin_sign_access_token",
            (args,),
        )
        .await?
    }

    /// the caller of agent should be canister manager
    pub async fn admin_attach_policies(&self, args: Token) -> Result<(), String> {
        update_call(&self.agent, &self.cluster, "admin_attach_policies", (args,)).await?
    }

    /// the caller of agent should be canister manager
    pub async fn admin_detach_policies(&self, args: Token) -> Result<(), String> {
        update_call(&self.agent, &self.cluster, "admin_detach_policies", (args,)).await?
    }

    pub async fn access_token(&self, audience: Principal) -> Result<ByteBuf, String> {
        update_call(&self.agent, &self.cluster, "access_token", (audience,)).await?
    }

    pub async fn get_cluster_info(&self) -> Result<ClusterInfo, String> {
        query_call(&self.agent, &self.cluster, "get_cluster_info", ()).await?
    }
}
