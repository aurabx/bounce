import os
import time
import logging
import asyncio
from concurrent.futures import ThreadPoolExecutor
from pynetdicom import AE, evt, AllStoragePresentationContexts
from transmission import send_archive

logger = logging.getLogger(__name__)


class DICOMServer:
    def __init__(self, config):
        self.config = config
        self.ae = AE()
        self.ae.supported_contexts = AllStoragePresentationContexts
        self.handlers = [(evt.EVT_C_STORE, self.handle_store)]
        self.study_timers = {}  # Track asyncio tasks for each study
        self.study_last_received = {}  # Track last file received time for each study
        self.loop = asyncio.get_event_loop()  # Main event loop
        self.executor = ThreadPoolExecutor(max_workers=5)  # Thread pool for offloading tasks

    def start_in_thread(self):
        """Start the DICOM server in a separate thread."""
        logger.info(f"Starting DICOM server on {self.config['dicom']['host']}:{self.config['dicom']['port']}")
        self.ae.start_server((self.config['dicom']['host'], self.config['dicom']['port']), evt_handlers=self.handlers)

    def handle_store(self, event):
        """Handle incoming C-STORE requests synchronously."""
        ds = event.dataset
        ds.file_meta = event.file_meta

        study_id = ds.StudyInstanceUID
        series_id = ds.SeriesInstanceUID
        sop_instance = ds.SOPInstanceUID

        # Construct the path where the file will be saved
        study_path = f"{self.config['storage']['base_dir']}/{study_id}/{series_id}"
        os.makedirs(study_path, exist_ok=True)  # Ensure the directory exists
        file_path = f"{study_path}/{sop_instance}.dcm"

        logger.info(f"Saving DICOM file to {file_path}")

        try:
            # Save the DICOM file synchronously
            ds.save_as(file_path, write_like_original=False)
            logger.info(f"Stored DICOM file: {file_path}")

            # Update last received time for the study
            self.study_last_received[study_id] = time.time()

            # Cancel any existing task for this study
            if study_id in self.study_timers:
                self.study_timers[study_id].cancel()

            # Start a new asyncio task to handle the timeout in a thread-safe way
            timeout = self.config.get('timeout', 60)  # Default to 60 seconds
            try:
                logger.info(f"Scheduling push for study {study_id} with timeout {timeout}")

                # Check if event loop is running
                if self.loop.is_running():
                    logger.info("Event loop is running, scheduling coroutine...")
                else:
                    logger.error("Event loop is not running!")

                # Log the current thread to check threading issues
                import threading
                logger.info(f"Current thread: {threading.current_thread().name}")

                # Use executor to offload the task to a separate thread
                self.loop.run_in_executor(self.executor, self.schedule_study_push, study_id, timeout)

            except Exception as e:
                logger.error(f"Failed to schedule study push for {study_id}: {e}")

            return 0x0000  # Success status

        except Exception as e:
            logger.error(f"Failed to save DICOM file: {e}")
            return 0xC000  # Failure status

    def schedule_study_push(self, study_id, timeout):
        """Run the asynchronous schedule task in a separate thread using an executor."""
        asyncio.run(self._schedule_study_push_async(study_id, timeout))

    async def _schedule_study_push_async(self, study_id, timeout):
        """Asynchronous version of schedule_study_push."""
        logger.info(f"Scheduling study push for {study_id} with a timeout of {timeout} seconds.")
        try:
            # Sleep for the duration of the timeout
            await asyncio.sleep(timeout)
            logger.info(f"Timeout reached for study {study_id}. Preparing to push the study.")

            # Log the last time a file was received for this study (debugging info)
            last_received = self.study_last_received.get(study_id, None)
            if last_received:
                time_since_last = time.time() - last_received
                logger.info(f"Last DICOM file received for study {study_id} was {time_since_last:.2f} seconds ago.")

            # After the timeout, push the study
            await self.push_study(study_id)

        except asyncio.CancelledError:
            logger.info(f"Timeout for study {study_id} was cancelled due to new file arrival.")
            # If the task was cancelled, simply return

    async def push_study(self, study_id):
        """Send the study after the timeout."""
        logger.info(f"Pushing study {study_id} after timeout.")

        study_path = f"{self.config['storage']['base_dir']}/{study_id}"
        logger.info(f"Study path for {study_id}: {study_path}")

        if not os.path.exists(study_path):
            logger.error(f"Study path {study_path} does not exist. Cannot push study.")
            return

        try:
            # Send the archive (compression is handled within send_archive)
            logger.info(f"Sending archive for study {study_id}")
            await send_archive(
                self.config['transmission']['api_endpoint'],
                self.config['transmission']['api_key'],
                study_path,
                delete_after_send=self.config.get('delete_after_send', False)
            )
            logger.info(f"Study {study_id} sent successfully.")
        except Exception as e:
            logger.error(f"Failed to send study {study_id}: {e}")
