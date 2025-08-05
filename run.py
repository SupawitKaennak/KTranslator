#!/usr/bin/env python3
"""
KTranslator - Real-time Screen Translator
Simple launcher script
"""

import sys
import os

# เพิ่ม current directory ใน Python path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

try:
    from KTranslator import TranslatorApp
    
    if __name__ == "__main__":
        print("Starting KTranslator...")
        TranslatorApp()
        
except ImportError as e:
    print(f"Error importing KTranslator: {e}")
    print("Please make sure all dependencies are installed:")
    print("pip install -r requirements.txt")
    sys.exit(1)
except Exception as e:
    print(f"Error starting KTranslator: {e}")
    sys.exit(1) 