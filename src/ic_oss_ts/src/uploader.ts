import { sha3_256 } from '@noble/hashes/sha3'
import { BucketCanister } from './bucket.canister.js'
import { crc32 } from './crc32.js'
import { ConcurrencyQueue } from './queue.js'
import {
  toFixedChunkSizeReadable,
  readableStreamAsyncIterator,
  readAll,
  CHUNK_SIZE
} from './stream.js'
import { FileConfig, UploadFileChunksResult, Progress } from './types.js'

export const MAX_FILE_SIZE_PER_CALL = 1024 * 2000

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
    const size = file.size || 0
    if (size > 0 && size <= MAX_FILE_SIZE_PER_CALL) {
      const content = await readAll(stream, size)
      const hash = file.hash || sha3_256(content)
      let res = await this.#cli.createFile({
        status: this.setReadonly ? [1] : [],
        content: [content],
        custom: [],
        hash: [hash],
        name: file.name,
        crc32: [crc32(content)],
        size: [BigInt(size)],
        content_type: file.contentType,
        parent: file.parent || 0
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
        uploadedChunks: []
      }
    }

    let res = await this.#cli.createFile({
      status: [],
      content: [],
      custom: [],
      hash: [],
      name: file.name,
      crc32: [],
      size: size > 0 ? [BigInt(size)] : [],
      content_type: file.contentType,
      parent: file.parent || 0
    })

    return await this.upload_chunks(
      stream,
      res.id,
      size,
      file.hash || null,
      [],
      onProgress
    )
  }

  async upload_chunks(
    stream: ReadableStream<Uint8Array>,
    id: number,
    size: number,
    hash: Uint8Array | null,
    excludeChunks: number[],
    onProgress: (progress: Progress) => void = () => {}
  ): Promise<UploadFileChunksResult> {
    const queue = new ConcurrencyQueue(this.concurrency)

    let chunkIndex = 0
    let prevChunkSize = CHUNK_SIZE
    const hasher = sha3_256.create()
    const rt: UploadFileChunksResult = {
      id,
      filled: 0,
      uploadedChunks: []
    }

    try {
      for await (const value of readableStreamAsyncIterator(stream)) {
        if (prevChunkSize !== CHUNK_SIZE) {
          throw new Error(
            `Prev chunk size mismatch, expected ${CHUNK_SIZE} but got ${prevChunkSize}`
          )
        }
        const chunk = new Uint8Array(value)
        prevChunkSize = chunk.byteLength
        const index = chunkIndex
        chunkIndex += 1

        if (excludeChunks.includes(index)) {
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
          !hash && hasher.update(chunk)
          const res = await this.#cli.updateFileChunk({
            id,
            chunk_index: index,
            content: chunk,
            crc32: [crc32(chunk)]
          })

          rt.filled += chunk.byteLength
          rt.uploadedChunks.push(index)
          onProgress({
            filled: Number(res.filled),
            size,
            chunkIndex: index,
            concurrency
          })
        })
      }

      await queue.wait()
      await this.#cli.updateFileInfo({
        id,
        status: this.setReadonly ? [1] : [],
        hash: [hash || hasher.digest()],
        custom: [],
        name: [],
        content_type: []
      })
    } catch (err) {
      rt.error = err
    }

    return rt
  }
}
