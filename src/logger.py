import logging


def get_logger(name):
    # Configure logging programmatically (no need for YAML)
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s [%(levelname)s] %(message)s",
        handlers=[
            logging.StreamHandler(),  # Logs to the console
        ]
    )

    # Return a logger instance
    return logging.getLogger(name)
