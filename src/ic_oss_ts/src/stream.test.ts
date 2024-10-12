import { open } from 'node:fs/promises'
import { expect, test } from 'vitest'
import { type ReadableStream } from 'web-streams-polyfill'
import {
  readableStreamAsyncIterator,
  readAll,
  streamToFixedChunkSizeReadable,
  uint8ArrayToFixedChunkSizeReadable
} from './stream'

test('uint8ArrayToFixedChunkSizeReadable', async () => {
  const src = new Uint8Array([0, 1, 2, 3, 4, 5, 6, 7, 8, 9])
  let stream = uint8ArrayToFixedChunkSizeReadable(3, src)

  let data = await readAll(stream, 10)
  expect(data).toEqual(src)

  stream = uint8ArrayToFixedChunkSizeReadable(3, src)
  await expect(() => readAll(stream, 8)).rejects.toThrow(
    'failed to read all data'
  )

  stream = uint8ArrayToFixedChunkSizeReadable(3, src)
  await expect(() => readAll(stream, 12)).rejects.toThrow(
    'failed to read all data'
  )
})

test('streamToFixedChunkSizeReadable', async () => {
  for (const name of [
    'package.json',
    'tsconfig.json',
    '.prettierrc.json',
    '.eslintrc.js'
  ]) {
    let fs = await open(name, 'r')
    const stat = await fs.stat()
    const src = await fs.readFile()
    fs.close()

    fs = await open(name, 'r')
    const stream = streamToFixedChunkSizeReadable(
      64,
      fs.readableWebStream() as any as ReadableStream<Uint8Array>,
      fs
    )

    let offset = 0
    const data = new Uint8Array(stat.size)
    for await (const chunk of readableStreamAsyncIterator(stream)) {
      data.set(chunk, offset)
      offset += chunk.byteLength
    }

    expect(Buffer.from(data)).toEqual(src)
  }
})
