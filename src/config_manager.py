import os


class ConfigManager:
    @staticmethod
    def load_config():
        # Fetch configuration from environment variables with defaults
        config = {
            'encryption': {
                'key': os.getenv('ENCRYPTION_KEY', 'default_key')  # Default key if ENV is not set
            },
            'transmission': {
                'api_key': os.getenv('API_KEY', 'default_api_key'),  # Default API key
                'api_endpoint': os.getenv('API_ENDPOINT', 'https://api.example.com')  # Default endpoint
            },
            'dicom': {
                'port': int(os.getenv('DICOM_PORT', 104)),  # Default DICOM port
                'host': os.getenv('DICOM_HOST', '0.0.0.0')  # Default host
            },
            'storage': {
                'base_dir': os.getenv('STORAGE_DIR', '/tmp/dicom_storage')  # Default storage directory
            }
        }
        return config
