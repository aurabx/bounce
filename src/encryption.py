from cryptography.fernet import Fernet


def encrypt_file(input_file, output_file, key):
    fernet = Fernet(key)
    with open(input_file, "rb") as f:
        data = f.read()
    encrypted = fernet.encrypt(data)
    with open(output_file, "wb") as f:
        f.write(encrypted)
