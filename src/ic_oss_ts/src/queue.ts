export type Task = (
  aborter: AbortController,
  concurrency: number
) => Promise<void>

export class ConcurrencyQueue {
  readonly #concurrency: number
  #total = 0
  readonly #aborter = new AbortController()
  readonly #queue: {
    task: Task
    resolve: () => void
    reject: (reason: unknown) => void
  }[] = []
  readonly #pending = new Set<Promise<void>>()
  readonly #waiters = new Set<() => void>()

  constructor(concurrency: number) {
    this.#concurrency = Number.isFinite(concurrency)
      ? Math.max(1, Math.floor(concurrency))
      : 1
  }

  #next() {
    while (
      !this.#aborter.signal.aborted &&
      this.#pending.size < this.#concurrency &&
      this.#queue.length
    ) {
      const { task, resolve } = this.#queue.shift()!
      const concurrency = this.#pending.size + 1
      const result = Promise.resolve()
        .then(() => task(this.#aborter, concurrency))
        .then(() => {
          this.#total += 1
        })
        .catch((err) => this.abort(err))
        .finally(() => {
          this.#pending.delete(result)
          this.#next()
          for (const wake of this.#waiters) wake()
          this.#waiters.clear()
        })
      this.#pending.add(result)
      resolve()
    }
  }

  abort(reason: unknown) {
    if (this.#aborter.signal.aborted) return
    this.#aborter.abort(reason)
    for (const { reject } of this.#queue.splice(0)) reject(reason)
  }

  /** Resolves when the task starts, providing producer backpressure. */
  push(task: Task): Promise<void> {
    if (this.#aborter.signal.aborted)
      return Promise.reject(this.#aborter.signal.reason)
    return new Promise<void>((resolve, reject) => {
      this.#queue.push({ task, resolve, reject })
      this.#next()
    })
  }

  /** Settles running tasks before exposing the final result or first error. */
  async wait(): Promise<number> {
    while (this.#queue.length || this.#pending.size) {
      await new Promise<void>((resolve) => this.#waiters.add(resolve))
    }
    if (this.#aborter.signal.aborted) throw this.#aborter.signal.reason
    return this.#total
  }
}
