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
