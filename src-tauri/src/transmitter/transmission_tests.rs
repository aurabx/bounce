#[cfg(test)]
mod tests {
    use crate::transmitter::transmission::Transmission;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;
    use walkdir::WalkDir;

    #[tokio::test]
    async fn test_zip_folder_static() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let study_dir = temp_dir.path().join("study");
        let series_dir = study_dir.join("series1");
        std::fs::create_dir_all(&series_dir).expect("Failed to create directory structure");

        // Create some test files
        let file1_path = series_dir.join("image1.dcm");
        let mut file1 = File::create(&file1_path).expect("Failed to create file1");
        file1
            .write_all(b"DICOM data 1")
            .expect("Failed to write file1");

        let file2_path = series_dir.join("image2.dcm");
        let mut file2 = File::create(&file2_path).expect("Failed to create file2");
        file2
            .write_all(b"DICOM data 2")
            .expect("Failed to write file2");

        // Create zip file path
        let zip_path = temp_dir.path().join("output.zip");
        let zip_file = File::create(&zip_path).expect("Failed to create zip file");

        // Prepare iterator
        let walkdir = WalkDir::new(&study_dir);
        let it = walkdir.into_iter().filter_map(|e| e.ok());

        // Call the static method
        let result = Transmission::zip_folder(it, &study_dir, zip_file).await;

        assert!(result.is_ok());
        assert!(zip_path.exists());
        assert!(zip_path.metadata().unwrap().len() > 0);

        // Verify zip content
        let file = File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        assert_eq!(archive.len(), 3); // Two files + one directory ("series1/")

        let mut zip_file1 = archive.by_name("series1/image1.dcm").unwrap();
        let mut content = Vec::new();
        std::io::Read::read_to_end(&mut zip_file1, &mut content).unwrap();
        assert_eq!(content, b"DICOM data 1");
    }
}
