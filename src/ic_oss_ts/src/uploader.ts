import { sha3_256 } from '@noble/hashes/sha3.js'
import type { BucketCanister } from './bucket.canister.js'
import { ConcurrencyQueue } from './queue.js'
import {
  CHUNK_SIZE,
  readableStreamAsyncIterator,
  readAll,
  toFixedChunkSizeReadable,
  type ReadableSource
} from './stream.js'
import type { FileConfig, Progress, UploadFileChunksResult } from './types.js'

export const MAX_FILE_SIZE_PER_CALL = 2000 * 1024

export class Uploader {
  readonly #cli: BucketCanister
  readonly concurrency: number
  readonly setReadonly: boolean

  constructor(
    client: BucketCanister,
    concurrency: number = 16,
    setReadonly = false
  ) {
    this.#cli = client
    this.concurrency = concurrency
    this.setReadonly = setReadonly
  }

  async upload(
    file: FileConfig,
    onProgress: (progress: Progress) => void = () => {}
  ): Promise<UploadFileChunksResult> {
    const stream = await toFixedChunkSizeReadable(file)
    const size = file.size
    if (size !== undefined && size <= MAX_FILE_SIZE_PER_CALL) {
      const content = await readAll(stream, size)
      const hash = file.hash || sha3_256(content)
      const res = await this.#cli.createFile({
        status: this.setReadonly ? [1] : [],
        content: content.length ? [content] : [],
        custom: [],
        hash: [hash],
        name: file.name,
        size: [BigInt(size)],
        content_type: file.contentType,
        parent: file.parent || 0,
        dek: []
      })

      onProgress({
        filled: size,
        size,
        chunkIndex: 0,
        concurrency: 1
      })

      return {
        id: res.id,
        filled: size,
        uploadedChunks: [],
        hash
      }
    }

    const res = await this.#cli
      .createFile({
        status: [],
        content: [],
        custom: [],
        hash: [file.hash || new Uint8Array(32)],
        name: file.name,
        size: size === undefined ? [] : [BigInt(size)],
        content_type: file.contentType,
        parent: file.parent || 0,
        dek: []
      })
      .catch(async (err) => {
        await stream.cancel(err).catch(() => {})
        throw err
      })

    return this.upload_chunks(
      stream,
      res.id,
      size ?? 0,
      file.hash || null,
      [],
      onProgress
    )
  }

  async upload_chunks(
    stream: ReadableSource<Uint8Array>,
    id: number,
    size: number,
    hash: Uint8Array | null = null,
    excludeChunks: number[] = [],
    onProgress: (progress: Progress) => void = () => {}
  ): Promise<UploadFileChunksResult> {
    const queue = new ConcurrencyQueue(this.concurrency)
    const excluded = new Set(excludeChunks)

    let chunkIndex = 0
    let prevChunkSize = CHUNK_SIZE
    const hasher = hash ? null : sha3_256.create()
    const rt: UploadFileChunksResult = {
      id,
      filled: 0,
      uploadedChunks: [...excluded],
      hash
    }

    try {
      for await (const value of readableStreamAsyncIterator(
        stream,
        queue.signal
      )) {
        if (prevChunkSize !== CHUNK_SIZE) {
          throw new Error(
            `Prev chunk size mismatch, expected ${CHUNK_SIZE} but got ${prevChunkSize}`
          )
        }
        // the Candid encoder copies the bytes, a view needs no copy here
        const chunk =
          value instanceof Uint8Array ? value : new Uint8Array(value)
        prevChunkSize = chunk.byteLength
        const index = chunkIndex
        chunkIndex += 1

        hasher?.update(chunk)
        if (excluded.has(index)) {
          rt.filled += chunk.byteLength
          onProgress({
            filled: rt.filled,
            size,
            chunkIndex: index,
            concurrency: 0
          })
          continue
        }

        await queue.push(async (_aborter, concurrency) => {
          await this.#cli.updateFileChunk({
            id,
            chunk_index: index,
            content: chunk
          })

          rt.filled += chunk.byteLength
          rt.uploadedChunks.push(index)
          onProgress({
            filled: rt.filled,
            size,
            chunkIndex: index,
            concurrency
          })
        })
      }

      await queue.wait()
      if (size > 0 && rt.filled !== size) {
        throw new Error(
          `File size mismatch, expected ${size} but got ${rt.filled}`
        )
      }
      const finalHash = hash ?? hasher!.digest()
      rt.hash = finalHash
      await this.#cli.updateFileInfo({
        id,
        status: this.setReadonly ? [1] : [],
        hash: [finalHash],
        custom: [],
        name: [],
        // Zero lets the bucket determine the size from its stored chunks.
        size: [BigInt(size)],
        content_type: []
      })
    } catch (err) {
      queue.abort(err)
      // Finish in-flight calls so the resume state cannot change after return.
      await queue.wait().catch(() => {})
      // the canister rejects with a bare string, which cannot carry a property
      if (
        err !== null &&
        (typeof err === 'object' || typeof err === 'function')
      ) {
        ;(err as any).data = rt
        throw err
      }
      const e = new Error(String(err), { cause: err })
      ;(e as any).data = rt
      throw e
    }

    return rt
  }
}
