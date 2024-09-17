import pytest
from unittest.mock import patch, MagicMock
from src.dicom_server import DICOMServer
import asyncio


@pytest.fixture
def mock_dicom_event():
    # Mock a DICOM event object
    event = MagicMock()
    event.dataset.StudyInstanceUID = "1.2.3.4.5"
    event.dataset.SeriesInstanceUID = "1.2.3.4.5.6"
    event.dataset.SOPInstanceUID = "1.2.3.4.5.6.7"
    return event


@pytest.fixture
def mock_transmission():
    with patch('src.transmission.send_archive', new_callable=MagicMock) as mock_send:
        # Simulate an asynchronous return by setting up a future
        future = asyncio.Future()
        future.set_result({"message": "Success"})  # Set the future result
        mock_send.return_value = future  # Mock send_archive to return the future
        yield mock_send


@pytest.mark.asyncio
@patch("src.dicom_server.os.makedirs")
@patch("src.dicom_server.asyncio.create_task")
async def test_end_to_end(mock_makedirs, mock_create_task, mock_dicom_event, mock_transmission):
    # Mock config
    config = {
        'storage': {'base_dir': '/tmp'},
        'transmission': {'api_endpoint': 'https://api.example.com', 'api_key': 'dummy_key'}
    }

    # Create the DICOMServer instance
    dicom_server = DICOMServer(config)

    # Simulate receiving a DICOM study
    status = dicom_server.handle_store(mock_dicom_event)

    # Ensure the study is processed and the file is transmitted successfully
    assert status == 0x0000
    mock_makedirs.assert_called_once()

    # Simulate the asynchronous task returned by create_task
    mock_task = asyncio.Future()
    mock_task.set_result(None)  # Simulate the task completing successfully
    mock_create_task.return_value = mock_task  # Mock create_task to return the future

    # Ensure the task is awaited
    await mock_create_task.return_value  # Await the mocked task

    # Assert that create_task and transmission were called
    mock_create_task.assert_called_once()

    # Assert that send_archive was called once
    mock_transmission.assert_called_once()
