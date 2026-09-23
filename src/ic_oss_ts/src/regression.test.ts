import { expect, test } from 'vitest'
import type { BucketCanister } from './bucket.canister.js'
import { ConcurrencyQueue } from './queue.js'
import {
  CHUNK_SIZE,
  readAll,
  toFixedChunkSizeReadable,
  uint8ArrayToFixedChunkSizeReadable
} from './stream.js'
import type { FileConfig } from './types.js'
import { Uploader } from './uploader.js'

const bytes = new Uint8Array([0, 1, 2, 3, 4, 5, 6, 7, 8, 9])

test('toFixedChunkSizeReadable accepts every byte-array input', async () => {
  const cases: Record<string, FileConfig['content']> = {
    // an ArrayBuffer is neither iterable nor array-like
    ArrayBuffer: bytes.buffer.slice(0),
    Uint8Array: bytes,
    'number[]': Array.from(bytes)
  }

  for (const [label, content] of Object.entries(cases)) {
    const file: FileConfig = {
      content,
      name: 'a.bin',
      contentType: 'application/octet-stream'
    }
    const stream = await toFixedChunkSizeReadable(file)
    expect(file.size, label).toBe(bytes.byteLength)
    expect(await readAll(stream, bytes.byteLength), label).toEqual(bytes)
  }
})

test('ConcurrencyQueue.wait awaits tasks still queued for a slot', async () => {
  const queue = new ConcurrencyQueue(2)
  const done: number[] = []

  // more tasks than slots, pushed without awaiting, so several sit in the queue
  for (let i = 0; i < 6; i++) {
    void queue.push(async () => {
      await new Promise((r) => setTimeout(r, 5))
      done.push(i)
    })
  }

  expect(await queue.wait()).toBe(6)
  expect(done).toHaveLength(6)
})

test('ConcurrencyQueue does not deadlock on a non-positive concurrency', async () => {
  const queue = new ConcurrencyQueue(0)
  let ran = false

  await queue.push(async () => {
    ran = true
  })

  expect(await queue.wait()).toBe(1)
  expect(ran).toBe(true)
})

test('upload_chunks keeps a string error and attaches the resume state', async () => {
  // the canister rejects Result<_, text>, which resultOk throws as a primitive
  const cli = {
    updateFileChunk: async () => {
      throw 'permission denied'
    }
  } as unknown as BucketCanister

  const stream = uint8ArrayToFixedChunkSizeReadable(CHUNK_SIZE, bytes)
  const err = await new Uploader(cli, 2)
    .upload_chunks(stream, 1, bytes.byteLength)
    .catch((e) => e)

  expect(err).toBeInstanceOf(Error)
  expect(err.message).toBe('permission denied')
  expect(err.data).toEqual({
    id: 1,
    filled: 0,
    uploadedChunks: [],
    hash: null
  })
})

test('queue retains failures that happened before wait and rejects later work', async () => {
  const queue = new ConcurrencyQueue(1)
  await queue.push(async () => {
    throw new Error('first failure')
  })
  await new Promise((resolve) => setTimeout(resolve, 0))
  await expect(queue.wait()).rejects.toThrow('first failure')
  await expect(queue.push(async () => {})).rejects.toThrow('first failure')
})

test('queue stops queued work and waits for running calls on failure', async () => {
  const queue = new ConcurrencyQueue(2)
  let fail!: () => void
  let finish!: () => void
  let finished = false
  let queuedRan = false
  await queue.push(
    () =>
      new Promise((_, reject) => {
        fail = () => reject(new Error('failed'))
      })
  )
  await queue.push(
    () =>
      new Promise<void>((resolve) => {
        finish = () => {
          finished = true
          resolve()
        }
      })
  )
  const queued = queue
    .push(async () => {
      queuedRan = true
    })
    .catch((err) => err)
  fail()
  const result = queue.wait().catch((err) => err)
  await new Promise((resolve) => setTimeout(resolve, 0))
  expect(finished).toBe(false)
  finish()
  expect((await result).message).toBe('failed')
  expect((await queued).message).toBe('failed')
  expect(finished).toBe(true)
  expect(queuedRan).toBe(false)
})

test('resume hashes the entire stream and retains previous chunk ids', async () => {
  const { sha3_256 } = await import('@noble/hashes/sha3.js')
  const source = new Uint8Array(CHUNK_SIZE + 13).fill(17)
  source[CHUNK_SIZE] = 23
  const uploaded: number[] = []
  let savedHash: Uint8Array | undefined
  const cli = {
    updateFileChunk: async ({ chunk_index }: { chunk_index: number }) => {
      uploaded.push(chunk_index)
      return { filled: BigInt(source.length) }
    },
    updateFileInfo: async ({ hash }: { hash: [Uint8Array] }) => {
      savedHash = hash[0]
    }
  } as unknown as BucketCanister
  const result = await new Uploader(cli).upload_chunks(
    uint8ArrayToFixedChunkSizeReadable(CHUNK_SIZE, source),
    1,
    source.length,
    null,
    [0]
  )
  expect(uploaded).toEqual([1])
  expect(result.uploadedChunks).toEqual([0, 1])
  expect(savedHash).toEqual(sha3_256(source))
})

test('large and exactly 2 MiB uploads supply an indexed hash and use chunks', async () => {
  const source = new Uint8Array(2 * 1024 * 1024)
  for (const hash of [undefined, new Uint8Array(32).fill(3)]) {
    let created: any
    let chunks = 0
    const cli = {
      createFile: async (input: any) => {
        created = input
        return { id: 5 }
      },
      updateFileChunk: async () => {
        chunks++
        return { filled: 0n }
      },
      updateFileInfo: async () => {}
    } as unknown as BucketCanister
    await new Uploader(cli).upload({
      name: 'large.bin',
      contentType: 'application/octet-stream',
      content: source,
      hash
    })
    expect(created.content).toEqual([])
    expect(created.hash).toEqual([hash || new Uint8Array(32)])
    expect(chunks).toBe(8)
  }
})

test('native streams are accepted and cancellation reaches their source', async () => {
  let cancelled = false
  const content = new globalThis.ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(new Uint8Array(CHUNK_SIZE * 2))
    },
    cancel() {
      cancelled = true
    }
  })
  const stream = await toFixedChunkSizeReadable({
    name: 'native.bin',
    contentType: 'application/octet-stream',
    content
  })
  const reader = stream.getReader()
  expect((await reader.read()).value?.length).toBe(CHUNK_SIZE)
  await reader.cancel('stop')
  expect(cancelled).toBe(true)
})
