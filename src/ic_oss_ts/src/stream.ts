import type { FileHandle } from 'node:fs/promises'
import { ReadableStream } from 'web-streams-polyfill'
import type { FileConfig } from './types.js'

export const CHUNK_SIZE = 256 * 1024

export interface ReadableSource<T> {
  getReader(): Pick<
    ReadableStreamDefaultReader<T>,
    'read' | 'cancel' | 'releaseLock'
  >
}

export async function* readableStreamAsyncIterator<T>(
  stream: ReadableSource<T>,
  signal?: AbortSignal
) {
  const reader = stream.getReader()
  let completed = false
  const cancel = () => {
    void reader.cancel(signal?.reason).catch(() => {})
  }
  if (signal?.aborted) cancel()
  else signal?.addEventListener('abort', cancel, { once: true })

  try {
    while (true) {
      const { done, value } = await reader.read()
      if (done) {
        completed = true
        return
      }
      yield value
    }
  } finally {
    signal?.removeEventListener('abort', cancel)
    try {
      if (!completed && !signal?.aborted) await reader.cancel()
    } finally {
      reader.releaseLock()
    }
  }
}

export async function toFixedChunkSizeReadable(file: FileConfig) {
  const source = file.content
  if (typeof File === 'function' && source instanceof File && !file.name) {
    file.name = source.name
  }

  if (typeof Blob === 'function' && source instanceof Blob) {
    file.contentType ||= source.type || 'application/octet-stream'
    file.size ||= source.size
    return streamToFixedChunkSizeReadable(CHUNK_SIZE, source.stream())
  }

  if (
    Array.isArray(source) ||
    source instanceof Uint8Array ||
    source instanceof ArrayBuffer
  ) {
    const content =
      source instanceof Uint8Array ? source : new Uint8Array(source)
    file.size ||= content.byteLength
    return uint8ArrayToFixedChunkSizeReadable(CHUNK_SIZE, content)
  }

  if (typeof source === 'object' && source && 'getReader' in source) {
    return streamToFixedChunkSizeReadable(CHUNK_SIZE, source)
  }

  if (typeof source === 'string') {
    const [{ open }, { basename }, { default: mime }] = await Promise.all([
      import('node:fs/promises'),
      import('node:path'),
      import('mime/lite')
    ])
    file.name ||= basename(source)
    file.contentType ||= mime.getType(file.name) ?? 'application/octet-stream'

    const fs = await open(source, 'r')
    try {
      const stat = await fs.stat()
      if (!stat.isFile()) throw new Error(`Not a file: ${source}`)
      file.size = stat.size
      // Node 20 requires the byte-stream option. Newer versions always use it.
      const options = { type: 'bytes', autoClose: false }
      return streamToFixedChunkSizeReadable(
        CHUNK_SIZE,
        fs.readableWebStream(options),
        fs
      )
    } catch (err) {
      await fs.close()
      throw err
    }
  }

  throw new Error(
    'Invalid arguments, FixedChunkSizeReadableStream could not be created'
  )
}

export function streamToFixedChunkSizeReadable(
  chunkSize: number,
  stream: ReadableSource<Uint8Array | ArrayBuffer>,
  fh?: FileHandle
) {
  if (!Number.isSafeInteger(chunkSize) || chunkSize <= 0) {
    throw new Error('chunkSize must be a positive integer')
  }
  const reader = stream.getReader()
  let pending: Uint8Array = new Uint8Array(0)
  let offset = 0
  let closed = false
  let cancelled = false
  const close = async () => {
    if (closed) return
    closed = true
    reader.releaseLock()
    await fh?.close()
  }

  return new ReadableStream<Uint8Array>({
    async pull(controller) {
      try {
        const chunk = new Uint8Array(chunkSize)
        let filled = 0
        while (filled < chunkSize) {
          if (offset === pending.byteLength) {
            const { done, value } = await reader.read()
            if (cancelled) return
            if (done) {
              if (filled) controller.enqueue(chunk.subarray(0, filled))
              controller.close()
              await close()
              return
            }
            pending =
              value instanceof Uint8Array ? value : new Uint8Array(value)
            offset = 0
          }
          const take = Math.min(chunkSize - filled, pending.byteLength - offset)
          chunk.set(pending.subarray(offset, offset + take), filled)
          offset += take
          filled += take
        }
        controller.enqueue(chunk)
      } catch (err) {
        try {
          await reader.cancel(err)
        } finally {
          await close()
        }
        throw err
      }
    },
    async cancel(reason) {
      cancelled = true
      try {
        await reader.cancel(reason)
      } finally {
        await close()
      }
    }
  })
}

export function uint8ArrayToFixedChunkSizeReadable(
  chunkSize: number,
  data: Uint8Array
) {
  if (!Number.isSafeInteger(chunkSize) || chunkSize <= 0) {
    throw new Error('chunkSize must be a positive integer')
  }
  let offset = 0
  return new ReadableStream<Uint8Array>({
    pull(controller) {
      if (offset === data.byteLength) {
        controller.close()
        return
      }
      const end = Math.min(offset + chunkSize, data.byteLength)
      controller.enqueue(data.subarray(offset, end))
      offset = end
    }
  })
}

export async function readAll(
  stream: ReadableSource<Uint8Array>,
  size: number
): Promise<Uint8Array> {
  const data = new Uint8Array(size)
  let offset = 0
  for await (const value of readableStreamAsyncIterator(stream)) {
    const chunk = value instanceof Uint8Array ? value : new Uint8Array(value)
    if (offset + chunk.byteLength <= size) {
      data.set(chunk, offset)
      offset += chunk.byteLength
    } else {
      offset += chunk.byteLength
      break
    }
  }

  if (offset != size) {
    throw new Error(
      `failed to read all data, expected ${size} bytes but got ${offset}`
    )
  }

  return data
}
