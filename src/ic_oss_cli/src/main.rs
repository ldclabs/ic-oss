use candid::Principal;
use clap::{Parser, Subcommand};
use ic_agent::identity::{AnonymousIdentity, BasicIdentity, Identity, Secp256k1Identity};
use ic_oss::agent::build_agent;
use ic_oss_types::format_error;
use ring::{rand, signature::Ed25519KeyPair};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

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

#[derive(Subcommand)]
pub enum Commands {
    Identity {
        /// file
        #[arg(long)]
        file: Option<String>,
        /// create a identity
        #[arg(long)]
        new: bool,
    },
    /// upload file to the ic-oss
    Upload {
        /// bucket
        #[arg(short, long, value_name = "CANISTER")]
        bucket: String,

        /// file
        #[arg(long)]
        file: String,

        /// retry times
        #[arg(long, default_value = "3")]
        retry: u8,

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
        Some(Commands::Identity { new, file }) => {
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

            let file = match file {
                Some(file) => Path::new(file).to_path_buf(),
                None => PathBuf::from(format!("{}.pem", principal)),
            };

            if file.try_exists().unwrap_or_default() {
                return Err(format!("file already exists: {:?}", file));
            }

            std::fs::write(&file, doc.as_bytes()).map_err(format_error)?;
            println!("principal: {}", principal);
            println!("new identity: {}", file.to_str().unwrap());
            return Ok(());
        }

        Some(Commands::Upload {
            bucket,
            file,
            retry,
            ic,
        }) => {
            let is_ic = *ic || cli.ic;
            let host = if is_ic { IC_HOST } else { cli.host.as_str() };
            let agent = build_agent(host, identity).await?;
            let bucket = Principal::from_text(bucket).map_err(format_error)?;
            let cli = ic_oss::bucket::Client::new(Arc::new(agent), bucket);
            upload_file(&cli, file, *retry).await?;
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
