import os
import tarfile


def compress_study(source_dir, output_tar_gz):
    with tarfile.open(output_tar_gz, "w:gz") as tar:
        # Add each file inside the directory, without preserving the folder structure
        for root, _, files in os.walk(source_dir):
            for file in files:
                file_path = os.path.join(root, file)
                tar.add(file_path, arcname=file)  # arcname=file ensures only the file name is added

