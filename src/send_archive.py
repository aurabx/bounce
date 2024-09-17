import asyncio

import requests
from logger import get_logger

logger = get_logger(__name__)


async def send_archive(api_endpoint, api_key, encrypted_file_path, checksum, max_retries=5):
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/octet-stream",
        "X-Checksum": checksum
    }
    with open(encrypted_file_path, "rb") as f:
        data = f.read()

    for attempt in range(max_retries):
        try:
            response = requests.post(api_endpoint, headers=headers, data=data, timeout=30)
            response.raise_for_status()
            return response.json()
        except requests.RequestException as e:
            logger.error(f"Attempt {attempt + 1}: Failed to send archive - {e}")
            if attempt < max_retries - 1:
                backoff = 2 ** attempt
                logger.info(f"Retrying in {backoff} seconds...")
                await asyncio.sleep(backoff)
            else:
                logger.error(f"All {max_retries} attempts failed.")
                raise e
