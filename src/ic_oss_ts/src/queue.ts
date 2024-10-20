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
    this.#concurrency = concurrency
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

      Promise.all(this.#results)
        .then(() => resolve(this.#total))
        .catch(reject)
    })
  }
}
