import mime from 'mime/lite'
import type { FileHandle } from 'node:fs/promises'
import { ReadableStream } from 'web-streams-polyfill'
import { FileConfig } from './types.js'

export const CHUNK_SIZE = 256 * 1024

// https://stackoverflow.com/questions/76700924/ts2504-type-readablestreamuint8array-must-have-a-symbol-asynciterator
export async function* readableStreamAsyncIterator<T>(self: ReadableStream<T>) {
  const reader = self.getReader()
  try {
    while (true) {
      const { done, value } = await reader.read()
      if (done) return
      yield value
    }
  } finally {
    reader.releaseLock()
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
    return uint8ArrayToFixedChunkSizeReadable(
      CHUNK_SIZE,
      Uint8Array.from(file.content as ArrayLike<number>)
    )
  }

  if (file.content instanceof ReadableStream) {
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
  const reader = stream.getReader()
  let buffer = new Uint8Array(0)

  return new ReadableStream<Uint8Array>({
    type: 'bytes',
    autoAllocateChunkSize: chunkSize,
    async pull(controller) {
      const byob = (controller as ReadableByteStreamController).byobRequest
      if (!byob) {
        throw new Error('byobRequest is required')
      }
      const v = byob.view!
      const w = new Uint8Array(v.buffer, v.byteOffset, v.byteLength)

      while (buffer.byteLength < chunkSize) {
        const { done, value } = await reader.read()

        if (done) {
          if (buffer.byteLength > 0) {
            w.set(buffer)
            byob.respond(buffer.byteLength)
          }

          reader.releaseLock()
          controller.close()
          fh?.close()
          return
        }

        const val = new Uint8Array(value)
        const newBuffer = new Uint8Array(buffer.byteLength + val.byteLength)
        newBuffer.set(buffer)
        newBuffer.set(val, buffer.byteLength)
        buffer = newBuffer
      }

      w.set(buffer.slice(0, w.byteLength))
      buffer = buffer.slice(w.byteLength)
      byob.respond(w.byteLength)
    },
    cancel(_reason) {
      reader.releaseLock()
      fh?.close()
    }
  })
}

export function uint8ArrayToFixedChunkSizeReadable(
  chunkSize: number,
  data: Uint8Array
) {
  let offset = 0

  return new ReadableStream<Uint8Array>({
    type: 'bytes',
    autoAllocateChunkSize: chunkSize,
    pull(controller) {
      const byob = (controller as ReadableByteStreamController).byobRequest
      if (!byob) {
        throw new Error('byobRequest is required')
      }

      const v = byob.view!
      const w = new Uint8Array(v.buffer, v.byteOffset, v.byteLength)
      const bytesToRead = Math.min(w.byteLength, data.byteLength - offset)
      w.set(data.subarray(offset, offset + bytesToRead))
      offset += bytesToRead

      if (bytesToRead === 0) {
        controller.close()
      } else {
        byob.respond(bytesToRead)
      }
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
    const chunk = new Uint8Array(value)
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
