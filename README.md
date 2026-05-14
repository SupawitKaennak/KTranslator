# KTranslator V2

[ภาษาไทย (Thai)](#thai) | [English](#english)

---
<img width="1917" height="1126" alt="{DD417730-D74E-48D7-B8F4-86A07DC7E0A6}" src="https://github.com/user-attachments/assets/b46b82c1-8395-4023-9a4f-83da7191a998" /><br>
<img width="1770" height="1068" alt="{C3BE31E9-110E-4894-8475-AFF12EEAE2B5}" src="https://github.com/user-attachments/assets/f1f0f849-1008-4acb-8806-6d7137ac9a33" /><br>
<img width="1601" height="641" alt="{9F263879-62FF-4DBC-881C-12D1DACB282B}" src="https://github.com/user-attachments/assets/cb1260b8-4180-4c3c-b8ed-d5b8c94d567f" /><br>
<img width="2554" height="1440" alt="{2A531C0F-AEC0-4498-9C81-D541733D3BD8}" src="https://github.com/user-attachments/assets/d278fe67-4039-4216-ad13-4a62af842980" /><br>

<a name="thai"></a>
## ภาษาไทย (Thai)

โปรแกรมแปลภาษาจากการจับภาพหน้าจอ (Screen Translator) เขียนด้วยภาษา Rust

### ลักษณะการใช้งาน
- **แปลเกม:** แปลบทสนทนา เมนู และคำอธิบายไอเทมภายในเกม
- **แปลมังงะและคอมมิค:** ตรวจจับและแปลข้อความแนวตั้ง แนวเอียง หรือข้อความในกรอบคำพูด
- **Manga-OCR & YOLOv8:** ตรวจจับกรอบข้อความและอ่านตัวอักษรภาษาญี่ปุ่นแนวตั้งโดยใช้โมเดล ONNX
- **Bubble Detection & Sorting:** ตรวจจับกรอบคำพูดและจัดเรียงลำดับการอ่านจากขวาไปซ้ายและบนลงล่าง รองรับหน้าคู่ (RTL Column-Major)
- **Word Segmentation:** แยกคำในข้อความภาษาอังกฤษที่ไม่มีการเว้นวรรคด้วยพจนานุกรม (`wordninja`)
- **Thai Word Wrap:** แทรกอักขระเว้นวรรคความกว้างศูนย์ (Zero Width Space) เพื่อรองรับการตัดบรรทัดภาษาไทย
- **Prompt Customization:** ปรับแต่ง System Prompt และ User Prompt พร้อมระบบแทนที่ตัวแปรบริบท
- **Telemetry Dashboard:** แสดงข้อมูลการทำงานภายในระบบ เช่น สถานะเธรด, ตัวนับเวลา, และพิกัดเค้าโครง
- **แปลเอกสารและหน้าเว็บ:** แปลข้อความจากรูปภาพ, PDF, หรือหน้าเว็บไซต์ที่ไม่สามารถคัดลอกข้อความได้
- **Sentence Merge:** รวมบรรทัดข้อความที่แยกกันให้เป็นประโยคเดียวเพื่อรักษาบริบทก่อนส่งแปล
- **Overlay Customization:** ปรับแต่งสีพื้นหลัง, สีตัวอักษร, ขนาดฟอนต์, และความโค้งของหน้าต่างแสดงผลคำแปล

### ความต้องการของระบบ (Requirements)

**1. ระบบ OCR (ตัวอ่านข้อความ)**
- **Manga-OCR (NEW):** ระบบ AI (ONNX) รันผ่าน GPU แม่นยำที่สุดสำหรับภาษาญี่ปุ่นแนวตั้ง (มีระบบ **Model Installation Center** ในตัวโปรแกรมเพื่อโหลดไฟล์โมเดลอัตโนมัติ)
- **Windows OCR:** (ติดมากับ Windows) ต้องติดตั้ง Language Pack ของภาษาต้นทางที่จะแปลให้เรียบร้อย (เช่น ญี่ปุ่น, จีน)
- **PaddleOCR:** (แนะนำสำหรับมังงะ) ต้องดาวน์โหลดตัวโปรแกรม [PaddleOCR-json](https://github.com/hiroi-sora/PaddleOCR-json/releases) และระบุที่อยู่ไฟล์ `.exe` ในหน้า Settings ของโปรแกรม

**2. ระบบการแปล (Translator)**
- **Google Translate (FREE):** แปลภาษาได้ทันทีโดยไม่ต้องใช้ API Key
- **Gemini:** ต้องใช้ API Key สมัครฟรีได้ที่ [Google AI Studio](https://aistudio.google.com/) 
- **Groq:** ต้องใช้ API Key สมัครฟรีได้ที่ [Groq Console](https://console.groq.com/) 
- **Ollama:** สำหรับการแปลแบบ Offline ดาวน์โหลดได้ที่ [Ollama.com](https://ollama.com/) 
- **Custom OpenAI:** รองรับ API ทุกเจ้าที่ใช้มาตรฐาน OpenAI (เช่น OpenRouter, DeepSeek, LM Studio) 

### แหล่งที่มาของโมเดล (Model Resources)
- **Manga-OCR 2025 (ONNX):** [l0wgear/manga-ocr-2025-onnx](https://huggingface.co/l0wgear/manga-ocr-2025-onnx) - โมเดล OCR คุณภาพสูงที่ปรับปรุงมาเพื่อมังงะญี่ปุ่นโดยเฉพาะ (เวอร์ชัน 2025)
- **YOLOv8 Text Detection:** [deepghs/manga109_yolo](https://huggingface.co/deepghs/manga109_yolo) - ใช้โมเดลเวอร์ชัน **manga109_yolo/v2023.12.07_s** (YOLOv8-Small) ซึ่งถูกปรับจูนมาเพื่อการตรวจจับตำแหน่งลูกโป่งคำพูดและข้อความในมังงะโดยเฉพาะ มีความสมดุลระหว่างความเร็วและความแม่นยำ

### เทคโนโลยีที่ใช้ (Tech Stack)
- **Language:** Rust (edition 2024)
- **UI Framework:** [egui](https://github.com/emilk/egui)
- **AI Models:** Vision Encoder-Decoder (Manga-OCR) & YOLOv8 (Text Detection)
- **NLP Engine:** Dictionary-based Tokenizer (`wordninja`)
- **Runtime:** ONNX Runtime with DirectML (GPU Acceleration)
- **OCR Engines:** Windows.Media.Ocr & PaddleOCR
- **Graphics:** Win32 API (สำหรับระบบ Overlay โปร่งใส)
- **Capture:** Screenshots crate พร้อมระบบ stabilization

### การติดตั้งและใช้งาน

**วิธีติดตั้ง (สำหรับนักพัฒนา):**
1. ติดตั้ง [Rust Toolchain](https://rustup.rs/)
2. Clone โปรเจกต์:
   ```bash
   git clone https://github.com/SupawitKaennak/KTranslatorV2.git
   cd KTranslatorV2
   ```
3. ดาวน์โหลดโมเดลมาวางไว้ที่ `models/manga-ocr/` (encoder, decoder, tokenizer, yolo)
4. รันโปรแกรม:
   ```bash
   cargo run --release
   ```

**ขั้นตอนการใช้งาน:**
1. เข้าไปที่ **Settings** (ไอคอนฟันเฟือง) เพื่อเลือก OCR และใส่ API Key
2. กด **Add Region** และเลือกพื้นที่บนหน้าจอที่ต้องการแปล
3. เลือกภาษาต้นทาง (From) และภาษาปลายทาง (To)
4. กดปุ่ม **Start** เพื่อเริ่มการแปล
5. เปิดโหมด **Overlay Mode** หากต้องการให้คำแปลแสดงทับตำแหน่งเดิมบนหน้าจอ
6. ปรับแต่งรูปลักษณ์ของ Overlay ได้ที่ **Appearance Settings** เช่น สีพื้นหลัง ความโปร่งใส และขนาดฟอนต์

---

<a name="english"></a>
## English

A powerful Screen Translator written in Rust for seamless real-time translation.

### Key Features
- **Game Translation:** Translate in-game dialogues, menus, and item descriptions.
- **Manga & Comics:** Detect and translate vertical, slanted, or curved text within speech bubbles.
- **Manga-OCR & YOLOv8:** Extract and recognize vertical Japanese text using specialized ONNX models.
- **Bubble Detection & Sorting:** Detect speech bubbles and sort reading order from right-to-left and top-to-bottom for double-page spreads (RTL Column-Major).
- **Word Segmentation:** Split unspaced English text into distinct words using a dictionary (`wordninja`).
- **Thai Word Wrap:** Inject Zero Width Spaces into text strings to enable line-breaking for Thai script.
- **Prompt Customization:** Modify System and User translation prompts with context variable interpolation.
- **Telemetry Dashboard:** Display internal pipeline metrics including thread status, timing counters, and layout coordinates.
- **Documents & Web Pages:** Translate text from uncopyable images, PDF files, or browser windows.
- **Sentence Merge:** Concatenate separate text lines into a single string to maintain translation context.
- **Overlay Customization:** Adjust background color, text color, font size, and window corner radius.

### System Requirements

**1. OCR Engines (Text Recognition)**
- **Manga-OCR (NEW):** High-precision AI recognition (ONNX) with GPU support.
- **Windows OCR:** Built-in. Requires language packs for source languages (e.g., Japanese, Chinese).
- **PaddleOCR:** Recommended for manga. Download [PaddleOCR-json](https://github.com/hiroi-sora/PaddleOCR-json/releases) and specify the `.exe` path in the app settings.

**2. Translation Providers**
- **Google Translate (FREE):** Instant translation without an API Key.
- **Gemini:** API Key required. Get it at [Google AI Studio](https://aistudio.google.com/) (Supports **Auto-Fetch**).
- **Groq:** High-speed API. Get your key at [Groq Console](https://console.groq.com/) (Supports **Auto-Fetch**).
- **Ollama:** For local/offline translation. Download at [Ollama.com](https://ollama.com/) (Supports **Auto-Fetch**).
- **Custom OpenAI:** Supports any OpenAI-compatible API (OpenRouter, DeepSeek, LM Studio) with **Auto-Fetch** model selection support.

### Tech Stack
- **Language:** Rust (edition 2024)
- **AI Models:** Vision Encoder-Decoder (Manga-OCR) & YOLOv8 (Text Detection)
- **NLP Engine:** Dictionary-based Tokenizer (`wordninja`)
- **Runtime:** ONNX Runtime with DirectML (GPU Acceleration)
- **UI Framework:** [egui](https://github.com/emilk/egui)
- **OCR Engines:** Windows.Media.Ocr & PaddleOCR
- **Graphics:** Win32 API (for transparent overlay system)
- **Capture:** Screenshots crate with stabilization logic

### Getting Started

**Installation (Developers):**
1. Install [Rust Toolchain](https://rustup.rs/).
2. Clone the repository and place the ONNX models in `models/manga-ocr/`.
3. Run the application:
   ```bash
   cargo run --release
   ```

**Basic Usage:**
1. Open **Settings** (gear icon) to select your OCR engine and enter API keys.
2. Click **Add Region** to select the area of the screen you want to translate.
3. Select Source (From) and Target (To) languages.
4. Click **Start** to begin the real-time translation loop.
5. Enable **Overlay Mode** to display translations directly over the original text.

### AI Models & Credits
- **Manga-OCR 2025 (ONNX):** [l0wgear/manga-ocr-2025-onnx](https://huggingface.co/l0wgear/manga-ocr-2025-onnx) - High-quality OCR model optimized for Japanese manga.
- **YOLOv8 Text Detection:** [deepghs/manga109_yolo](https://huggingface.co/deepghs/manga109_yolo) - Utilizing the **manga109_yolo/v2023.12.07_s** (Small) variant for real-time bubble and text detection with high precision and performance.

---

### License
Copyright (c) 2024 Supawit Kaennak [GPL v3.0](LICENSE). All rights reserved.
