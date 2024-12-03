use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use zip::{
    write::ZipWriter,
    write::SimpleFileOptions,
    CompressionMethod
};
use walkdir::{DirEntry, WalkDir};
use std::io::{Seek, Write};
use reqwest::{Client, Body};
use tokio_util::io::ReaderStream;
use tokio::{fs, fs::File};
use tokio::io::AsyncReadExt;
use zip::result::ZipError;

pub struct Transmission {
    client: Client,
    api_endpoint: String,
    api_key: String,
}

impl Transmission {
    pub fn new(api_endpoint: String, api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_endpoint,
            api_key,
        }
    }

    pub async fn send_archive(
        &self,
        study_path: &Path,
        delete_after_send: bool
    ) -> Result<()> {

        let archive_path = self.compress_study(study_path).await?;

        // Send archive
        let file = File::open(&archive_path).await?;
        // let file_size = file.metadata()?.len();
        let stream = ReaderStream::new(file);
        let body_stream = Body::wrap_stream(stream);

        let response = self.client.post(&self.api_endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/octet-stream")
            .timeout(Duration::from_secs(30))
            .body(body_stream)
            .send()
            .await?;

        if response.status().is_success() {
            if delete_after_send {
                self.delete_local_study_files(study_path).await?;
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to send archive. Status: {}", response.status()))
        }
    }


    pub async fn zip_folder<T>(
        &self,
        it: &mut dyn Iterator<Item = DirEntry>,
            prefix: &Path,
            writer: T
        ) -> anyhow::Result<()>
        where
            T: Write + Seek,
        {
            let mut zip = ZipWriter::new(writer);
            let options = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .unix_permissions(0o755);

            let prefix = Path::new(prefix);
            let mut buffer = Vec::new();
            for entry in it {
                let path = entry.path();
                let name = path.strip_prefix(prefix).unwrap();
                let path_as_string = name
                    .to_str()
                    .map(str::to_owned)
                    .with_context(|| format!("{name:?} Is a Non UTF-8 Path"))?;

                // Write file or directory explicitly
                // Some unzip tools unzip files with directory paths correctly, some do not!
                if path.is_file() {
                    println!("adding file {path:?} as {name:?} ...");
                    zip.start_file(path_as_string, options)?;
                    let mut f = File::open(path).await?;

                    f.read_to_end(&mut buffer).await?;
                    zip.write_all(&buffer)?;
                    buffer.clear();
                } else if !name.as_os_str().is_empty() {
                    // Only if not root! Avoids path spec / warning
                    // and mapname conversion failed error on unzip
                    println!("adding dir {path_as_string:?} as {name:?} ...");
                    zip.add_directory(path_as_string, options)?;
                }
            }
            zip.finish()?;
            Ok(())
        }

    async fn compress_study(&self, study_path: &Path) -> Result<PathBuf> {
        let archive_path = study_path.with_extension("zip");


        if !Path::new(study_path).is_dir() {
            return Err(ZipError::FileNotFound.into());
        }

        let path = Path::new(&archive_path);
        let file = std::fs::File::create(path)?;

        let walkdir = WalkDir::new(study_path);
        let it = walkdir.into_iter();

        self.zip_folder(&mut it.filter_map(|e| e.ok()), study_path, file).await?;

        Ok(archive_path)

    }

    async fn delete_local_study_files(&self, study_path: &Path) -> Result<()> {
        fs::remove_dir_all(study_path)
            .await
            .context("Failed to delete local study files")?;

        Ok(())
    }

}