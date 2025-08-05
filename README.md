# KTranslator - Real-time Screen Translator

This program is a Real-time Screen Translator. I made this program for translating subtitles in games and other applications.

## Features
- Real-time OCR (Optical Character Recognition) from screen
- Support for multiple languages
- Live translation using Google Translate
- Easy-to-use GUI interface
- Always-on-top window

## Screenshots
OLD VERSION<br>
<img src="https://github.com/user-attachments/assets/32056c17-acd5-4118-ba78-098dddb71b1a" width="300" height="140"><br>

NEW<br>
<img src="https://github.com/user-attachments/assets/511d0a11-b960-4272-8dfc-69dc18607784" width="300" height="170"><br>

FINAL<br>
<img src="https://github.com/user-attachments/assets/f107b762-e367-46ee-8a8f-184612e2984d" width="300" height="170"><br>

## Requirements

### Required Program: Tesseract OCR
Download and install from: https://sourceforge.net/projects/tesseract-ocr.mirror/

### Python Libraries
Install using pip:
```bash
pip install deep-translator pillow pytesseract numpy
```

## Installation

### Method 1: Direct Installation
```bash
# Clone or download the repository
git clone https://github.com/yourusername/KTranslator.git
cd KTranslator

# Install dependencies
pip install -r requirements.txt

# Run the program
python run.py
# or
python KTranslator.py
```

### Method 2: Using setup.py
```bash
# Install as a package
pip install -e .

# Run using console script
ktranslator
```

## Usage
1. Run `python run.py` or `python KTranslator.py`
2. Select OCR language (language of text on screen)
3. Select translation language (target language)
4. Click "Select Area" to choose screen region
5. Click "Start Translation" to begin real-time translation
6. Click "Stop Translation" to stop

## Project Structure
```
KTranslator/
├── KTranslator.py          # Main program
├── run.py                  # Launcher script
├── requirements.txt        # Python dependencies
├── setup.py               # Package setup
├── MANIFEST.in            # Distribution manifest
├── README.md              # This file
└── data/                  # Data files directory
    ├── languages.csv      # Language codes for Google Translate
    └── ocr_mapping.csv    # OCR language mapping
```
