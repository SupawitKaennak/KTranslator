from setuptools import setup, find_packages
import os

# อ่าน README.md
def read_readme():
    with open("README.md", "r", encoding="utf-8") as fh:
        return fh.read()

# อ่าน requirements.txt
def read_requirements():
    with open("requirements.txt", "r", encoding="utf-8") as fh:
        return [line.strip() for line in fh if line.strip() and not line.startswith("#")]

setup(
    name="KTranslator",
    version="1.0.0",
    author="Your Name",
    author_email="your.email@example.com",
    description="Real-time Screen Translator with OCR and Google Translate",
    long_description=read_readme(),
    long_description_content_type="text/markdown",
    url="https://github.com/yourusername/KTranslator",
    packages=find_packages(),
    classifiers=[
        "Development Status :: 4 - Beta",
        "Intended Audience :: End Users/Desktop",
        "Topic :: Multimedia :: Graphics :: Capture",
        "Topic :: Text Processing :: Linguistic",
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
    ],
    python_requires=">=3.8",
    install_requires=read_requirements(),
    include_package_data=True,
    package_data={
        "": ["data/*.csv"],
    },
    entry_points={
        "console_scripts": [
            "ktranslator=KTranslator:main",
        ],
    },
    keywords="translator, ocr, screen capture, real-time, google translate",
    project_urls={
        "Bug Reports": "https://github.com/yourusername/KTranslator/issues",
        "Source": "https://github.com/yourusername/KTranslator",
    },
) 