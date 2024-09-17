# Bounce

Bounce is a small service that runs within your network and sends any received files to Aurabox.

Bounce is still under development and probably shouldn't be used in a production environment.

## Build

Running the Python Application on macOS

### Step 1: Ensure Python is Installed
Check if you have Python 3 installed:

```bash
python3 --version
```
If Python 3 is not installed, you can install it using Homebrew:

```bash
brew install python
```

### Step 2: Set Up a Virtual Environment
Create a Virtual Environment:

Navigate to your project directory and create a virtual environment:

```bash
python3 -m venv venv
```
Activate the Virtual Environment:

```bash
source venv/bin/activate
```

Install Dependencies:

If you have a requirements.txt file, install all the required packages using:

```bash
pip install -r requirements.txt
```

If you don’t have a requirements.txt, install the necessary dependencies manually. For example:

```bash
pip install pynetdicom asyncio pytest-asyncio
```

### Step 3: Run the Application
After setting up the environment and dependencies, you can run your Python application. For example, if your main script is main.py, you can run it using:

```bash
python src/main.py
python src/main.py --port 105 --destination "https://webhook.site/98745971-f3a3-4c4b-91ce-402bb6eff845" --api_key "my_secure_api_key" --storage "/custom/dicom_storage" --delete-after-send

```

##Packaging the Application as a Standalone Executable on macOS

If you want to package your Python application as a standalone executable so that it can run without manually setting up a Python environment, you can use PyInstaller.

### Step 1: Install PyInstaller

With your virtual environment activated, install PyInstaller:

```bash
pip install pyinstaller
```

### Step 2: Package the Application Using PyInstaller

Run the PyInstaller Command:

Assuming your main Python script is src/main.py, and you want to package it into a standalone executable, use this command:

```bash
pyinstaller --onefile --windowed --name "Bounce" src/main.py
```

--onefile: Packages everything into a single executable file.
--windowed: Ensures the app doesn’t open a terminal window (for GUI apps).
--name "Bounce": Sets the name of the final application to "Bounce".

### Step 3: Locate the Packaged Application

After the PyInstaller build process finishes, you’ll find the packaged app in the dist/ directory. You can run it like any other macOS application:

```bash
cd dist/
./Bounce
```

## Running the Application as a Background Service (Optional)

You can run your application as a background service on macOS using launchd.

### Step 1: Create a plist File for launchd
Create a file named com.mycompany.bounce.plist in ~/Library/LaunchAgents/ with the following content:

```xml

<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.mycompany.bounce</string>
    <key>ProgramArguments</key>
    <array>
        <string>/path/to/your/executable</string> <!-- or the Python script -->
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/bounce.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/bounce.err</string>
</dict>
</plist>
```

### Step 2: Load the plist File

Load the agent to start your application in the background:

```bash
launchctl load ~/Library/LaunchAgents/com.mycompany.bounce.plist
```

This will run your application as a background service.
