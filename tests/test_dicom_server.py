from unittest.mock import patch, MagicMock
from src.dicom_server import DICOMServer
import pytest
import sys
import os

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '../src')))


@pytest.fixture
def mock_dicom_event():
    event = MagicMock()
    event.dataset.StudyInstanceUID = "1.2.840.10008.1"
    event.dataset.SeriesInstanceUID = "1.2.840.10008.2"
    event.dataset.SOPInstanceUID = "1.2.840.10008.3"
    event.file_meta.TransferSyntaxUID = "1.2.840.10008.1.2.1"
    return event


@patch("src.dicom_server.os.makedirs")
@patch("src.dicom_server.asyncio.create_task")
def test_handle_store(mock_create_task, mock_makedirs, mock_dicom_event):
    # Arrange
    config = {
        'storage': {'base_dir': '/tmp'},
    }
    dicom_server = DICOMServer(config)

    # Act
    status = dicom_server.handle_store(mock_dicom_event)

    # Assert
    assert status == 0x0000  # DICOM success status
    mock_makedirs.assert_called_once()
    mock_create_task.assert_called_once()
