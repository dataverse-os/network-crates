struct ContentFolder {
    fs_version: String,
    index_folder_id: String,
    mirror_file_ids: Vec<String>,
    encrypted_file_keys: Option<String>,
    reserved: Option<String>,
}
