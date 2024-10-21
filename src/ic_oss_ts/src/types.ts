import type { Principal } from '@dfinity/principal'
import type { CanisterOptions as Options } from '@dfinity/utils'

export interface CanisterOptions<T> extends Omit<Options<T>, 'canisterId'> {
  canisterId: Principal
  unwrapResult?: typeof resultOk
}

export interface Ok<T> {
  Ok: T
}

export interface Err<T> {
  Err: T
}

export type Result<T, E> = Ok<T> | Err<E>

export function resultOk<T, E>(res: Result<T, E>): T {
  if ('Err' in res) {
    throw res.Err
  }

  return res.Ok
}

export type FileChunk = [number, Uint8Array]

export interface FileConfig {
  content: ReadableStream | Blob | File | Uint8Array | ArrayBuffer | string
  name: string
  contentType: string
  size?: number
  /**
   * Folder that file will be uploaded to
   * @default 0, root folder
   */
  parent?: number
  /**
   * File hash generation will be skipped if hash is provided
   */
  hash?: Uint8Array
}

export interface UploadFileChunksResult {
  id: number
  filled: number
  uploadedChunks: number[]
  hash: Uint8Array | null
}

export interface Progress {
  filled: number
  size?: number // total size of file, may be unknown
  chunkIndex: number
  concurrency: number
}
