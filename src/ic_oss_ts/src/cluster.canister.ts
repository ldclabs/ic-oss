import type { Principal } from '@dfinity/principal'
import { Canister, createServices } from '@dfinity/utils'
import type {
  AddWasmInput,
  BucketDeploymentInfo,
  ClusterInfo,
  _SERVICE as ClusterService,
  DeployWasmInput,
  Token,
  WasmInfo
} from '../candid/ic_oss_cluster/ic_oss_cluster.did.js'
import { idlFactory } from '../candid/ic_oss_cluster/ic_oss_cluster.did.js'
import type { CanisterOptions } from './types.js'
import { resultOk } from './types.js'

export class ClusterCanister extends Canister<ClusterService> {
  #resultOk: typeof resultOk = resultOk

  static create(options: CanisterOptions<ClusterService>) {
    const { service, certifiedService, canisterId } =
      createServices<ClusterService>({
        options,
        idlFactory,
        certifiedIdlFactory: idlFactory
      })

    const self = new ClusterCanister(canisterId, service, certifiedService)
    self.#resultOk = options.unwrapResult || resultOk
    return self
  }

  async getClusterInfo(): Promise<ClusterInfo> {
    const res = await this.service.get_cluster_info()
    return this.#resultOk(res)
  }

  async accessToken(audience: Principal): Promise<Uint8Array> {
    const res = await this.service.access_token(audience)
    return Uint8Array.from(this.#resultOk(res))
  }

  async ed25519AccessToken(audience: Principal): Promise<Uint8Array> {
    const res = await this.service.ed25519_access_token(audience)
    return Uint8Array.from(this.#resultOk(res))
  }

  async adminSignAccessToken(input: Token): Promise<Uint8Array> {
    const res = await this.service.admin_sign_access_token(input)
    return Uint8Array.from(this.#resultOk(res))
  }

  async adminEd25519AccessToken(input: Token): Promise<Uint8Array> {
    const res = await this.service.admin_ed25519_access_token(input)
    return Uint8Array.from(this.#resultOk(res))
  }

  async adminWeakAccessToken(
    input: Token,
    now_sec: bigint,
    expiration_sec: bigint
  ): Promise<Uint8Array> {
    const res = await this.service.admin_weak_access_token(
      input,
      now_sec,
      expiration_sec
    )
    return Uint8Array.from(this.#resultOk(res))
  }

  async adminSetManagers(input: Principal[]): Promise<null> {
    const res = await this.service.admin_set_managers(input)
    return this.#resultOk(res)
  }

  async adminAddWasm(
    input: AddWasmInput,
    forcePrevHash: Uint8Array | null = null
  ): Promise<null> {
    const res = await this.service.admin_add_wasm(
      input,
      forcePrevHash ? [forcePrevHash] : []
    )
    return this.#resultOk(res)
  }

  async adminAttachPolicies(input: Token): Promise<null> {
    const res = await this.service.admin_attach_policies(input)
    return this.#resultOk(res)
  }

  async adminDetachPolicies(input: Token): Promise<null> {
    const res = await this.service.admin_detach_policies(input)
    return this.#resultOk(res)
  }

  async adminBatchCallBuckets(
    buckets: Principal[],
    method: string,
    args: Uint8Array | null = null
  ): Promise<Uint8Array[]> {
    const res = await this.service.admin_batch_call_buckets(
      buckets,
      method,
      args ? [args] : []
    )

    return this.#resultOk(res) as Uint8Array[]
  }

  async adminDeployBucket(
    input: DeployWasmInput,
    ignorePrevHash: Uint8Array | null = null
  ): Promise<null> {
    const res = await this.service.admin_deploy_bucket(
      input,
      ignorePrevHash ? [ignorePrevHash] : []
    )
    return this.#resultOk(res)
  }

  async adminUpgradeAllBuckets(args: Uint8Array | null = null): Promise<null> {
    const res = await this.service.admin_upgrade_all_buckets(args ? [args] : [])
    return this.#resultOk(res)
  }

  async adminTopupAllBuckets(): Promise<bigint> {
    const res = await this.service.admin_topup_all_buckets()
    return this.#resultOk(res)
  }

  async bucketDeploymentLogs(
    prev: bigint | null = null,
    take: bigint | null = null
  ): Promise<BucketDeploymentInfo[]> {
    const res = await this.service.bucket_deployment_logs(
      prev == null ? [] : [prev],
      take == null ? [] : [take]
    )
    return this.#resultOk(res)
  }

  async getBucketWasm(hash: Uint8Array): Promise<WasmInfo> {
    const res = await this.service.get_bucket_wasm(hash)
    return this.#resultOk(res)
  }

  async getBuckets(): Promise<Principal[]> {
    const res = await this.service.get_buckets()
    return this.#resultOk(res)
  }

  async getDeployedBuckets(): Promise<BucketDeploymentInfo[]> {
    const res = await this.service.get_deployed_buckets()
    return this.#resultOk(res)
  }

  async getSubjectPolicies(
    subject: Principal
  ): Promise<Array<[Principal, string]>> {
    const res = await this.service.get_subject_policies(subject)
    return this.#resultOk(res)
  }

  async getSubjectPoliciesFor(
    subject: Principal,
    audience: Principal
  ): Promise<String> {
    const res = await this.service.get_subject_policies_for(subject, audience)
    return this.#resultOk(res)
  }
}
