import mime from 'mime/lite'
import type { FileHandle } from 'node:fs/promises'
import { ReadableStream } from 'web-streams-polyfill'
import { FileConfig } from './types.js'

export const CHUNK_SIZE = 256 * 1024

// https://stackoverflow.com/questions/76700924/ts2504-type-readablestreamuint8array-must-have-a-symbol-asynciterator
export async function* readableStreamAsyncIterator<T>(self: ReadableStream<T>) {
  const reader = self.getReader()
  let completed = false
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
    try {
      if (!completed) await reader.cancel()
    } finally {
      reader.releaseLock()
    }
  }
}

export async function toFixedChunkSizeReadable(file: FileConfig) {
  if (typeof File === 'function' && file.content instanceof File) {
    if (!file.name) {
      file.name = file.content.name
    }
    if (!file.contentType) {
      file.contentType = file.content.type
    }
    if (!file.size) {
      file.size = file.content.size
    }
    return streamToFixedChunkSizeReadable(
      CHUNK_SIZE,
      file.content.stream() as any as ReadableStream<Uint8Array>
    )
  }

  if (typeof Blob === 'function' && file.content instanceof Blob) {
    if (!file.contentType) {
      file.contentType = file.content.type
    }
    if (!file.size) {
      file.size = file.content.size
    }
    return streamToFixedChunkSizeReadable(
      CHUNK_SIZE,
      file.content.stream() as any as ReadableStream<Uint8Array>
    )
  }

  if (
    Array.isArray(file.content) ||
    file.content instanceof Uint8Array ||
    file.content instanceof ArrayBuffer
  ) {
    // an ArrayBuffer is neither iterable nor array-like, Uint8Array.from
    // would silently yield an empty array for it
    const content =
      file.content instanceof ArrayBuffer
        ? new Uint8Array(file.content)
        : file.content instanceof Uint8Array
          ? file.content
          : Uint8Array.from(file.content as ArrayLike<number>)
    if (!file.size) {
      file.size = content.byteLength
    }
    return uint8ArrayToFixedChunkSizeReadable(CHUNK_SIZE, content)
  }

  if (
    file.content &&
    typeof (file.content as ReadableStream<Uint8Array>).getReader === 'function'
  ) {
    return streamToFixedChunkSizeReadable(
      CHUNK_SIZE,
      file.content as any as ReadableStream<Uint8Array>
    )
  }

  if (typeof file.content == 'string') {
    const { open } = await import('node:fs/promises')
    const path = await import('node:path')
    if (!file.name) {
      file.name = path.basename(file.content)
    }
    if (!file.contentType) {
      file.contentType = mime.getType(file.name) ?? 'application/octet-stream'
    }

    const fs = await open(file.content, 'r')
    const stat = await fs.stat()
    file.size = stat.size
    // try to fix "Closing file descriptor xx on garbage collection"
    ;(file as any).originFile = fs
    return streamToFixedChunkSizeReadable(
      CHUNK_SIZE,
      fs.readableWebStream() as any as ReadableStream<Uint8Array>,
      fs
    )
  }

  throw new Error(
    'Invalid arguments, FixedChunkSizeReadableStream could not be created'
  )
}

export function streamToFixedChunkSizeReadable(
  chunkSize: number,
  stream: ReadableStream<Uint8Array>,
  fh?: FileHandle
) {
  if (!Number.isSafeInteger(chunkSize) || chunkSize <= 0) {
    throw new Error('chunkSize must be a positive integer')
  }
  const reader = stream.getReader()
  let pending: Uint8Array = new Uint8Array(0)
  let offset = 0
  let closed = false
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
  stream: ReadableStream<Uint8Array>,
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
