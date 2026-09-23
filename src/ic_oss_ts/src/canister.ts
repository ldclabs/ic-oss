import {
  Actor,
  AnonymousIdentity,
  HttpAgent,
  type ActorConfig,
  type ActorSubclass,
  type Agent
} from '@icp-sdk/core/agent'
import type { IDL } from '@icp-sdk/core/candid'
import type { Principal } from '@icp-sdk/core/principal'

export const IC_HOST = 'https://icp-api.io'

export interface ServiceOptions<T> {
  agent?: Agent
  canisterId?: Principal
  serviceOverride?: ActorSubclass<T>
}

export const defaultAgent = (): Agent =>
  HttpAgent.createSync({ host: IC_HOST, identity: new AnonymousIdentity() })

export const createServices = <T>({
  options: {
    canisterId,
    serviceOverride,
    agent: agentOption,
    callTransform,
    queryTransform
  },
  idlFactory
}: {
  options: Required<Pick<ServiceOptions<T>, 'canisterId'>> &
    Omit<ServiceOptions<T>, 'canisterId'> &
    Pick<ActorConfig, 'queryTransform' | 'callTransform'>
  idlFactory: IDL.InterfaceFactory
}): {
  service: ActorSubclass<T>
  agent: Agent
  canisterId: Principal
} => {
  const agent = agentOption ?? defaultAgent()
  const service =
    serviceOverride ??
    Actor.createActor<T>(idlFactory, {
      agent,
      canisterId,
      ...(callTransform && { callTransform }),
      ...(queryTransform && { queryTransform })
    })

  return { service, agent, canisterId }
}

export abstract class Canister<T> {
  protected constructor(
    private readonly id: Principal,
    protected readonly service: T
  ) {}

  get canisterId(): Principal {
    return this.id
  }
}
