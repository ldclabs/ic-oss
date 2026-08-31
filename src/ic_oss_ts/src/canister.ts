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

export interface QueryParams {
  /** Perform update calls (certified) or query calls (not certified). */
  certified?: boolean
}

export interface ServiceOptions<T> {
  agent?: Agent
  canisterId?: Principal
  serviceOverride?: ActorSubclass<T>
  certifiedServiceOverride?: ActorSubclass<T>
}

export const defaultAgent = (): Agent =>
  HttpAgent.createSync({ host: IC_HOST, identity: new AnonymousIdentity() })

export const createServices = <T>({
  options: {
    canisterId,
    serviceOverride,
    certifiedServiceOverride,
    agent: agentOption,
    callTransform,
    queryTransform
  },
  idlFactory,
  certifiedIdlFactory
}: {
  options: Required<Pick<ServiceOptions<T>, 'canisterId'>> &
    Omit<ServiceOptions<T>, 'canisterId'> &
    Pick<ActorConfig, 'queryTransform' | 'callTransform'>
  idlFactory: IDL.InterfaceFactory
  certifiedIdlFactory: IDL.InterfaceFactory
}): {
  service: ActorSubclass<T>
  certifiedService: ActorSubclass<T>
  agent: Agent
  canisterId: Principal
} => {
  const agent = agentOption ?? defaultAgent()
  const config: ActorConfig = {
    agent,
    canisterId,
    ...(callTransform && { callTransform }),
    ...(queryTransform && { queryTransform })
  }
  const service = serviceOverride ?? Actor.createActor<T>(idlFactory, config)
  const certifiedService =
    certifiedServiceOverride ??
    Actor.createActor<T>(certifiedIdlFactory, config)

  return { service, certifiedService, agent, canisterId }
}

export abstract class Canister<T> {
  protected constructor(
    private readonly id: Principal,
    protected readonly service: T,
    protected readonly certifiedService: T
  ) {}

  get canisterId(): Principal {
    return this.id
  }

  protected caller = ({ certified = true }: QueryParams): T =>
    certified ? this.certifiedService : this.service
}
