import hashlib
import logging
import shutil
import aiohttp

logger = logging.getLogger(__name__)

async def send_archive(api_endpoint, api_key, study_path, delete_after_send=False):
    """Compress, send the archive to the destination, and optionally delete local files."""
    logger.info(f"Preparing to send archive from {study_path} to {api_endpoint}")

    try:
        # Compress the study folder into a .tar.gz archive
        archive_path = await compress_study(study_path)
        logger.info(f"Successfully compressed study to {archive_path}")

        # Send the compressed archive to the external destination
        async with aiohttp.ClientSession() as session:
            with open(archive_path, 'rb') as archive_file:
                data = {'file': archive_file}
                headers = {'Authorization': f'Bearer {api_key}'}

                async with session.post(api_endpoint, data=data, headers=headers) as response:
                    if response.status == 200:
                        logger.info(f"Successfully sent archive {archive_path} to {api_endpoint}")
                    else:
                        logger.error(f"Failed to send archive {archive_path}. Status: {response.status}")
                        return  # Don't proceed to delete if the send fails

    except Exception as e:
        logger.error(f"Error sending archive {archive_path}: {e}")
        return

    # Optionally delete the local files after sending
    if delete_after_send:
        try:
            delete_local_study_files(study_path)
            logger.info(f"Successfully deleted local files for study {study_path}")
        except Exception as e:
            logger.error(f"Error while deleting local files for study {study_path}: {e}")

async def compress_study(study_path):
    """Compress the study directory into a .tar.gz archive."""
    archive_path = f"{study_path}.tar.gz"
    try:
        shutil.make_archive(study_path, 'gztar', study_path)
        return archive_path
    except Exception as e:
        logger.error(f"Error compressing study at {study_path}: {e}")
        raise

def delete_local_study_files(study_path):
    """Delete the local study files after they have been sent."""
    try:
        # Deletes the study directory and all its contents
        shutil.rmtree(study_path)  # Remove original uncompressed study directory
        logger.info(f"Local files for study {study_path} deleted successfully.")
    except Exception as e:
        logger.error(f"Error while deleting local files at {study_path}: {e}")
        raise

def compute_checksum(file_path):
    """Compute the SHA-256 checksum of a file."""
    sha256 = hashlib.sha256()
    try:
        with open(file_path, "rb") as f:
            for chunk in iter(lambda: f.read(4096), b""):
                sha256.update(chunk)
        return sha256.hexdigest()
    except Exception as e:
        logger.error(f"Error computing checksum for {file_path}: {e}")
        raise
