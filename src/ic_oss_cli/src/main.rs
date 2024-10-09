use candid::{pretty::candid::value::pp_value, CandidType, IDLValue, Principal};
use clap::{Parser, Subcommand};
use ic_agent::{
    identity::{AnonymousIdentity, BasicIdentity, Secp256k1Identity},
    Identity,
};
use ic_oss::agent::build_agent;
use ic_oss_types::{
    cluster::AddWasmInput,
    file::{MoveInput, CHUNK_SIZE},
    folder::CreateFolderInput,
    format_error,
};
use ring::{rand, signature::Ed25519KeyPair};
use serde_bytes::{ByteArray, ByteBuf};
use sha3::{Digest, Sha3_256};
use std::{
    io::SeekFrom,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

mod file;

use file::upload_file;

static IC_HOST: &str = "https://icp-api.io";

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// The user identity to run this command as.
    #[arg(short, long, value_name = "PEM_FILE", default_value = "Anonymous")]
    identity: String,

    /// The host to connect to. it will be set to "https://icp-api.io" with option '--ic'
    #[arg(long, default_value = "http://127.0.0.1:4943")]
    host: String,

    /// Use the ic network
    #[arg(long, default_value = "false")]
    ic: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

impl Cli {
    async fn bucket(
        &self,
        identity: Box<dyn Identity>,
        ic: &bool,
        bucket: &str,
    ) -> Result<ic_oss::bucket::Client, String> {
        let is_ic = *ic || self.ic;
        let host = if is_ic { IC_HOST } else { self.host.as_str() };
        let agent = build_agent(host, identity).await?;
        let bucket = Principal::from_text(bucket).map_err(format_error)?;
        Ok(ic_oss::bucket::Client::new(Arc::new(agent), bucket))
    }

    async fn cluster(
        &self,
        identity: Box<dyn Identity>,
        ic: &bool,
        cluster: &str,
    ) -> Result<ic_oss::cluster::Client, String> {
        let is_ic = *ic || self.ic;
        let host = if is_ic { IC_HOST } else { self.host.as_str() };
        let agent = build_agent(host, identity).await?;
        let cluster = Principal::from_text(cluster).map_err(format_error)?;
        Ok(ic_oss::cluster::Client::new(Arc::new(agent), cluster))
    }
}

#[derive(Subcommand)]
pub enum Commands {
    Identity {
        /// file path
        #[arg(long)]
        path: Option<String>,
        /// create a identity
        #[arg(long)]
        new: bool,
    },
    /// Add a bucket wasm to cluster
    ClusterAddWasm {
        /// bucket
        #[arg(short, long, value_name = "CANISTER")]
        cluster: String,

        /// wasm file path
        #[arg(long)]
        path: String,

        /// description
        #[arg(short, long, default_value = "")]
        description: String,

        /// previous wasm hash
        #[arg(long)]
        prev_hash: Option<String>,

        /// Use the ic network
        #[arg(long, default_value = "false")]
        ic: bool,
    },
    /// Add a folder to a bucket
    Add {
        /// bucket
        #[arg(short, long, value_name = "CANISTER")]
        bucket: String,

        /// parent folder id
        #[arg(short, long, default_value = "0")]
        parent: u32,

        /// folder name
        #[arg(short, long)]
        name: String,

        /// Use the ic network
        #[arg(long, default_value = "false")]
        ic: bool,
    },
    /// Uploads a file to a bucket
    #[command(visible_alias = "upload")]
    Put {
        /// bucket
        #[arg(short, long, value_name = "CANISTER")]
        bucket: String,

        /// parent folder id
        #[arg(short, long, default_value = "0")]
        parent: u32,

        /// file path
        #[arg(long)]
        path: String,

        /// retry times
        #[arg(long, default_value = "3")]
        retry: u8,

        /// Use the ic network
        #[arg(long, default_value = "false")]
        ic: bool,

        /// digest algorithm, default is SHA3-256
        #[arg(long, default_value = "SHA3-256")]
        digest: String,
    },
    /// Downloads an file from a target bucket to the local file system
    Get {
        /// bucket
        #[arg(short, long, value_name = "CANISTER")]
        bucket: String,

        /// downloads file by id
        #[arg(long)]
        id: Option<u32>,

        /// downloads file by hash
        #[arg(long)]
        hash: Option<String>,

        /// file path to save
        #[arg(long, default_value = "./")]
        path: String,

        /// Use the ic network
        #[arg(long, default_value = "false")]
        ic: bool,

        /// digest algorithm to verify the file, default is SHA3-256
        #[arg(long, default_value = "SHA3-256")]
        digest: String,
    },
    /// Lists files or folders in a folder
    Ls {
        /// bucket
        #[arg(short, long, value_name = "CANISTER")]
        bucket: String,

        /// parent folder id
        #[arg(short, long, default_value = "0")]
        parent: u32,

        /// kind 0: file, 1: folder
        #[arg(short, long, default_value = "0")]
        kind: u8,

        /// Use the ic network
        #[arg(long, default_value = "false")]
        ic: bool,
    },
    /// Displays information on file, folder, or bucket, including metadata
    Stat {
        /// bucket
        #[arg(short, long, value_name = "CANISTER")]
        bucket: String,

        /// file or folder id
        #[arg(long, default_value = "0")]
        id: u32,

        /// kind 0: file, 1: folder, other: bucket
        #[arg(short, long, default_value = "0")]
        kind: u8,

        /// Use the ic network
        #[arg(long, default_value = "false")]
        ic: bool,

        /// Displays file information by file hash
        #[arg(long)]
        hash: Option<String>,
    },
    /// Removes file or folder from a bucket
    Mv {
        /// bucket
        #[arg(short, long, value_name = "CANISTER")]
        bucket: String,

        /// file or folder id
        #[arg(long)]
        id: u32,

        /// file or folder's parent id
        #[arg(long)]
        from: u32,

        /// target folder id
        #[arg(long)]
        to: u32,

        /// kind 0: file, 1: folder
        #[arg(short, long, default_value = "0")]
        kind: u8,

        /// Use the ic network
        #[arg(long, default_value = "false")]
        ic: bool,
    },
    /// Removes file or folder from a bucket
    Rm {
        /// bucket
        #[arg(short, long, value_name = "CANISTER")]
        bucket: String,

        /// file or folder id
        #[arg(long)]
        id: u32,

        /// kind 0: file, 1: folder
        #[arg(short, long, default_value = "0")]
        kind: u8,

        /// Use the ic network
        #[arg(long, default_value = "false")]
        ic: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let cli = Cli::parse();
    let identity = load_identity(&cli.identity).map_err(format_error)?;

    match &cli.command {
        Some(Commands::Identity { new, path }) => {
            if !new {
                let principal = identity.sender()?;
                println!("principal: {}", principal);
                return Ok(());
            }

            let doc =
                Ed25519KeyPair::generate_pkcs8(&rand::SystemRandom::new()).map_err(format_error)?;

            let doc = pem::Pem::new("PRIVATE KEY", doc.as_ref());
            let doc = pem::encode(&doc);
            let id = BasicIdentity::from_pem(doc.as_bytes()).map_err(format_error)?;
            let principal = id.sender()?;

            let file = match path {
                Some(path) => Path::new(path).to_path_buf(),
                None => PathBuf::from(format!("{}.pem", principal)),
            };

            if file.try_exists().unwrap_or_default() {
                Err(format!("file already exists: {:?}", file))?;
            }

            std::fs::write(&file, doc.as_bytes()).map_err(format_error)?;
            println!("principal: {}", principal);
            println!("new identity: {}", file.to_str().unwrap());
            return Ok(());
        }

        Some(Commands::ClusterAddWasm {
            cluster,
            path,
            description,
            prev_hash,
            ic,
        }) => {
            let cli = cli.cluster(identity, ic, cluster).await?;
            let wasm = std::fs::read(path).map_err(format_error)?;
            let prev_hash = prev_hash.as_ref().map(|s| parse_file_hash(s)).transpose()?;
            cli.admin_add_wasm(
                AddWasmInput {
                    wasm: ByteBuf::from(wasm),
                    description: description.to_owned(),
                },
                prev_hash,
            )
            .await
            .map_err(format_error)?;
            return Ok(());
        }

        Some(Commands::Add {
            bucket,
            parent,
            name,
            ic,
        }) => {
            let cli = cli.bucket(identity, ic, bucket).await?;
            let folder = cli
                .create_folder(CreateFolderInput {
                    parent: *parent,
                    name: name.clone(),
                })
                .await
                .map_err(format_error)?;
            pretty_println(&folder)?;
            return Ok(());
        }

        Some(Commands::Put {
            bucket,
            parent,
            path,
            retry,
            ic,
            digest,
        }) => {
            if digest != "SHA3-256" {
                Err("unsupported digest algorithm".to_string())?;
            }
            let cli = cli.bucket(identity, ic, bucket).await?;
            let info = cli.get_bucket_info().await.map_err(format_error)?;
            upload_file(&cli, info.enable_hash_index, *parent, path, *retry).await?;

            return Ok(());
        }

        Some(Commands::Get {
            bucket,
            id,
            path,
            ic,
            digest,
            hash,
        }) => {
            if digest != "SHA3-256" {
                Err("unsupported digest algorithm".to_string())?;
            }
            let cli = cli.bucket(identity, ic, bucket).await?;
            let info = if let Some(hash) = hash {
                let hash = parse_file_hash(hash)?;
                cli.get_file_info_by_hash(hash)
                    .await
                    .map_err(format_error)?
            } else if let Some(id) = id {
                cli.get_file_info(*id).await.map_err(format_error)?
            } else {
                Err("missing file id or hash".to_string())?
            };

            if info.size != info.filled {
                Err("file not fully uploaded".to_string())?;
            }
            let mut f = Path::new(path).to_path_buf();
            if f.is_dir() {
                f = f.join(info.name);
            }
            let mut file = tokio::fs::File::create_new(&f)
                .await
                .map_err(format_error)?;
            file.set_len(info.size as u64).await.map_err(format_error)?;
            let mut hasher = Sha3_256::new();
            let mut filled = 0usize;
            // TODO: support parallel download
            for index in (0..info.chunks).step_by(6) {
                let chunks = cli
                    .get_file_chunks(info.id, index, Some(6))
                    .await
                    .map_err(format_error)?;
                for chunk in chunks.iter() {
                    file.seek(SeekFrom::Start(chunk.0 as u64 * CHUNK_SIZE as u64))
                        .await
                        .map_err(format_error)?;
                    hasher.update(&chunk.1);
                    file.write_all(&chunk.1).await.map_err(format_error)?;
                    filled += chunk.1.len();
                }

                println!(
                    "downloaded chunks: {}/{}, {:.2}%",
                    index as usize + chunks.len(),
                    info.chunks,
                    (filled as f32 / info.size as f32) * 100.0,
                );
            }

            let hash: [u8; 32] = hasher.finalize().into();
            if let Some(h) = info.hash {
                if *h != hash {
                    Err(format!(
                        "file hash mismatch, expected {}, got {}",
                        hex::encode(*h),
                        hex::encode(hash),
                    ))?;
                }
            }

            println!(
                "\n{}:\n{}\t{}",
                digest,
                hex::encode(hash),
                f.to_string_lossy(),
            );

            return Ok(());
        }

        Some(Commands::Ls {
            bucket,
            parent,
            kind,
            ic,
        }) => {
            let cli = cli.bucket(identity, ic, bucket).await?;
            match kind {
                0 => {
                    let files = cli
                        .list_files(*parent, None, None)
                        .await
                        .map_err(format_error)?;
                    pretty_println(&files)?;
                }
                1 => {
                    let folders = cli
                        .list_folders(*parent, None, None)
                        .await
                        .map_err(format_error)?;
                    pretty_println(&folders)?;
                }
                _ => return Err("invalid kind".to_string()),
            }
            return Ok(());
        }

        Some(Commands::Stat {
            bucket,
            id,
            kind,
            ic,
            hash,
        }) => {
            let cli = cli.bucket(identity, ic, bucket).await?;
            match kind {
                0 => {
                    let info = if let Some(hash) = hash {
                        let hash = parse_file_hash(hash)?;
                        cli.get_file_info_by_hash(hash)
                            .await
                            .map_err(format_error)?
                    } else {
                        cli.get_file_info(*id).await.map_err(format_error)?
                    };

                    pretty_println(&info)?;
                }
                1 => {
                    let info = cli.get_folder_info(*id).await.map_err(format_error)?;
                    pretty_println(&info)?;
                }
                _ => {
                    let info = cli.get_bucket_info().await.map_err(format_error)?;
                    pretty_println(&info)?;
                }
            }
            return Ok(());
        }

        Some(Commands::Mv {
            bucket,
            id,
            from,
            to,
            kind,
            ic,
        }) => {
            let cli = cli.bucket(identity, ic, bucket).await?;
            match kind {
                0 => {
                    let res = cli
                        .move_file(MoveInput {
                            id: *id,
                            from: *from,
                            to: *to,
                        })
                        .await
                        .map_err(format_error)?;
                    pretty_println(&res)?;
                }
                1 => {
                    let res = cli
                        .move_folder(MoveInput {
                            id: *id,
                            from: *from,
                            to: *to,
                        })
                        .await
                        .map_err(format_error)?;
                    pretty_println(&res)?;
                }
                _ => return Err("invalid kind".to_string()),
            }
            return Ok(());
        }

        Some(Commands::Rm {
            bucket,
            id,
            kind,
            ic,
        }) => {
            let cli = cli.bucket(identity, ic, bucket).await?;
            match kind {
                0 => {
                    let res = cli.delete_file(*id).await.map_err(format_error)?;
                    pretty_println(&res)?;
                }
                1 => {
                    let res = cli.delete_folder(*id).await.map_err(format_error)?;
                    pretty_println(&res)?;
                }
                _ => return Err("invalid kind".to_string()),
            }
            return Ok(());
        }

        None => {}
    }

    Ok(())
}

fn load_identity(path: &str) -> anyhow::Result<Box<dyn Identity>> {
    if path == "Anonymous" {
        return Ok(Box::new(AnonymousIdentity));
    }

    let content = std::fs::read_to_string(path)?;
    match Secp256k1Identity::from_pem(content.as_bytes()) {
        Ok(identity) => Ok(Box::new(identity)),
        Err(_) => match BasicIdentity::from_pem(content.as_bytes()) {
            Ok(identity) => Ok(Box::new(identity)),
            Err(err) => Err(err.into()),
        },
    }
}

fn pretty_println<T>(data: &T) -> Result<(), String>
where
    T: CandidType,
{
    let val = IDLValue::try_from_candid_type(data).map_err(format_error)?;
    let doc = pp_value(7, &val);
    println!("{}", doc.pretty(120));
    Ok(())
}

fn parse_file_hash(s: &str) -> Result<ByteArray<32>, String> {
    let s = s.replace("\\", "");
    let data = hex::decode(s.strip_prefix("0x").unwrap_or(&s)).map_err(format_error)?;
    let hash: [u8; 32] = data.try_into().map_err(format_error)?;
    Ok(hash.into())
}
