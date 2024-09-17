import pytest
from cryptography.fernet import Fernet
from src.encryption import encrypt_file
import sys
import os

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '../src')))


@pytest.fixture
def sample_file(tmpdir):
    test_file = tmpdir.join("test_file.txt")
    test_file.write("Sample text for encryption")
    return test_file


def test_encrypt_file(sample_file):
    key = Fernet.generate_key()
    encrypted_file = str(sample_file) + ".enc"

    # Call the encryption function
    encrypt_file(str(sample_file), encrypted_file, key)

    # Check that the encrypted file was created
    assert os.path.exists(encrypted_file)

    # Check that the encrypted file content is not the same as the original
    with open(encrypted_file, "rb") as ef:
        encrypted_content = ef.read()
    with open(sample_file, "rb") as sf:
        original_content = sf.read()

    assert encrypted_content != original_content
