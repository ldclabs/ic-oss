import { Canister, createServices } from '@dfinity/utils'
import type {
  BucketInfo,
  _SERVICE as BucketService,
  CanisterStatusResponse,
  CreateFileInput,
  CreateFileOutput,
  CreateFolderInput,
  FileInfo,
  FolderInfo,
  FolderName,
  MoveInput,
  UpdateFileChunkInput,
  UpdateFileChunkOutput,
  UpdateFileInput,
  UpdateFileOutput,
  UpdateFolderInput
} from '../candid/ic_oss_bucket/ic_oss_bucket.did.js'
import { idlFactory } from '../candid/ic_oss_bucket/ic_oss_bucket.did.js'
import type { CanisterOptions } from './types.js'
import { FileChunk, resultOk } from './types.js'

export class BucketCanister extends Canister<BucketService> {
  #resultOk: typeof resultOk = resultOk
  #accessToken: [] | [Uint8Array] = []

  static create(
    options: CanisterOptions<BucketService> & {
      accessToken?: Uint8Array
    }
  ) {
    const { service, certifiedService, canisterId } =
      createServices<BucketService>({
        options,
        idlFactory,
        certifiedIdlFactory: idlFactory
      })

    const self = new BucketCanister(canisterId, service, certifiedService)
    self.#resultOk = options.unwrapResult || resultOk
    self.#accessToken = options.accessToken ? [options.accessToken] : []
    return self
  }

  async getCanisterStatus(): Promise<CanisterStatusResponse> {
    const res = await this.service.get_canister_status()
    return this.#resultOk(res)
  }

  async getBucketInfo(): Promise<BucketInfo> {
    const res = await this.service.get_bucket_info(this.#accessToken)
    return this.#resultOk(res)
  }

  async batchDeleteSubfiles(parent: number, ids: number[]): Promise<number[]> {
    const res = await this.service.batch_delete_subfiles(
      parent,
      ids,
      this.#accessToken
    )
    return this.#resultOk(res) as number[]
  }

  async createFile(input: CreateFileInput): Promise<CreateFileOutput> {
    const res = await this.service.create_file(input, this.#accessToken)
    return this.#resultOk(res)
  }

  async createFolder(input: CreateFolderInput): Promise<CreateFileOutput> {
    const res = await this.service.create_folder(input, this.#accessToken)
    return this.#resultOk(res)
  }

  async deleteFile(id: number): Promise<boolean> {
    const res = await this.service.delete_file(id, this.#accessToken)
    return this.#resultOk(res)
  }

  async deleteFolder(id: number): Promise<boolean> {
    const res = await this.service.delete_folder(id, this.#accessToken)
    return this.#resultOk(res)
  }

  async getFileAncestors(id: number): Promise<FolderName[]> {
    const res = await this.service.get_file_ancestors(id, this.#accessToken)
    return this.#resultOk(res)
  }

  async getFolderAncestors(id: number): Promise<FolderName[]> {
    const res = await this.service.get_folder_ancestors(id, this.#accessToken)
    return this.#resultOk(res)
  }

  async getFileChunks(
    id: number,
    chunkIdex: number,
    take: number = 0
  ): Promise<FileChunk[]> {
    const res = await this.service.get_file_chunks(
      id,
      chunkIdex,
      take > 0 ? [take] : [],
      this.#accessToken
    )
    return this.#resultOk(res) as FileChunk[]
  }

  async getFileInfo(id: number): Promise<FileInfo> {
    const res = await this.service.get_file_info(id, this.#accessToken)
    return this.#resultOk(res)
  }

  async getFileInfoByHash(hash: Uint8Array): Promise<FileInfo> {
    const res = await this.service.get_file_info_by_hash(
      hash,
      this.#accessToken
    )
    return this.#resultOk(res)
  }

  async getFolderInfo(id: number): Promise<FolderInfo> {
    const res = await this.service.get_folder_info(id, this.#accessToken)
    return this.#resultOk(res)
  }

  async listFiles(
    parent: number,
    prev: number = 0,
    take: number = 0
  ): Promise<FileInfo[]> {
    const res = await this.service.list_files(
      parent,
      prev > 0 ? [prev] : [],
      take > 0 ? [take] : [],
      this.#accessToken
    )
    return this.#resultOk(res)
  }

  async listFolders(
    parent: number,
    prev: number = 0,
    take: number = 0
  ): Promise<FolderInfo[]> {
    const res = await this.service.list_folders(
      parent,
      prev > 0 ? [prev] : [],
      take > 0 ? [take] : [],
      this.#accessToken
    )
    return this.#resultOk(res)
  }

  async moveFile(input: MoveInput): Promise<UpdateFileOutput> {
    const res = await this.service.move_file(input, this.#accessToken)
    return this.#resultOk(res)
  }

  async moveFolder(input: MoveInput): Promise<UpdateFileOutput> {
    const res = await this.service.move_folder(input, this.#accessToken)
    return this.#resultOk(res)
  }

  async updateFileChunk(
    input: UpdateFileChunkInput
  ): Promise<UpdateFileChunkOutput> {
    const res = await this.service.update_file_chunk(input, this.#accessToken)
    return this.#resultOk(res)
  }

  async updateFileInfo(input: UpdateFileInput): Promise<UpdateFileOutput> {
    const res = await this.service.update_file_info(input, this.#accessToken)
    return this.#resultOk(res)
  }

  async updateFolderInfo(input: UpdateFolderInput): Promise<UpdateFileOutput> {
    const res = await this.service.update_folder_info(input, this.#accessToken)
    return this.#resultOk(res)
  }
}
