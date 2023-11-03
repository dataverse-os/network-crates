# Dataverse Crates

## Description

The core open source repository of DataverseOS.

## Crates

- `types` is the type definition of dataverse. It contains all the necessary data structures and types that are used across the DataverseOS. This crate is fundamental to the entire project as it provides a consistent way to handle data.
- `dapp-table-client` is the Rust client for dapp table. It is used to create and get information about dapps. This crate is crucial for interacting with the dapp table, allowing users to manage their dapps effectively.
- `file-system` is the basic type definition of the dataverse file system. It is used to check the access control list (ACL) of a file. This crate ensures that file permissions are handled correctly in the DataverseOS, providing a secure environment for users.
- `iroh-store` is the extension of iroh. This crate extends the functionality of the iroh crate, providing additional features such as ceramic stream and dataverse file handling.


## Getting Started

To get started with Dataverse Crates, you need to have Rust installed on your machine. Once Rust is installed, you can clone this repository and run `cargo build` to build the project.

## Contributing

We welcome contributions to Dataverse Crates! Please see our [CONTRIBUTING.md](CONTRIBUTING.md) for more information on how to contribute.

## License

Dataverse Crates is licensed under the [MIT License](LICENSE).