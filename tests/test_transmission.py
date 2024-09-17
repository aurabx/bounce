import pytest
from unittest.mock import patch
from src.transmission import send_archive


# Mocking an API response for successful transmission
@pytest.fixture
def mock_successful_response():
    with patch('requests.post') as mock_post:
        mock_post.return_value.status_code = 200
        mock_post.return_value.json.return_value = {"message": "Success"}
        yield mock_post


@pytest.mark.asyncio
async def test_send_archive_success(mock_successful_response, tmpdir):
    # Arrange
    test_file = tmpdir.join("encrypted_file.enc")
    test_file.write("Encrypted content")

    api_endpoint = "https://example.com/upload"
    api_key = "dummy_api_key"
    checksum = "dummy_checksum"

    # Act
    response = await send_archive(api_endpoint, api_key, str(test_file), checksum)  # Await the async function

    # Assert
    assert response['message'] == "Success"
