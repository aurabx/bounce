import argparse
import asyncio
import os
import signal
import threading
from dicom_server import DICOMServer
from logger import get_logger

logger = get_logger(__name__)


def parse_arguments():
    """Parse command-line arguments and return the configuration."""
    parser = argparse.ArgumentParser(description='DICOM Server Configuration')
    parser.add_argument('--port', type=int, default=104, help='Port for the DICOM server to listen on')
    parser.add_argument('--destination', type=str, default='https://api.example.com',
                        help='Destination URL for transmission')
    parser.add_argument('--api_key', type=str, required=True, help='API Key for transmission authentication')
    parser.add_argument('--storage', type=str, default='/tmp/dicom_storage', help='Base directory to store DICOM files')
    parser.add_argument('--delete-after-send', action='store_true',
                        help='Delete local files after they are successfully sent')

    args = parser.parse_args()
    config = {
        'dicom': {'host': '0.0.0.0', 'port': args.port},
        'transmission': {'api_endpoint': args.destination, 'api_key': args.api_key},
        'storage': {'base_dir': args.storage},
        'delete_after_send': args.delete_after_send
    }

    return config


async def shutdown(loop, dicom_server):
    """Cleanup tasks tied to the event loop."""
    logger.info("Shutting down DICOM server and event loop...")

    # Gracefully stop the DICOM server if it's running
    if dicom_server:
        try:
            logger.info("Attempting to shut down the DICOM server...")
            dicom_server.shutdown()

            # Join the thread to make sure it finishes
            if dicom_server.server_thread:
                logger.info("Joining DICOM server thread...")
                dicom_server.server_thread.join(timeout=5)  # wait for up to 5 seconds
                logger.info("DICOM server thread joined.")
        except Exception as e:
            logger.error(f"Error shutting down DICOM server: {e}")

    # Cancel all running tasks
    tasks = [task for task in asyncio.all_tasks() if task is not asyncio.current_task()]
    for task in tasks:
        task.cancel()

    await asyncio.gather(*tasks, return_exceptions=True)
    logger.info("All tasks cancelled.")

    # Stop the event loop
    loop.stop()
    logger.info("Event loop stopped.")

    # Force exit in case any threads are hanging
    os._exit(0)


def handle_exit(loop, dicom_server):
    """Handler for SIGINT (Ctrl+C) to trigger shutdown."""
    logger.info("SIGINT received. Exiting...")
    asyncio.ensure_future(shutdown(loop, dicom_server))


async def start_server(dicom_server):
    """Start the DICOM server asynchronously in a separate thread."""
    try:
        dicom_server.start_in_thread()
    except Exception as e:
        logger.error(f"Failed to start DICOM server: {e}")
        os._exit(1)


def main():
    config = parse_arguments()

    loop = asyncio.get_event_loop()

    # Initialize and start the DICOM server
    dicom_server = DICOMServer(config)
    loop.create_task(start_server(dicom_server))

    # Setup a signal handler to directly catch SIGINT (Ctrl+C)
    signal.signal(signal.SIGINT, lambda s, f: handle_exit(loop, dicom_server))

    try:
        logger.info("Starting event loop...")
        loop.run_forever()
    finally:
        logger.info("Closing the event loop.")
        pending = asyncio.all_tasks(loop)
        loop.run_until_complete(asyncio.gather(*pending, return_exceptions=True))
        loop.run_until_complete(loop.shutdown_asyncgens())
        loop.close()


if __name__ == "__main__":
    main()
