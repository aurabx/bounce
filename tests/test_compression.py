import os
import tarfile
import pytest
from src.compression import compress_study
import sys

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '../src')))


@pytest.fixture
def test_dir(tmpdir):
    # Create a temporary directory with some test files
    test_file_dir = tmpdir.mkdir("study")
    test_file = test_file_dir.join("test_file.dcm")
    test_file.write("Test DICOM content")
    return test_file_dir


def test_compress_study(test_dir):
    # Define the path for the output compressed file
    compressed_file = str(test_dir) + ".tar.gz"

    # Call the function to compress the directory
    compress_study(test_dir, compressed_file)

    # Assert that the .tar.gz file was created
    assert os.path.exists(compressed_file)

    # Verify that the tar file contains the expected file
    with tarfile.open(compressed_file, "r:gz") as tar:
        tar_files = tar.getnames()
        assert "test_file.dcm" in tar_files
