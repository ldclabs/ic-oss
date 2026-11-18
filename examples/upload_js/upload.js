import { readFile, writeFile } from 'node:fs/promises'
import { HttpAgent } from '@icp-sdk/core/agent'
import { Ed25519KeyIdentity } from '@icp-sdk/core/identity'
import { Principal } from '@icp-sdk/core/principal'
import { BucketCanister, Uploader } from '@ldclabs/ic_oss_ts'

async function main() {
  const args = process.argv.slice(2)
  if (args[0] === '--new-identity' && args.length === 2) {
    const identity = Ed25519KeyIdentity.generate()
    await writeFile(args[1], JSON.stringify(identity.toJSON()), {
      flag: 'wx',
      mode: 0o600
    })
    console.log(identity.getPrincipal().toText())
    return
  }

  const [bucket, identityPath, ...files] = args
  if (!bucket || !identityPath || files.length === 0) {
    throw new Error(
      'Usage: node upload.js BUCKET_ID IDENTITY_JSON FILE [FILE ...]'
    )
  }

  const canisterId = Principal.fromText(bucket)
  const identity = Ed25519KeyIdentity.fromJSON(
    await readFile(identityPath, 'utf8')
  )
  const host = process.env.IC_HOST ?? 'http://127.0.0.1:4943'
  const agent = await HttpAgent.create({
    host,
    identity,
    shouldFetchRootKey: ['localhost', '127.0.0.1'].includes(
      new URL(host).hostname
    )
  })
  const uploader = new Uploader(BucketCanister.create({ agent, canisterId }))

  for (const content of files) {
    // Empty name and content type are inferred from the local path.
    const result = await uploader.upload({ content, name: '', contentType: '' })
    console.log(content, result)
  }
}

main().catch((error) => {
  console.error(error)
  process.exitCode = 1
})
