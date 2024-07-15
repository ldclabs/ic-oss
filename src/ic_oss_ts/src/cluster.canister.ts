import { Canister, createServices } from '@dfinity/utils'
import type { _SERVICE as ClusterService } from '../candid/ic_oss_cluster/ic_oss_cluster.did'
import { idlFactory } from '../candid/ic_oss_cluster/ic_oss_cluster.did'
import type { ClusterInfo } from '../candid/ic_oss_cluster/ic_oss_cluster.did'
import type { CanisterOptions } from './types'
import { resultOk } from './types'
import type { Principal } from '@dfinity/principal'

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
}
