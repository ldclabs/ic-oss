use candid::{Nat, Principal};
use ic_agent::Agent;
use ic_oss_types::{cluster::*, cose::Token};
use serde_bytes::{ByteArray, ByteBuf};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

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

    pub async fn admin_ed25519_access_token(&self, args: Token) -> Result<ByteBuf, String> {
        update_call(
            &self.agent,
            &self.cluster,
            "admin_ed25519_access_token",
            (args,),
        )
        .await?
    }

    pub async fn admin_weak_access_token(
        &self,
        args: Token,
        now_sec: u64,
        expiration_sec: u64,
    ) -> Result<ByteBuf, String> {
        query_call(
            &self.agent,
            &self.cluster,
            "admin_weak_access_token",
            (args, now_sec, expiration_sec),
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

    pub async fn ed25519_access_token(&self, audience: Principal) -> Result<ByteBuf, String> {
        update_call(
            &self.agent,
            &self.cluster,
            "ed25519_access_token",
            (audience,),
        )
        .await?
    }

    pub async fn get_cluster_info(&self) -> Result<ClusterInfo, String> {
        query_call(&self.agent, &self.cluster, "get_cluster_info", ()).await?
    }

    pub async fn get_bucket_wasm(&self, hash: ByteArray<32>) -> Result<WasmInfo, String> {
        query_call(&self.agent, &self.cluster, "get_bucket_wasm", (hash,)).await?
    }

    pub async fn get_buckets(&self) -> Result<Vec<Principal>, String> {
        query_call(&self.agent, &self.cluster, "get_buckets", ()).await?
    }

    pub async fn get_deployed_buckets(&self) -> Result<Vec<BucketDeploymentInfo>, String> {
        query_call(&self.agent, &self.cluster, "get_deployed_buckets", ()).await?
    }

    pub async fn bucket_deployment_logs(
        &self,
        prev: Option<Nat>,
        take: Option<Nat>,
    ) -> Result<Vec<BucketDeploymentInfo>, String> {
        query_call(
            &self.agent,
            &self.cluster,
            "bucket_deployment_logs",
            (prev, take),
        )
        .await?
    }

    pub async fn get_subject_policies(
        &self,
        subject: Principal,
    ) -> Result<BTreeMap<Principal, String>, String> {
        query_call(
            &self.agent,
            &self.cluster,
            "get_subject_policies",
            (subject,),
        )
        .await?
    }

    pub async fn get_subject_policies_for(
        &self,
        subject: Principal,
        audience: Principal,
    ) -> Result<String, String> {
        query_call(
            &self.agent,
            &self.cluster,
            "get_subject_policies_for",
            (subject, audience),
        )
        .await?
    }

    pub async fn admin_add_wasm(
        &self,
        args: AddWasmInput,
        force_prev_hash: Option<ByteArray<32>>,
    ) -> Result<(), String> {
        update_call(
            &self.agent,
            &self.cluster,
            "admin_add_wasm",
            (args, force_prev_hash),
        )
        .await?
    }

    pub async fn admin_deploy_bucket(
        &self,
        args: DeployWasmInput,
        ignore_prev_hash: Option<ByteArray<32>>,
    ) -> Result<(), String> {
        update_call(
            &self.agent,
            &self.cluster,
            "admin_deploy_bucket",
            (args, ignore_prev_hash),
        )
        .await?
    }

    pub async fn admin_upgrade_all_buckets(&self, args: Option<ByteBuf>) -> Result<(), String> {
        update_call(
            &self.agent,
            &self.cluster,
            "admin_upgrade_all_buckets",
            (args,),
        )
        .await?
    }

    pub async fn admin_batch_call_buckets(
        &self,
        buckets: BTreeSet<Principal>,
        method: String,
        args: Option<ByteBuf>,
    ) -> Result<Vec<ByteBuf>, String> {
        update_call(
            &self.agent,
            &self.cluster,
            "admin_batch_call_buckets",
            (buckets, method, args),
        )
        .await?
    }

    pub async fn admin_topup_all_buckets(&self) -> Result<u128, String> {
        update_call(&self.agent, &self.cluster, "admin_topup_all_buckets", ()).await?
    }
}
