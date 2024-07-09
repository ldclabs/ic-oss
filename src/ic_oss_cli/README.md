# `ic-oss-cli`
> A CLI tool of ic-oss

`ic-oss` is a fully open-source decentralized object storage service running on the Internet Computer. It provides a simple and efficient way to store and retrieve files, supports large files, and offers unlimited horizontal scalability. It can serve as a reliable decentralized file infrastructure for NFT, chain blocks, verifiable credentials, blogs, documents, knowledge bases, games and other decentralized applications.

## Usage

Install:
```sh
cargo install ic-oss-cli
# get help info
ic-oss-cli --help
ic-oss-cli identity --help
ic-oss-cli upload --help

# Generate a new identity
ic-oss-cli identity --new --file myid.pem
# Output:
# principal: lxph3-nvpsv-yrevd-im4ug-qywcl-5ir34-rpsbs-6olvf-qtugo-iy5ai-jqe
# new identity: myid.pem
```

Upload a file to the local canister:
```sh
ic-oss-cli -i myid.pem upload -b bkyz2-fmaaa-aaaaa-qaaaq-cai --file test.tar.gz
```

Upload a file to the mainnet canister:
```sh
ic-oss-cli -i myid.pem upload -b bkyz2-fmaaa-aaaaa-qaaaq-cai --file test.tar.gz --ic
```