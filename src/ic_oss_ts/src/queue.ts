export type Task = (
  aborter: AbortController,
  concurrency: number
) => Promise<void>

export class ConcurrencyQueue {
  #concurrency: number
  #total: number = 0
  #aborter: AbortController = new AbortController()
  #reject: (reason: unknown) => void = () => {}
  #queue: [Task, () => void][] = []
  #pending: Set<Task> = new Set()
  #results: Set<Promise<void>> = new Set()

  constructor(concurrency: number) {
    // a non-positive concurrency would never dequeue anything
    this.#concurrency = concurrency > 0 ? Math.floor(concurrency) : 1
  }

  #next() {
    if (this.#pending.size < this.#concurrency && this.#queue.length > 0) {
      const [fn, resolve] = this.#queue.shift()!
      this.#pending.add(fn)
      const result = fn(this.#aborter, this.#pending.size)
      this.#results.add(result)

      result
        .then(() => (this.#total += 1))
        .catch((err) => this.#abort(err))
        .finally(() => {
          this.#pending.delete(fn)
          this.#results.delete(result)
          this.#next()
        })

      resolve()
      this.#next()
    }
  }

  #abort(reason: unknown) {
    this.#aborter.abort(reason)
    this.#reject(reason)
  }

  push(fn: Task): Promise<void> {
    return new Promise<void>((resolve, reject) => {
      this.#reject = reject
      this.#queue.push([fn, resolve])
      this.#next()
    })
  }

  wait(): Promise<number> {
    return new Promise<number>((resolve, reject) => {
      this.#reject = reject

      // #results only holds the running tasks, so keep draining until the
      // queue is empty too, otherwise tasks still waiting for a slot are
      // neither awaited nor counted
      const drain = () => {
        if (this.#queue.length === 0 && this.#results.size === 0) {
          resolve(this.#total)
          return
        }

        Promise.all(this.#results).then(drain).catch(reject)
      }

      drain()
    })
  }
}
