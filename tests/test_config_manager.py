import pytest
from src.config_manager import ConfigManager

@pytest.fixture
def mock_env(monkeypatch):
    # Mock environment variables
    monkeypatch.setenv('ENCRYPTION_KEY', 'test_encryption_key')
    monkeypatch.setenv('API_KEY', 'test_api_key')
    monkeypatch.setenv('API_ENDPOINT', 'https://test.example.com')
    monkeypatch.setenv('DICOM_PORT', '105')
    monkeypatch.setenv('DICOM_HOST', '127.0.0.1')
    monkeypatch.setenv('STORAGE_DIR', '/custom/dicom_storage')

def test_load_config(mock_env):
    # Load the configuration with environment variable overrides
    config = ConfigManager.load_config()

    # Assert that environment variables are correctly loaded
    assert config['encryption']['key'] == "test_encryption_key"
    assert config['transmission']['api_key'] == "test_api_key"
    assert config['transmission']['api_endpoint'] == "https://test.example.com"
    assert config['dicom']['port'] == 105  # Port should be converted to an integer
    assert config['dicom']['host'] == "127.0.0.1"
    assert config['storage']['base_dir'] == "/custom/dicom_storage"
